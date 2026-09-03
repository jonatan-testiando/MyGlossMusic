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
pub mod queue;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub use queue::{Queue, Repeat};
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
    pub fn start() -> Result<Self> {
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
        self.armed.store(false, Ordering::Release);
        self.seek_base = Duration::ZERO;
        self.player.clear();
        self.publish();

        let resolved = ytm_source::resolve(&self.innertube, &track.video_id)
            .await
            .with_context(|| format!("no se pudo resolver {}", track.video_id))?;

        // Los metadatos reales solo se conocen tras resolver; la cola pudo
        // haberse construido con un id pelado.
        if track.title.is_none() {
            self.queue.set_current_meta(resolved.track.clone());
        }
        self.current_duration =
            Duration::from_millis(resolved.audio.duration_ms.unwrap_or(0));

        let path = cache::track_path(&track.video_id, resolved.audio.itag);
        let cache = cache::TrackCache::start(
            resolved.audio.url.clone(),
            resolved.audio.size_bytes,
            path,
            self.http.clone(),
        )?;

        let reader = {
            let cache = cache.clone();
            // La apertura espera al primer byte, asi que fuera del hilo async.
            tokio::task::spawn_blocking(move || cache.reader()).await??
        };

        let source = rodio::Decoder::try_from(std::io::BufReader::new(reader))
            .context("no se pudo decodificar el audio")?;

        self.player.clear();
        self.player.set_volume(self.volume);
        self.player.append(source);
        self.player.play();

        self.current_cache = Some(cache);
        self.loading = false;
        self.armed.store(true, Ordering::Release);

        // Precarga de la siguiente para que la transicion no tenga hueco.
        self.prefetch_next();
        Ok(())
    }

    /// Descarga la siguiente pista en segundo plano. Los fallos se ignoran a
    /// proposito: es una optimizacion, no una funcionalidad.
    fn prefetch_next(&self) {
        let Some(next) = self.queue.peek_next().cloned() else {
            return;
        };
        if Some(&next.video_id) == self.queue.current().map(|c| &c.video_id) {
            return; // Repeat::One: ya esta en cache
        }
        let it = self.innertube.clone();
        let http = self.http.clone();
        tokio::spawn(async move {
            match ytm_source::resolve(&it, &next.video_id).await {
                Ok(r) => {
                    let path = cache::track_path(&next.video_id, r.audio.itag);
                    if let Err(e) =
                        cache::TrackCache::start(r.audio.url, r.audio.size_bytes, path, http)
                    {
                        tracing::debug!(error = %e, "precarga fallida");
                    }
                }
                Err(e) => tracing::debug!(error = %e, "precarga: no se pudo resolver"),
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
                self.error = Some(e);
                self.armed.store(false, Ordering::Release);
                return;
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
