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

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager};
use ytm_audio::{Command, Engine, PlaybackState, Repeat};
use ytm_source::{Filter, InnerTube, SearchResult, TrackInfo};

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
}

// --------------------------------------------------------------------------
// Busqueda y biblioteca
// --------------------------------------------------------------------------

#[tauri::command]
async fn search(
    state: tauri::State<'_, App>,
    query: String,
    only_songs: bool,
) -> Result<Vec<SearchResult>, String> {
    let filter = if only_songs { Filter::Songs } else { Filter::All };
    state
        .innertube
        .search(&query, filter)
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
}

impl From<TrackInput> for TrackInfo {
    fn from(t: TrackInput) -> Self {
        TrackInfo {
            video_id: t.video_id,
            title: t.title,
            author: t.author,
            thumbnail: t.thumbnail,
        }
    }
}

#[tauri::command]
fn play_queue(state: tauri::State<'_, App>, tracks: Vec<TrackInput>, start: usize) {
    let tracks: Vec<TrackInfo> = tracks.into_iter().map(Into::into).collect();
    state.engine.send(Command::SetQueue { tracks, start });
}

#[tauri::command]
fn play_now(state: tauri::State<'_, App>, video_id: String) {
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
    let p = palette::from_url(&state.http, &url)
        .await
        .map_err(|e| e.to_string())?;
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
fn window_minimize(window: tauri::Window) {
    let _ = window.minimize();
}

#[tauri::command]
fn window_toggle_maximize(window: tauri::Window) {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
}

#[tauri::command]
fn window_close(window: tauri::Window) {
    let _ = window.close();
}

// --------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "posible=info,ytm_source=info,ytm_audio=info".into()),
        )
        .with_target(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // `setup` corre FUERA del contexto del runtime asincrono, asi que
            // cualquier `tokio::spawn` dentro de `Engine::start` entraria en
            // panico ("there is no reactor running"). Entramos en el runtime de
            // Tauri durante la construccion; asi `ytm-audio` no necesita saber
            // nada de Tauri.
            let runtime = tauri::async_runtime::handle();
            let _guard = runtime.inner().enter();

            // El acunador necesita el webview, asi que lo inyecta la app: el
            // crate de audio no depende de Tauri.
            let handle_for_mint = app.handle().clone();
            let provider: ytm_audio::UrlProvider = Arc::new(move |video_id: String| {
                let h = handle_for_mint.clone();
                Box::pin(async move { minter::mint(&h, &video_id).await })
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

            media::init(app.handle(), engine.clone());

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
                    for _ in 0..40 {
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
            discord::spawn(engine.subscribe());

            // Reenvia cada cambio de estado del motor a la interfaz. Es un canal
            // `watch`, asi que no hay sondeo: la interfaz solo se despierta
            // cuando algo cambia de verdad.
            let mut rx = engine.subscribe();
            let handle = app.handle().clone();
            let db_for_task = Arc::clone(&database);
            tauri::async_runtime::spawn(async move {
                let mut last_track: Option<String> = None;
                let mut last_prefs = (f32::NAN, String::new(), false);

                while rx.changed().await.is_ok() {
                    let state = rx.borrow().clone();
                    let _ = handle.emit("playback", state.clone());

                    // La tarjeta del sistema solo se toca desde el hilo
                    // principal: `MediaControls` no es `Send` en Windows.
                    let for_media = state.clone();
                    let _ = handle.run_on_main_thread(move || media::update(&for_media));

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
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
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
            window_minimize,
            window_toggle_maximize,
            window_close,
        ])
        .run(tauri::generate_context!())
        .expect("error al arrancar la aplicacion");
}
