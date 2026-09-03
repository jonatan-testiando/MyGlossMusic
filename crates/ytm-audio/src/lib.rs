//! Motor de audio: cola, cache, reproduccion y estado observable.
//!
//! # Modelo de hilos
//!
//! - Un **hilo de audio** dedicado, que solo existe para mantener vivo el
//!   `MixerDeviceSink` de cpal (no es `Send`, no puede cruzar hilos).
//! - El `Player` de rodio si se comparte por `Arc`, asi que el control
//!   (play/pause/seek/volumen) se hace desde donde haga falta.
//! - Una **tarea tokio** procesa comandos y publica estado por un canal `watch`,
//!   de modo que la interfaz se entera de los cambios sin sondear.

pub mod cache;
pub mod decode;
pub mod queue;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use cache::BoxDone;
pub use queue::{Queue, Repeat};

/// Como llega el audio de una pista.
pub enum Provided {
    /// Una URL que descargamos nosotros.
    Url {
        url: String,
        size: Option<u64>,
        mime: Option<String>,
    },
    /// Un proceso externo (yt-dlp) esta escribiendo `path`; `done` resuelve al
    /// terminar. Los metadatos son opcionales: sirven de respaldo si InnerTube
    /// no pudo resolver la pista.
    External {
        path: PathBuf,
        size: Option<u64>,
        mime: Option<String>,
        done: BoxDone,
        title: Option<String>,
        author: Option<String>,
        thumbnail: Option<String>,
        duration_ms: Option<u64>,
    },
}

/// Proveedor de fuente de audio. Lo inyecta el anfitrion (la app Tauri): acunar
/// una URL exige un webview y lanzar yt-dlp exige saber donde esta, y este crate
/// no debe saber nada de eso. Recibe el id del video y el directorio de cache.
pub type SourceProvider = Arc<
    dyn Fn(String, PathBuf) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Provided>> + Send>>
        + Send
        + Sync,
>;

/// Una pista lista (o en curso) para reproducir.
struct Prepared {
    cache: cache::TrackCache,
    itag: u32,
    mime: Option<String>,
    meta: Option<TrackInfo>,
    duration_ms: Option<u64>,
}
use ytm_source::{InnerTube, TrackInfo};

/// Cadencia de publicacion de estado. 100 ms basta para que una barra de
/// progreso se vea fluida sin despertar la interfaz sin motivo.
const TICK: Duration = Duration::from_millis(100);

/// Ordenes que acepta el motor.
#[derive(Debug, Clone)]
pub enum Command {
    /// Sustituye la cola y empieza a reproducir en `start`.
    SetQueue { tracks: Vec<TrackInfo>, start: usize },
    /// Reproduce un video suelto, sustituyendo la cola.
    PlayNow(String),
    /// Anade al final de la cola.
    Enqueue(TrackInfo),
    /// Salta a una posicion de la cola.
    JumpTo(usize),
    TogglePlay,
    Pause,
    Resume,
    Next,
    Prev,
    Seek(Duration),
    SetVolume(f32),
    SetRepeat(Repeat),
    SetShuffle(bool),
    Stop,
}

/// Estado observable por la interfaz.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    pub track: Option<Track>,
    pub playing: bool,
    pub loading: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
    /// Fraccion de la pista ya descargada, 0.0 a 1.0.
    pub buffered: f32,
    pub queue: Vec<Track>,
    pub queue_index: usize,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub error: Option<String>,
}

/// Version serializable de [`TrackInfo`], para cruzar a la interfaz.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub video_id: String,
    pub title: String,
    pub author: String,
    pub thumbnail: Option<String>,
}

impl From<&TrackInfo> for Track {
    fn from(t: &TrackInfo) -> Self {
        Self {
            video_id: t.video_id.clone(),
            title: t.title.clone().unwrap_or_else(|| "Sin titulo".into()),
            author: t.author.clone().unwrap_or_else(|| "Desconocido".into()),
            thumbnail: t.thumbnail.clone(),
        }
    }
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            track: None,
            playing: false,
            loading: false,
            position_ms: 0,
            duration_ms: 0,
            volume: 1.0,
            buffered: 0.0,
            queue: Vec::new(),
            queue_index: 0,
            repeat: Repeat::Off,
            shuffle: false,
            error: None,
        }
    }
}

/// Mando a distancia del motor. Barato de clonar.
#[derive(Clone)]
pub struct Engine {
    tx: tokio::sync::mpsc::UnboundedSender<Command>,
    state: tokio::sync::watch::Receiver<PlaybackState>,
}

