//! Puente entre el motor en Rust y la interfaz.
//!
//! La interfaz nunca habla con YouTube: solo invoca estos comandos y escucha el
//! evento `playback`. Toda la logica fragil queda del lado de Rust.

mod actualizacion;
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

/// Siguiente tanda de pistas de una pagina ya abierta.
#[tauri::command]
async fn browse_more(
    state: tauri::State<'_, App>,
    continuation: String,
) -> Result<ytm_source::BrowsePage, String> {
    state
        .innertube
        .browse_more(&continuation)
        .await
        .map_err(|e| e.to_string())
}

/// Cuela una pista justo despues de la que suena.
#[tauri::command]
fn play_next(state: tauri::State<'_, App>, track: TrackInput) {
    state.engine.send(Command::PlayNext(track.into()));
}

/// Anade una pista al final de la cola.
#[tauri::command]
fn enqueue(state: tauri::State<'_, App>, track: TrackInput) {
    state.engine.send(Command::Enqueue(track.into()));
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

// --------------------------------------------------------------------------
// Almacenamiento y ajustes
// --------------------------------------------------------------------------

/// Lo que ocupa en disco la cache de audio y la base de datos.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Storage {
    cache_bytes: u64,
    cache_files: u64,
    /// Tope en bytes. 0 es sin limite.
    cache_limit: u64,
    db_bytes: u64,
    cache_dir: String,
    data_dir: String,
}

/// Tope de la cache guardado, o el de fabrica si no hay ninguno.
fn limite_cache(database: &db::Db) -> u64 {
    database
        .get_setting("cache_limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(ytm_audio::cache::DEFAULT_LIMIT)
}

/// Suma el tamanio de un directorio sin bajar a subdirectorios.
///
/// La cache es plana — un archivo por pista — asi que no hace falta recursion,
/// y no bajar evita que un enlace simbolico mal puesto haga recorrer medio
/// disco.
fn dir_size(dir: &std::path::Path) -> (u64, u64) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    entradas
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .fold((0, 0), |(bytes, n), m| (bytes + m.len(), n + 1))
}

#[tauri::command]
fn storage_info(state: tauri::State<'_, App>) -> Storage {
    let cache = ytm_audio::cache_dir();
    let datos = db::data_dir();
    let (cache_bytes, cache_files) = dir_size(&cache);
    let db_bytes = std::fs::metadata(datos.join("posible.db"))
        .map(|m| m.len())
        .unwrap_or(0);

    Storage {
        cache_bytes,
        cache_files,
        cache_limit: limite_cache(&state.db),
        db_bytes,
        cache_dir: cache.display().to_string(),
        data_dir: datos.display().to_string(),
    }
}

/// Vacia la cache de audio. No toca la base de datos.
///
/// Se borran solo archivos del primer nivel, por lo mismo que `dir_size`: un
/// borrado recursivo sobre una ruta inesperada es de las pocas cosas de esta
/// aplicacion que no tienen vuelta atras.
#[tauri::command]
fn clear_cache() -> Result<u64, String> {
    let dir = ytm_audio::cache_dir();
    let Ok(entradas) = std::fs::read_dir(&dir) else {
        return Ok(0);
    };
    let mut borrados = 0;
    for e in entradas.filter_map(Result::ok) {
        if e.metadata().map(|m| m.is_file()).unwrap_or(false) && std::fs::remove_file(e.path()).is_ok()
        {
            borrados += 1;
        }
    }
    Ok(borrados)
}

/// Cambia el tope de la cache y recorta al momento si ya se pasaba.
///
/// Se guarda en la base de datos y no en `localStorage` porque quien tiene que
/// respetarlo es el motor, que arranca antes que la interfaz.
#[tauri::command]
fn set_cache_limit(state: tauri::State<'_, App>, bytes: u64) -> Result<(), String> {
    state
        .db
        .set_setting("cache_limit", &bytes.to_string())
        .map_err(|e| e.to_string())?;
    state.engine.send(Command::SetCacheLimit(bytes));
    Ok(())
}

/// Cuantas pistas rotas se arreglan por arranque.
///
/// Son peticiones de 9 KB, pero en serie y con pausa: esto corre mientras el
/// usuario esta usando la aplicacion y no debe competir con lo que pida el.
/// Cuarenta cubren de sobra el historial visible; si quedan mas, caen en el
/// siguiente arranque.
const REPARAR_POR_ARRANQUE: usize = 40;

