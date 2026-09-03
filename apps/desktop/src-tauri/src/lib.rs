//! Puente entre el motor en Rust y la interfaz.
//!
//! La interfaz nunca habla con YouTube: solo invoca estos comandos y escucha el
//! evento `playback`. Toda la logica fragil queda del lado de Rust.

mod db;
mod discord;
mod lyrics;
mod media;
mod minter;
mod palette;
mod thumbs;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager};
use ytm_audio::{Command, Engine, PlaybackState, Repeat};
use ytm_source::{InnerTube, SearchPage, TrackInfo};

/// Estado global de la aplicacion.
struct App {
    engine: Engine,
    innertube: InnerTube,
    http: reqwest::Client,
    /// Cache de paletas: extraer una cuesta ~15 ms y la misma portada sale
    /// muchas veces (cola, historial, volver a la misma cancion).
    palettes: Arc<tokio::sync::Mutex<std::collections::HashMap<String, palette::Palette>>>,
    lyrics: Arc<tokio::sync::Mutex<std::collections::HashMap<String, Option<lyrics::Lyrics>>>>,
    db: Arc<db::Db>,
    ytdlp: Option<ytm_source::ytdlp::YtDlp>,
}

// --------------------------------------------------------------------------
// Busqueda y biblioteca
// --------------------------------------------------------------------------