impl Engine {
    /// Arranca el motor. Debe llamarse dentro de un runtime tokio.
    ///
    /// `source_provider` trae el audio de cada pista (yt-dlp, acunador...). Sin
    /// el, se usa la URL de InnerTube, que googlevideo corta a ~1 MiB (~65 s).
    pub fn start(source_provider: Option<SourceProvider>) -> Result<Self> {
        let player = spawn_audio_thread().context("no se pudo iniciar la salida de audio")?;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (state_tx, state_rx) = tokio::sync::watch::channel(PlaybackState::default());

        let inner = Inner {
            player,
            http: reqwest::Client::new(),
            innertube: InnerTube::new()?,
            queue: Queue::default(),
            current_cache: None,
            volume: 1.0,
            // Guarda de arranque: `player.empty()` es cierto durante unos
            // milisegundos justo despues de encolar. Sin esto, cada pista se
            // saltaria sola nada mas empezar.
            armed: Arc::new(AtomicBool::new(false)),
            seek_base: Duration::ZERO,
            current_duration: Duration::ZERO,
            state: state_tx,
            error: None,
            loading: false,
            source_provider,
            prepared: Arc::new(Mutex::new(HashMap::new())),
            partial_warned: false,
        };

        tokio::spawn(inner.run(rx));

        Ok(Self { tx, state: state_rx })
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    pub fn state(&self) -> PlaybackState {
        self.state.borrow().clone()
    }

    /// Canal para reaccionar a cambios de estado sin sondear.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<PlaybackState> {
        self.state.clone()
    }
}

/// Obtiene la fuente de audio de una pista: proveedor si lo hay, InnerTube si no.
async fn prepare(
    provider: Option<SourceProvider>,
    http: reqwest::Client,
    video_id: &str,
    fallback: Option<&ytm_source::AudioStream>,
) -> Result<Prepared> {
    if let Some(p) = provider {
        match p(video_id.to_string(), cache::cache_dir()).await {
            Ok(Provided::Url { url, size, mime }) => {
                let itag = param(&url, "itag")
                    .and_then(|v| v.parse().ok())
                    .or(fallback.map(|f| f.itag))
                    .unwrap_or(0);
                let mime = mime.or_else(|| param(&url, "mime").map(|m| m.replace("%2F", "/")));
                let size = size.or_else(|| param(&url, "clen").and_then(|v| v.parse().ok()));
                let path = cache::track_path(video_id, itag);
                let cache = cache::TrackCache::start(url, size, path, http)?;
                return Ok(Prepared { cache, itag, mime, meta: None, duration_ms: None });
            }
            Ok(Provided::External { path, size, mime, done, title, author, thumbnail, duration_ms }) => {
                let cache = cache::TrackCache::start_external(path, size, done)?;
                let meta = title.is_some().then(|| TrackInfo {
                    video_id: video_id.to_string(),
                    title,
                    author,
                    thumbnail,
                });
                return Ok(Prepared {
                    cache,
                    itag: fallback.map(|f| f.itag).unwrap_or(0),
                    mime,
                    meta,
                    duration_ms,
                });
            }
            Err(e) => tracing::warn!(error = %e, "el proveedor fallo; se usa InnerTube"),
        }
    }
    let f = fallback.context("sin proveedor de audio y sin URL de InnerTube")?;
    let path = cache::track_path(video_id, f.itag);
    let cache = cache::TrackCache::start(f.url.clone(), f.size_bytes, path, http)?;
    Ok(Prepared { cache, itag: f.itag, mime: None, meta: None, duration_ms: None })
}

/// Lee un parametro de una URL de googlevideo.
fn param(url: &str, key: &str) -> Option<String> {
    url.split(['?', '&'])
        .find_map(|kv| kv.strip_prefix(&format!("{key}="))) 
        .map(|v| v.to_string())
}

/// Crea la salida de audio y devuelve el `Player` compartible.
///
/// El hilo se queda aparcado para siempre a proposito: su unica funcion es ser
/// duena del `MixerDeviceSink`, que al soltarse cerraria el dispositivo.
fn spawn_audio_thread() -> Result<Arc<rodio::Player>> {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::Builder::new()
        .name("ytm-audio-out".into())
        .spawn(move || match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(device) => {
                let player = Arc::new(rodio::Player::connect_new(device.mixer()));
                if tx.send(Ok(player)).is_err() {
                    return;
                }
                loop {
                    std::thread::park();
                }
            }
            Err(e) => {
                let _ = tx.send(Err(anyhow::anyhow!("{e}")));
            }
        })
        .context("no se pudo crear el hilo de audio")?;

    rx.recv().context("el hilo de audio no respondio")?
}