/// Recupera el nombre de las pistas que se guardaron sin el.
///
/// Solo arregla lo VIEJO: las nuevas ya no pueden guardarse rotas desde que el
/// motor pide los metadatos aparte. Esto existe porque una base de datos ya
/// creada arrastra las que se guardaron antes, y esas no se arreglan solas.
fn reparar_metadatos(
    handle: tauri::AppHandle,
    db: Arc<db::Db>,
    it: ytm_source::InnerTube,
) {
    tauri::async_runtime::spawn(async move {
        let pendientes = match db.tracks_sin_titulo(REPARAR_POR_ARRANQUE) {
            Ok(v) if !v.is_empty() => v,
            _ => return,
        };
        tracing::info!(cuantas = pendientes.len(), "reparando metadatos guardados");

        let mut arregladas = 0;
        for id in pendientes {
            match ytm_source::metadata(&it, &id).await {
                Ok(m) => {
                    let title = m.title.unwrap_or_default();
                    if title.is_empty() {
                        continue; // sigue sin saberse; no se pisa con vacio
                    }
                    let author = m.author.unwrap_or_else(|| ytm_audio::SIN_AUTOR.to_string());
                    if db.fill_track_meta(&id, &title, &author, m.thumbnail.as_deref()).is_ok() {
                        arregladas += 1;
                    }
                }
                Err(e) => tracing::debug!(video_id = %id, error = %e, "sin metadatos"),
            }
            // Con calma: esto es trabajo de fondo, no urge.
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }

        if arregladas > 0 {
            tracing::info!(arregladas, "metadatos recuperados");
            let _ = handle.emit("biblioteca", ());
        }
    });
}

/// Iguala el volumen entre pistas, o deja de hacerlo.
#[tauri::command]
fn set_normalize(state: tauri::State<'_, App>, on: bool) -> Result<(), String> {
    state
        .db
        .set_setting("normalize", if on { "true" } else { "false" })
        .map_err(|e| e.to_string())?;
    state.engine.send(Command::SetNormalize(on));
    Ok(())
}

/// Ajustes que guarda el backend y que la interfaz necesita al arrancar.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Ajustes {
    normalize: bool,
}

#[tauri::command]
fn settings(state: tauri::State<'_, App>) -> Ajustes {
    Ajustes {
        normalize: state.db.get_setting("normalize").as_deref() != Some("false"),
    }
}

/// Version de la aplicacion, de `Cargo.toml`.
#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// --------------------------------------------------------------------------
// Playlists locales
// --------------------------------------------------------------------------