/// Busca, y devuelve TODO: canciones, videos, artistas, albumes y playlists.
///
/// `params` sale de los filtros que trae la propia respuesta, no de una tabla
/// codificada aqui. Sin `params`, la busqueda es "todo mezclado".
#[tauri::command]
async fn search(
    state: tauri::State<'_, App>,
    query: String,
    params: Option<String>,
) -> Result<SearchPage, String> {
    state
        .innertube
        .search_page(&query, params.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Siguiente pagina de resultados.
#[tauri::command]
async fn search_more(
    state: tauri::State<'_, App>,
    continuation: String,
) -> Result<SearchPage, String> {
    state
        .innertube
        .search_more(&continuation)
        .await
        .map_err(|e| e.to_string())
}

// --------------------------------------------------------------------------
// Reproduccion
// --------------------------------------------------------------------------

/// Pista tal y como la manda la interfaz.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackInput {
    video_id: String,
    title: Option<String>,
    author: Option<String>,
    thumbnail: Option<String>,
    /// La busqueda ya sabe cuanto dura cada pista; mandarla evita que la cola
    /// se pinte sin duraciones hasta que cada una se resuelva.
    duration_ms: Option<u64>,
}

impl From<TrackInput> for TrackInfo {
    fn from(t: TrackInput) -> Self {
        TrackInfo {
            video_id: t.video_id,
            title: t.title,
            author: t.author,
            thumbnail: t.thumbnail,
            duration_ms: t.duration_ms,
        }
    }
}

#[tauri::command]
async fn playlist(
    state: tauri::State<'_, App>,
    id: String,
) -> Result<ytm_source::Playlist, String> {
    state.innertube.playlist(&id).await.map_err(|e| e.to_string())
}

/// Cola de recomendaciones a partir de una pista.
#[tauri::command]
async fn radio(
    state: tauri::State<'_, App>,
    video_id: String,
) -> Result<ytm_source::Radio, String> {
    state.innertube.radio(&video_id).await.map_err(|e| e.to_string())
}

/// Sustituye lo que viene despues de la pista actual.
///
/// La radio se pide mientras la cancion ya suena; usar `play_queue` la
/// reiniciaria desde cero.
#[tauri::command]
fn set_up_next(state: tauri::State<'_, App>, tracks: Vec<TrackInput>) {
    let tracks: Vec<TrackInfo> = tracks.into_iter().map(Into::into).collect();
    state.engine.send(Command::SetUpNext { tracks });
}

/// Sugerencias mientras se escribe.
#[tauri::command]
async fn search_suggestions(
    state: tauri::State<'_, App>,
    query: String,
) -> Result<Vec<String>, String> {
    state
        .innertube
        .search_suggestions(&query)
        .await
        .map_err(|e| e.to_string())
}

/// Feed de inicio.
#[tauri::command]
async fn home(state: tauri::State<'_, App>) -> Result<ytm_source::BrowsePage, String> {
    state.innertube.home().await.map_err(|e| e.to_string())
}

/// Una pagina de `browse`: artista, album o playlist.
///
/// Un solo comando en vez de uno por tipo: para InnerTube todos son el mismo
/// endpoint con distinto `browseId`, y que cosa es cada id ya lo dice el
/// `ItemKind` del elemento que llevo hasta aqui.
#[tauri::command]
async fn browse(
    state: tauri::State<'_, App>,
    browse_id: String,
    params: Option<String>,
) -> Result<ytm_source::BrowsePage, String> {
    state
        .innertube
        .browse(&browse_id, params.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn play_queue(state: tauri::State<'_, App>, tracks: Vec<TrackInput>, start: usize) {
    tracing::info!(count = tracks.len(), start, "Comando play_queue recibido");
    let tracks: Vec<TrackInfo> = tracks.into_iter().map(Into::into).collect();
    state.engine.send(Command::SetQueue { tracks, start });
}

#[tauri::command]
fn play_now(state: tauri::State<'_, App>, video_id: String) {
    tracing::info!(video_id = %video_id, "Comando play_now recibido");
    state.engine.send(Command::PlayNow(video_id));
}

#[tauri::command]
fn toggle_play(state: tauri::State<'_, App>) {
    state.engine.send(Command::TogglePlay);
}

#[tauri::command]
fn next_track(state: tauri::State<'_, App>) {
    state.engine.send(Command::Next);
}

#[tauri::command]
fn prev_track(state: tauri::State<'_, App>) {
    state.engine.send(Command::Prev);
}

#[tauri::command]
fn jump_to(state: tauri::State<'_, App>, index: usize) {
    state.engine.send(Command::JumpTo(index));
}

#[tauri::command]
fn seek(state: tauri::State<'_, App>, position_ms: u64) {
    state
        .engine
        .send(Command::Seek(Duration::from_millis(position_ms)));
}

#[tauri::command]
fn set_volume(state: tauri::State<'_, App>, volume: f32) {
    state.engine.send(Command::SetVolume(volume));
}

#[tauri::command]
fn set_repeat(state: tauri::State<'_, App>, mode: String) {
    let mode = match mode.as_str() {
        "all" => Repeat::All,
        "one" => Repeat::One,
        _ => Repeat::Off,
    };
    state.engine.send(Command::SetRepeat(mode));
}

#[tauri::command]
fn set_shuffle(state: tauri::State<'_, App>, on: bool) {
    state.engine.send(Command::SetShuffle(on));
}

#[tauri::command]
fn get_state(state: tauri::State<'_, App>) -> PlaybackState {
    state.engine.state()
}

// --------------------------------------------------------------------------
// Estetica
// --------------------------------------------------------------------------

#[tauri::command]
async fn get_palette(
    state: tauri::State<'_, App>,
    url: String,
) -> Result<palette::Palette, String> {
    if let Some(cached) = state.palettes.lock().await.get(&url) {
        return Ok(cached.clone());
    }
    // Los mismos bytes que muestra la interfaz, sin segunda peticion.
    let bytes = thumbs::cached_bytes(&url).await.map_err(|e| e.to_string())?;
    let p = palette::from_bytes(&bytes).map_err(|e| e.to_string())?;
    state.palettes.lock().await.insert(url, p.clone());
    Ok(p)
}

#[tauri::command]
async fn get_lyrics(
    state: tauri::State<'_, App>,
    title: String,
    artist: String,
    duration_ms: u64,
) -> Result<Option<lyrics::Lyrics>, String> {
    let key = format!("{title}|{artist}");
    if let Some(hit) = state.lyrics.lock().await.get(&key) {
        return Ok(hit.clone());
    }
    // Una letra que no existe tambien se cachea: si no, cada cambio de pestana
    // repetiria la busqueda fallida.
    let found = lyrics::fetch(&state.http, &title, &artist, duration_ms / 1000)
        .await
        .unwrap_or(None);
    state.lyrics.lock().await.insert(key, found.clone());
    Ok(found)
}

/// Acuna la URL de audio de una pista en un webview oculto.
///
/// Es la unica forma de obtener una URL sin el tope de 1 MiB: `poToken`, `sig` y
/// `n` los produce el JavaScript de YouTube. Ver `minter.rs`.
#[tauri::command]
async fn mint_url(app: tauri::AppHandle, video_id: String) -> Result<String, String> {
    minter::mint(&app, &video_id).await.map_err(|e| e.to_string())
}

// --------------------------------------------------------------------------
// Biblioteca local
// --------------------------------------------------------------------------

#[tauri::command]
fn toggle_favorite(state: tauri::State<'_, App>) -> Result<bool, String> {
    let s = state.engine.state();
    let track = s.track.ok_or("nada sonando")?;
    state
        .db
        .toggle_favorite(&db::SavedTrack {
            video_id: track.video_id,
            title: track.title,
            author: track.author,
            thumbnail: track.thumbnail,
            at: 0,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn is_favorite(state: tauri::State<'_, App>, video_id: String) -> bool {
    state.db.is_favorite(&video_id)
}

#[tauri::command]
fn favorites(state: tauri::State<'_, App>) -> Result<Vec<db::SavedTrack>, String> {
    state.db.favorites().map_err(|e| e.to_string())
}

#[tauri::command]
fn history(state: tauri::State<'_, App>) -> Result<Vec<db::SavedTrack>, String> {
    state.db.history(100).map_err(|e| e.to_string())
}

// --------------------------------------------------------------------------
// Diagnostico (Fase 6)
// --------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientHealth {
    id: String,
    status: String,
    direct_audio: usize,
    best: Option<String>,
}

/// Estado del extractor: es la pieza que YouTube rompe, asi que su version y
/// su ruta se ensenan en el panel de diagnostico.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExtractorStatus {
    available: bool,
    version: Option<String>,
    program: Option<String>,
}

#[tauri::command]
fn extractor_status(state: tauri::State<'_, App>) -> ExtractorStatus {
    match &state.ytdlp {
        Some(y) => ExtractorStatus {
            available: true,
            version: Some(y.version.clone()),
            program: Some(y.describe()),
        },
        None => ExtractorStatus {
            available: false,
            version: None,
            program: None,
        },
    }
}

/// Equivalente en la app a `ytm-spike probe`: cuando algo deja de sonar, dice en
/// 10 segundos si YouTube cerro un cliente y cual sigue en pie.
#[tauri::command]
async fn diagnose(state: tauri::State<'_, App>) -> Result<Vec<ClientHealth>, String> {
    // Un video de prueba estable y sin restricciones regionales.
    const PROBE_VIDEO: &str = "dQw4w9WgXcQ";
    let mut out = Vec::new();

    for client in ytm_source::clients::ALL {
        let health = match state.innertube.player(PROBE_VIDEO, *client).await {
            Err(e) => ClientHealth {
                id: client.id.into(),
                status: format!("error: {e}"),
                direct_audio: 0,
                best: None,
            },
            Ok(res) => {
                let status = res
                    .playability_status
                    .as_ref()
                    .and_then(|p| p.status.clone())
                    .unwrap_or_else(|| "SIN_ESTADO".into());
                let formats = res
                    .streaming_data
                    .map(|s| s.adaptive_formats)
                    .unwrap_or_default();
                let audio: Vec<_> = formats.into_iter().filter(|f| f.is_audio()).collect();
                let direct = audio.iter().filter(|f| f.is_playable_directly()).count();
                let best = ytm_source::select::best_audio(&audio).map(|f| {
                    format!(
                        "itag {} {} {} kbps",
                        f.itag,
                        f.codec().unwrap_or("?"),
                        f.bitrate.or(f.average_bitrate).unwrap_or(0) / 1000
                    )
                });
                ClientHealth {
                    id: client.id.into(),
                    status,
                    direct_audio: direct,
                    best,
                }
            }
        };
        out.push(health);
    }
    Ok(out)
}

// --------------------------------------------------------------------------
// Ventana sin marco
// --------------------------------------------------------------------------

#[tauri::command]
fn window_minimize(window: tauri::WebviewWindow) {
    let _ = window.minimize();
}

#[tauri::command]
fn window_toggle_maximize(window: tauri::WebviewWindow) {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
}

#[tauri::command]
fn window_close(window: tauri::WebviewWindow) {
    let _ = window.close();
}

// --------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    {
        // Evita que Chromium/WebView2 congele o suspenda el bucle de renderizado
        // y de mensajes cuando la ventana pasa a estar oculta o minimizada.
        let cur = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
        let args = format!("{cur} --disable-background-timer-throttling --disable-backgrounding-occluded-windows");
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", args.trim());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "posible=info,ytm_source=info,ytm_audio=info".into()),
        )
        .with_target(false)
        .init();

    let is_minimized = Arc::new(AtomicBool::new(false));
    let is_minimized_for_event = Arc::clone(&is_minimized);
    let is_minimized_for_setup = Arc::clone(&is_minimized);

    thumbs::register(tauri::Builder::default())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(move |window, event| {
            match event {
                tauri::WindowEvent::Resized(size) => {
                    if size.width == 0 || size.height == 0 {
                        is_minimized_for_event.store(true, Ordering::Release);
                    } else {
                        let was = is_minimized_for_event.swap(false, Ordering::AcqRel);
                        if was {
                            if let Some(app) = window.try_state::<App>() {
                                let state = app.engine.state();
                                let _ = window.emit("playback", state);
                            }
                        }
                    }
                }
                tauri::WindowEvent::Focused(true) => {
                    if let Some(app) = window.try_state::<App>() {
                        let state = app.engine.state();
                        let _ = window.emit("playback", state);
                    }
                }
                _ => {}
            }
        })
        .setup(move |app| {
            let is_minimized_for_rx = Arc::clone(&is_minimized_for_setup);
            // `setup` corre FUERA del contexto del runtime asincrono, asi que
            // cualquier `tokio::spawn` dentro de `Engine::start` entraria en
            // panico ("there is no reactor running"). Entramos en el runtime de
            // Tauri durante la construccion; asi `ytm-audio` no necesita saber
            // nada de Tauri.
            let runtime = tauri::async_runtime::handle();
            let _guard = runtime.inner().enter();

            // El acunador necesita el webview, asi que lo inyecta la app: el
            // crate de audio no depende de Tauri.
            // yt-dlp primero: es lo unico que trae pistas enteras (ver
            // `ytdlp.rs`). El acunador queda de respaldo, capado a ~48 s.
            let ytdlp = ytm_source::ytdlp::YtDlp::detect();
            if ytdlp.is_none() {
                tracing::warn!("yt-dlp no encontrado: se usara el acunador (pistas cortadas)");
            }
            let ytdlp_for_state = ytdlp.clone();
            let handle_for_mint = app.handle().clone();
            let provider: ytm_audio::SourceProvider =
                Arc::new(move |video_id: String, dir: std::path::PathBuf| {
                    let h = handle_for_mint.clone();
                    let y = ytdlp.clone();
                    Box::pin(async move {
                        tracing::info!(video_id = %video_id, "source_provider: resolviendo fuente de audio...");
                        if let Some(y) = y {
                            tracing::info!(video_id = %video_id, "Iniciando descarga de pista con yt-dlp...");
                            match y.download(&video_id, &dir).await {
                                Ok(d) => {
                                    let info = d.info.clone();
                                    let mime = info.mime();
                                    tracing::info!(
                                        video_id = %video_id,
                                        title = ?info.title,
                                        path = ?info.path,
                                        "yt-dlp resolvio con exito la pista"
                                    );
                                    return Ok(ytm_audio::Provided::External {
                                        path: info.path,
                                        size: info.size,
                                        mime,
                                        done: Box::pin(d.wait()),
                                        title: info.title,
                                        author: info.author,
                                        thumbnail: info.thumbnail,
                                        duration_ms: info.duration_ms,
                                    });
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "yt-dlp fallo; se intenta el acunador")
                                }
                            }
                        }
                        tracing::warn!(video_id = %video_id, "Llamando a minter::mint (posible ventana oculta)...");
                        let url = minter::mint(&h, &video_id).await?;
                        tracing::info!(video_id = %video_id, "minter::mint finalizo con exito");
                        Ok(ytm_audio::Provided::Url { url, size: None, mime: None })
                    })
                });

            let engine = Engine::start(Some(provider))?;
            let innertube = InnerTube::new()?;
            let database = Arc::new(db::Db::open()?);

            // Ajustes de la sesion anterior.
            if let Some(v) = database.get_setting("volume").and_then(|v| v.parse().ok()) {
                engine.send(Command::SetVolume(v));
            }
            if let Some(r) = database.get_setting("repeat") {
                let mode = match r.as_str() {
                    "all" => Repeat::All,
                    "one" => Repeat::One,
                    _ => Repeat::Off,
                };
                engine.send(Command::SetRepeat(mode));
            }
            if database.get_setting("shuffle").as_deref() == Some("true") {
                engine.send(Command::SetShuffle(true));
            }

            // Esquinas redondeadas via DWM. La ventana dejo de ser transparente
            // (transparent + WebView2 se congela al restaurar desde minimizado
            // en Windows), asi que el redondeo CSS ya no puede recortarla: se le
            // pide al compositor, que ademas dibuja borde y sombra nativos.
            #[cfg(target_os = "windows")]
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
                #[cfg(debug_assertions)]
                {
                    w.open_devtools();
                }
            }

            // Interruptor de diagnostico: la tarjeta multimedia (SMTC) va por
            // COM en el hilo principal y es sospechosa del congelamiento al
            // restaurar. Con POSIBLE_NO_SMTC=1 se desactiva entera para poder
            // bisecar el fallo con el arnes de minimizar/restaurar.
            let smtc_enabled = std::env::var("POSIBLE_NO_SMTC").is_err();
            if smtc_enabled {
                media::init(app.handle(), engine.clone());
            } else {
                tracing::warn!("SMTC desactivado (POSIBLE_NO_SMTC)");
            }

            // Sonda de reproduccion: `POSIBLE_PLAY_TEST=<videoId>` reproduce una
            // pista al arrancar y registra el progreso. Verifica la cadena
            // completa (acunar, descargar, decodificar Opus, sonar) sin
            // depender de hacer clic en la interfaz.
            if let Ok(id) = std::env::var("POSIBLE_PLAY_TEST") {
                let e = engine.clone();
                tauri::async_runtime::spawn(async move {
                    let t = std::time::Instant::now();
                    e.send(Command::PlayNow(id));
                    let mut last = 0u64;
                    for _ in 0..100 {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        let s = e.state();
                        if let Some(err) = &s.error {
                            tracing::error!(error = %err, "PLAY FALLO");
                            return;
                        }
                        if s.position_ms != last {
                            last = s.position_ms;
                            tracing::info!(
                                pos_s = s.position_ms / 1000,
                                dur_s = s.duration_ms / 1000,
                                buf = format!("{:.0}%", s.buffered * 100.0),
                                playing = s.playing,
                                "PLAY"
                            );
                        }
                        // Mas de 65 s reproducidos = el muro de 1 MiB, superado.
                        if s.position_ms > 70_000 {
                            tracing::info!(
                                ms = t.elapsed().as_millis() as u64,
                                "PLAY OK: superado el limite de 1 MiB"
                            );
                            return;
                        }
                    }
                    tracing::warn!(pos_ms = last, "PLAY: no llego a 70 s");
                });
            }

            // Sonda de acunacion: `POSIBLE_MINT_TEST=<videoId>` prueba el
            // webview oculto y registra el resultado. Sirve para verificar el
            // camino completo sin depender de la interfaz.
            if let Ok(id) = std::env::var("POSIBLE_MINT_TEST") {
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let t = std::time::Instant::now();
                    let origin = if std::env::var("POSIBLE_MINT_WATCH").is_ok() {
                        minter::Origin::Watch
                    } else {
                        minter::Origin::Music
                    };
                    match minter::mint_from(&h, &id, origin).await {
                        Ok(url) => {
                            let itag = url
                                .split("itag=")
                                .nth(1)
                                .and_then(|s| s.split('&').next())
                                .unwrap_or("?")
                                .to_string();
                            let clen = url
                                .split("clen=")
                                .nth(1)
                                .and_then(|s| s.split('&').next())
                                .unwrap_or("?")
                                .to_string();
                            if let Ok(out) = std::env::var("POSIBLE_MINT_OUT") {
                                let _ = std::fs::write(&out, &url);
                            }

                            // Prueba inmediata desde ESTE proceso, para aislar
                            // si el 403 depende del proceso o del descargador.
                            tracing::info!(
                                ms = t.elapsed().as_millis() as u64,
                                itag,
                                clen,
                                pot = url.contains("pot="),
                                "MINT OK"
                            )
                        }
                        Err(e) => tracing::error!(error = %e, "MINT FALLO"),
                    }
                });
            }
            if std::env::var("POSIBLE_NO_DISCORD").is_err() {
                discord::spawn(engine.subscribe());
            } else {
                tracing::warn!("Discord RPC desactivado (POSIBLE_NO_DISCORD)");
            }

            // Reenvia cada cambio de estado del motor a la interfaz. Es un canal
            // `watch`, asi que no hay sondeo: la interfaz solo se despierta
            // cuando algo cambia de verdad.
            let mut rx = engine.subscribe();
            let handle = app.handle().clone();
            let db_for_task = Arc::clone(&database);
            tauri::async_runtime::spawn(async move {
                let mut last_track: Option<String> = None;
                let mut last_prefs = (f32::NAN, String::new(), false);
                let mut last_sent_rev: Option<u64> = None;
                let mut last_media = (String::new(), false);
                let mut media_primed = false;

                while rx.changed().await.is_ok() {
                    let state = rx.borrow().clone();

                    let media_key = (
                        state
                            .track
                            .as_ref()
                            .map(|t| t.video_id.clone())
                            .unwrap_or_default(),
                        state.playing,
                    );
                    let track_changed = !media_primed || media_key.0 != last_media.0;
                    let play_changed = !media_primed || media_key.1 != last_media.1;

                    // Si la ventana está minimizada, no enviamos 10 eventos por segundo
                    // a WebView2 (Chromium suspende su renderizado y acumular mensajes IPC
                    // bloqueaba la ventana al restaurar). Solo emitimos si cambió de pista
                    // o el estado de reproducción.
                    let is_min = is_minimized_for_rx.load(Ordering::Acquire);
                    if !is_min || track_changed || play_changed {
                        let mut wire = state.clone();
                        if last_sent_rev == Some(wire.queue_rev) {
                            wire.queue = Vec::new();
                        } else {
                            last_sent_rev = Some(wire.queue_rev);
                        }
                        let _ = handle.emit("playback", wire);
                    }

                    // La tarjeta multimedia del sistema (SMTC) va por COM en el
                    // HILO PRINCIPAL. Actualizarla solo cuando cambie de pista o
                    // de estado play/pause, NUNCA en un sondeo periódico de posición,
                    // que satura el bucle de mensajes de Windows al minimizar.
                    if smtc_enabled && (track_changed || play_changed) {
                        media_primed = true;
                        last_media = media_key;
                        let for_media = state.clone();
                        let _ = handle.run_on_main_thread(move || {
                            if track_changed {
                                media::update(&for_media);
                            } else {
                                media::update_playback(&for_media);
                            }
                        });
                    }

                    // Historial: al cambiar de pista, no en cada tick.
                    if let Some(t) = &state.track {
                        if last_track.as_deref() != Some(t.video_id.as_str()) {
                            last_track = Some(t.video_id.clone());
                            let _ = db_for_task.push_history(&db::SavedTrack {
                                video_id: t.video_id.clone(),
                                title: t.title.clone(),
                                author: t.author.clone(),
                                thumbnail: t.thumbnail.clone(),
                                at: 0,
                            });
                        }
                    }

                    // Ajustes: solo cuando cambian de verdad, para no escribir
                    // en disco diez veces por segundo.
                    let repeat = format!("{:?}", state.repeat).to_lowercase();
                    let prefs = (state.volume, repeat.clone(), state.shuffle);
                    if prefs != last_prefs {
                        last_prefs = prefs;
                        let _ = db_for_task.set_setting("volume", &state.volume.to_string());
                        let _ = db_for_task.set_setting("repeat", &repeat);
                        let _ = db_for_task.set_setting("shuffle", &state.shuffle.to_string());
                    }
                }
            });

            app.manage(App {
                engine,
                innertube,
                http: reqwest::Client::new(),
                palettes: Arc::new(tokio::sync::Mutex::new(Default::default())),
                lyrics: Arc::new(tokio::sync::Mutex::new(Default::default())),
                db: database,
                ytdlp: ytdlp_for_state,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            search_more,
            search_suggestions,
            radio,
            set_up_next,
            playlist,
            home,
            browse,
            play_queue,
            play_now,
            toggle_play,
            next_track,
            prev_track,
            jump_to,
            seek,
            set_volume,
            set_repeat,
            set_shuffle,
            get_state,
            get_palette,
            get_lyrics,
            mint_url,
            toggle_favorite,
            is_favorite,
            favorites,
            history,
            diagnose,
            extractor_status,
            window_minimize,
            window_toggle_maximize,
            window_close,
        ])
        .run(tauri::generate_context!())
        .expect("error al arrancar la aplicacion");
}