struct Inner {
    player: Arc<rodio::Player>,
    http: reqwest::Client,
    innertube: InnerTube,
    queue: Queue,
    current_cache: Option<cache::TrackCache>,
    volume: f32,
    armed: Arc<AtomicBool>,
    /// Posicion desde la que se reanudo tras un seek: `player.get_pos()` cuenta
    /// desde el inicio del `Source` encolado, no desde el inicio de la pista.
    seek_base: Duration,
    current_duration: Duration,
    state: tokio::sync::watch::Sender<PlaybackState>,
    error: Option<String>,
    loading: bool,
    source_provider: Option<SourceProvider>,
    /// Pistas precargadas, por id. Reproducir una que ya esta aqui reutiliza la
    /// descarga en curso en vez de abrir otra sobre el mismo archivo.
    prepared: Arc<Mutex<HashMap<String, Prepared>>>,
    /// Ya se aviso de que esta pista quedo incompleta.
    partial_warned: bool,
}

impl Inner {
    async fn run(mut self, mut rx: tokio::sync::mpsc::UnboundedReceiver<Command>) {
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                Some(cmd) = rx.recv() => {
                    if let Err(e) = self.handle(cmd).await {
                        tracing::warn!(error = %e, "comando fallido");
                        self.error = Some(e.to_string());
                    }
                    self.publish();
                }
                _ = ticker.tick() => {
                    self.on_tick().await;
                    self.publish();
                }
                else => break,
            }
        }
    }

    async fn handle(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::SetQueue { tracks, start } => {
                self.queue.set_items(tracks, start);
                self.start_current().await?;
            }
            Command::PlayNow(video_id) => {
                let track = TrackInfo {
                    video_id,
                    title: None,
                    author: None,
                    thumbnail: None,
                };
                self.queue.set_items(vec![track], 0);
                self.start_current().await?;
            }
            Command::Enqueue(t) => {
                let was_empty = self.queue.is_empty();
                self.queue.push(t);
                if was_empty {
                    self.start_current().await?;
                }
            }
            Command::JumpTo(i) => {
                if self.queue.jump_to(i).is_some() {
                    self.start_current().await?;
                }
            }
            Command::TogglePlay => {
                if self.player.is_paused() {
                    self.player.play();
                } else {
                    self.player.pause();
                }
            }
            Command::Pause => self.player.pause(),
            Command::Resume => self.player.play(),
            Command::Next => {
                if self.queue.next(false).is_some() {
                    self.start_current().await?;
                } else {
                    self.stop();
                }
            }
            Command::Prev => {
                // Convencion universal: si ya sonaron mas de 3 s, "anterior"
                // reinicia la pista actual en vez de cambiar de cancion.
                if self.position() > Duration::from_secs(3) {
                    self.seek(Duration::ZERO).await?;
                } else if self.queue.prev().is_some() {
                    self.start_current().await?;
                }
            }
            Command::Seek(pos) => self.seek(pos).await?,
            Command::SetVolume(v) => {
                self.volume = v.clamp(0.0, 2.0);
                self.player.set_volume(self.volume);
            }
            Command::SetRepeat(r) => self.queue.set_repeat(r),
            Command::SetShuffle(on) => self.queue.set_shuffle(on),
            Command::Stop => self.stop(),
        }
        Ok(())
    }

    /// Resuelve, cachea y empieza a reproducir la pista actual de la cola.
    async fn start_current(&mut self) -> Result<()> {
        let Some(track) = self.queue.current().cloned() else {
            return Ok(());
        };

        self.error = None;
        self.loading = true;
        self.partial_warned = false;
        self.armed.store(false, Ordering::Release);
        self.seek_base = Duration::ZERO;
        self.player.clear();
        self.publish();

        // InnerTube da titulo, portada y duracion. Si falla y hay proveedor, no
        // es fatal: yt-dlp trae sus propios metadatos.
        let resolved = match ytm_source::resolve(&self.innertube, &track.video_id).await {
            Ok(r) => Some(r),
            Err(e) if self.source_provider.is_some() => {
                tracing::warn!(error = %e, "InnerTube no resolvio; se sigue con el proveedor");
                None
            }
            Err(e) => return Err(e.context(format!("no se pudo resolver {}", track.video_id))),
        };

        // Si la precarga ya la dejo lista (o en curso), se reutiliza: arrancar
        // una segunda descarga sobre el mismo archivo lo corromperia.
        let already = self.prepared.lock().unwrap().remove(&track.video_id);
        let prepared = match already {
            Some(p) if p.cache.error().is_none() => p,
            _ => {
                prepare(
                    self.source_provider.clone(),
                    self.http.clone(),
                    &track.video_id,
                    resolved.as_ref().map(|r| &r.audio),
                )
                .await?
            }
        };

        // Metadatos: InnerTube primero, yt-dlp de respaldo.
        if track.title.is_none() {
            if let Some(r) = &resolved {
                self.queue.set_current_meta(r.track.clone());
            } else if let Some(m) = &prepared.meta {
                self.queue.set_current_meta(m.clone());
            }
        }
        self.current_duration = Duration::from_millis(
            resolved
                .as_ref()
                .and_then(|r| r.audio.duration_ms)
                .or(prepared.duration_ms)
                .unwrap_or(0),
        );

        let reader = {
            let c = prepared.cache.clone();
            tokio::task::spawn_blocking(move || c.reader()).await??
        };
        let ext = decode::extension_for(
            prepared.itag,
            prepared.mime.as_deref(),
            resolved.as_ref().and_then(|r| r.audio.codec.as_deref()),
        );
        let source = decode::SymphoniaSource::new(reader, ext)
            .map_err(|e| anyhow::anyhow!("no se pudo decodificar ({ext}): {e:#}"))?;

        self.player.clear();
        self.player.set_volume(self.volume);
        self.player.append(source);
        self.player.play();

        self.current_cache = Some(prepared.cache);
        self.loading = false;
        self.armed.store(true, Ordering::Release);

        self.prefetch_next();
        Ok(())
    }

    /// Deja lista la siguiente pista en segundo plano. Los fallos se ignoran a
    /// proposito: es una optimizacion, no una funcionalidad.
    fn prefetch_next(&self) {
        let Some(next) = self.queue.peek_next().cloned() else {
            return;
        };
        if Some(&next.video_id) == self.queue.current().map(|c| &c.video_id) {
            return; // Repeat::One: ya esta en cache
        }
        if self.prepared.lock().unwrap().contains_key(&next.video_id) {
            return;
        }
        let it = self.innertube.clone();
        let http = self.http.clone();
        let provider = self.source_provider.clone();
        let prepared = Arc::clone(&self.prepared);
        tokio::spawn(async move {
            let resolved = ytm_source::resolve(&it, &next.video_id).await.ok();
            match prepare(provider, http, &next.video_id, resolved.as_ref().map(|r| &r.audio)).await {
                Ok(p) => {
                    prepared.lock().unwrap().insert(next.video_id.clone(), p);
                }
                Err(e) => tracing::debug!(error = %e, "precarga fallida"),
            }
        });
    }

    async fn seek(&mut self, pos: Duration) -> Result<()> {
        match self.player.try_seek(pos) {
            Ok(()) => {
                self.seek_base = Duration::ZERO;
                Ok(())
            }
            Err(e) => {
                tracing::debug!(error = %e, "seek no soportado por la fuente");
                Err(anyhow::anyhow!("no se pudo saltar a esa posicion"))
            }
        }
    }

    fn stop(&mut self) {
        self.player.clear();
        self.armed.store(false, Ordering::Release);
        self.current_cache = None;
        self.seek_base = Duration::ZERO;
        self.current_duration = Duration::ZERO;
    }

    fn position(&self) -> Duration {
        self.player.get_pos() + self.seek_base
    }

    /// Detecta el fin natural de una pista y encadena la siguiente.
    async fn on_tick(&mut self) {
        if !self.armed.load(Ordering::Acquire) {
            return;
        }
        if let Some(c) = &self.current_cache {
            if let Some(e) = c.error() {
                // Sin un solo byte, es un fallo. Con bytes, se reproduce lo que
                // hay: un corte de red a mitad no debe parar la musica.
                if c.downloaded_bytes() == 0 {
                    self.error = Some(e);
                    self.armed.store(false, Ordering::Release);
                    return;
                }
                if !self.partial_warned {
                    self.partial_warned = true;
                    tracing::warn!(error = %e, "descarga incompleta: se reproduce lo que hay");
                }
            }
        }
        if self.player.empty() && !self.player.is_paused() {
            self.armed.store(false, Ordering::Release);
            if self.queue.next(true).is_some() {
                if let Err(e) = self.start_current().await {
                    tracing::warn!(error = %e, "no se pudo encadenar la siguiente");
                    self.error = Some(e.to_string());
                }
            }
        }
    }

    fn publish(&self) {
        let pos = self.position();
        let state = PlaybackState {
            track: self.queue.current().map(Track::from),
            playing: self.armed.load(Ordering::Acquire) && !self.player.is_paused(),
            loading: self.loading,
            position_ms: pos.as_millis() as u64,
            duration_ms: self.current_duration.as_millis() as u64,
            volume: self.volume,
            buffered: self.current_cache.as_ref().map_or(0.0, |c| c.progress()),
            queue: self.queue.items().iter().map(Track::from).collect(),
            queue_index: self.queue.index(),
            repeat: self.queue.repeat(),
            shuffle: self.queue.shuffle(),
            error: self.error.clone(),
        };
        let _ = self.state.send(state);
    }
}