#[tauri::command]
fn create_playlist(state: tauri::State<'_, App>, name: String) -> Result<i64, String> {
    if name.trim().is_empty() {
        return Err("el nombre no puede estar vacio".into());
    }
    state.db.create_playlist(&name).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_playlist(state: tauri::State<'_, App>, id: i64, name: String) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("el nombre no puede estar vacio".into());
    }
    state.db.rename_playlist(id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_playlist(state: tauri::State<'_, App>, id: i64) -> Result<(), String> {
    state.db.delete_playlist(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn playlists(state: tauri::State<'_, App>) -> Result<Vec<db::Playlist>, String> {
    state.db.playlists().map_err(|e| e.to_string())
}

#[tauri::command]
fn playlist_tracks(state: tauri::State<'_, App>, id: i64) -> Result<Vec<db::SavedTrack>, String> {
    state.db.playlist_tracks(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_to_playlist(
    state: tauri::State<'_, App>,
    id: i64,
    track: TrackInput,
) -> Result<(), String> {
    let guardada = db::SavedTrack {
        video_id: track.video_id,
        title: track.title.unwrap_or_else(|| "Sin titulo".into()),
        author: track.author.unwrap_or_else(|| "Desconocido".into()),
        thumbnail: track.thumbnail,
        at: 0,
    };
    state.db.add_to_playlist(id, &guardada).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_from_playlist(
    state: tauri::State<'_, App>,
    id: i64,
    video_id: String,
) -> Result<(), String> {
    state
        .db
        .remove_from_playlist(id, &video_id)
        .map_err(|e| e.to_string())
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
                .unwrap_or_else(|_| "myglossmusic=info,ytm_source=info,ytm_audio=info".into()),
        )
        .with_target(false)
        .init();

    let is_minimized = Arc::new(AtomicBool::new(false));
    let is_minimized_for_event = Arc::clone(&is_minimized);
    let is_minimized_for_setup = Arc::clone(&is_minimized);

    thumbs::register(tauri::Builder::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            let provider: ytm_audio::SourceProvider = Arc::new(
                move |video_id: String, dir: std::path::PathBuf, urgencia: ytm_audio::Urgencia| {
                    let h = handle_for_mint.clone();
                    let y = ytdlp.clone();
                    Box::pin(async move {
                        tracing::info!(video_id = %video_id, ?urgencia, "resolviendo fuente de audio");
                        if let Some(y) = y {
                            match y.download(&video_id, &dir).await {
                                Ok(d) => {
                                    let info = d.info.clone();
                                    let mime = info.mime();
                                    tracing::info!(
                                        video_id = %video_id,
                                        title = ?info.title,
                                        "yt-dlp resolvio la pista"
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
                                    // Dos casos en los que el acunador solo
                                    // sirve para perder sus 25 s de margen.
                                    //
                                    // 1. La pista no existe, es privada o esta
                                    //    bloqueada. No hay nada que extraer, y
                                    //    el acunador tarda 25 s en descubrir lo
                                    //    mismo que yt-dlp ya sabia en dos.
                                    //    Medido el 2026-09-05 con un
                                    //    `Video unavailable` que costo 27 s en
                                    //    vez de 2.
                                    if matches!(
                                        e.downcast_ref::<ytm_source::ytdlp::SinPista>(),
                                        Some(f) if f.motivo == ytm_source::ytdlp::Motivo::NoDisponible
                                    ) {
                                        tracing::info!(
                                            video_id = %video_id,
                                            error = %e,
                                            "la pista no esta disponible; no se intenta el acunador"
                                        );
                                        return Err(e);
                                    }
                                    // 2. Es una precarga. Abrir un webview
                                    //    oculto por una cancion que puede que no
                                    //    suene nunca es desproporcionado, y
                                    //    ademas compite con la que SI esta
                                    //    sonando. Si luego se pulsa esa pista,
                                    //    llegara por el camino de `Ahora` y
                                    //    entonces si se acuna.
                                    if urgencia == ytm_audio::Urgencia::Precarga {
                                        tracing::debug!(
                                            video_id = %video_id,
                                            error = %e,
                                            "yt-dlp fallo en una precarga; no se acuna"
                                        );
                                        return Err(e);
                                    }
                                    tracing::warn!(error = %e, "yt-dlp fallo; se intenta el acunador");
                                }
                            }
                        }
                        tracing::warn!(video_id = %video_id, "acunando la URL en un webview oculto");
                        let url = minter::mint(&h, &video_id).await?;
                        tracing::info!(video_id = %video_id, "URL acunada");
                        Ok(ytm_audio::Provided::Url { url, size: None, mime: None })
                    })
                },
            );

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
            engine.send(Command::SetCacheLimit(limite_cache(&database)));
            // Igualar volumen viene puesto: hay casi 6 dB entre unas pistas y
            // otras, y quien no lo quiera lo apaga en Ajustes.
            reparar_metadatos(app.handle().clone(), Arc::clone(&database), innertube.clone());

            engine.send(Command::SetNormalize(
                database.get_setting("normalize").as_deref() != Some("false"),
            ));

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
                // La pista actual se guardo sin nombre y hay que corregirla si llega.
                let mut titulo_pendiente = false;
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
                            titulo_pendiente = ytm_audio::falta_el_titulo(&t.title);
                            let _ = db_for_task.push_history(&db::SavedTrack {
                                video_id: t.video_id.clone(),
                                title: t.title.clone(),
                                author: t.author.clone(),
                                thumbnail: t.thumbnail.clone(),
                                at: 0,
                            });
                        } else if titulo_pendiente && !ytm_audio::falta_el_titulo(&t.title) {
                            // El nombre ha llegado despues de guardar la fila.
                            // Sin esto, la pista queda en el historial como
                            // "Sin titulo" aunque en pantalla ya se lea bien.
                            titulo_pendiente = false;
                            let _ = db_for_task.fill_track_meta(
                                &t.video_id,
                                &t.title,
                                &t.author,
                                t.thumbnail.as_deref(),
                            );
                            let _ = handle.emit("biblioteca", ());
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

            // La actualizacion se comprueba y se descarga sola, con margen.
            // Ver `actualizacion.rs`.
            app.manage(actualizacion::EstadoActualizacion::default());
            actualizacion::comprobar_en_segundo_plano(app.handle().clone());

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
            browse_more,
            search_suggestions,
            radio,
            set_up_next,
            play_next,
            enqueue,
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
            storage_info,
            set_cache_limit,
            set_normalize,
            settings,
            clear_cache,
            app_version,
            create_playlist,
            rename_playlist,
            delete_playlist,
            playlists,
            playlist_tracks,
            add_to_playlist,
            remove_from_playlist,
            diagnose,
            extractor_status,
            window_minimize,
            window_toggle_maximize,
            window_close,
            actualizacion::update_pending,
            actualizacion::update_check_now,
            actualizacion::update_install,
        ])
        .run(tauri::generate_context!())
        .expect("error al arrancar la aplicacion");
}
