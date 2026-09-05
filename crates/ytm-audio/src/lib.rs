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

pub use cache::{cache_dir, BoxDone};
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
    /// Lo que devolvio InnerTube para esta pista, si se llego a pedir.
    ///
    /// La precarga ya paga esa peticion, asi que guardarla aqui hace que
    /// arrancar una pista precargada no toque la red **en absoluto**. Antes se
    /// volvia a resolver aunque la descarga estuviera hecha: ~760 ms de espera
    /// en cada cambio de cancion, tambien en el automatico.
    resolved: Option<ytm_source::Resolved>,
}
use ytm_source::{InnerTube, TrackInfo};

/// Cuantas pistas se dejan listas por delante de la que suena.
///
/// Dos y no una: con una sola, dos "siguiente" seguidos —o un salto justo
/// despues de un cambio automatico— vuelven a pagar el arranque en frio. Subir
/// mas tampoco sale gratis: cada precarga es un proceso de yt-dlp y una pista
/// entera bajada a disco.
const PRECARGA: usize = 2;

/// Registro de precargas: lo que ya esta listo y lo que se esta trayendo.
///
/// Los dos campos van bajo el MISMO candado a proposito, y `en_curso` guarda la
/// tarea, no solo el id, para poder ESPERARLA.
///
/// # Que evita esto
///
/// Que dos yt-dlp escriban el mismo archivo. Pasa por dos caminos: dos
/// precargas de la misma pista, y —el peligroso— saltar a una pista que se
/// esta precargando justo en ese momento. En los dos casos el segundo yt-dlp
/// arranca borrando los restos `<id>.*` del primero (`purge_partial`, en
/// `ytm-source/src/ytdlp.rs`). Con `--no-part` los dos apuntan al mismo
/// nombre, y en Windows eso ni siquiera falla limpio: da un error de comparticion
/// y la pista acaba cayendo al acunador, con sus 25 s de espera.
///
/// Antes el riesgo era pequeno porque la precarga solo se disparaba al arrancar
/// una pista. Ahora se rearma tambien al cambiar el aleatorio o la cola, y son
/// dos pistas por delante en vez de una, asi que hay que cerrarlo de verdad.
#[derive(Default)]
struct Precargas {
    listas: HashMap<String, Prepared>,
    en_curso: HashMap<String, tokio::task::JoinHandle<()>>,
}

/// Factor de volumen que iguala una pista con las demas.
///
/// # De donde sale el numero
///
/// No hay que medir nada: YouTube ya trae la medicion hecha. En la respuesta
/// del reproductor, `playerConfig.audioConfig` da el volumen absoluto de la
/// pista y el objetivo al que se normaliza, y cada formato trae ya la resta
/// —eso es `loudness_db`—, es decir, cuantos dB se pasa la pista del objetivo.
/// Comprobado sobre cuatro pistas reales:
///
/// | pista                    | absoluto   | objetivo   | `loudness_db` |
/// |--------------------------|-----------:|-----------:|--------------:|
/// | Never Gonna Give You Up  | -13,01 LKFS| -14 LKFS   |      0,99 dB  |
/// | Blinding Lights          | -10,59     | -14        |      3,41     |
/// | Fool For You             |  -7,25     | -14        |      6,75     |
///
/// Casi 6 dB entre la primera y la ultima: eso es lo que obliga a tocar la
/// rueda de volumen en cada cambio de cancion.
///
/// # El tope
///
/// De dB a factor lineal es `10^(-dB/20)`. Se acota a [0,25, 2,0] por una razon
/// concreta: subir el volumen de una pista que ya venia baja puede recortar los
/// picos, y aqui no hay limitador que lo recoja. Dos es el maximo que se
/// permite subir; por debajo, bajar nunca distorsiona, y el tope de 0,25 solo
/// esta para que un dato absurdo no deje la cancion muda.
///
/// Sin dato, factor 1: no tocar nada es siempre mejor que adivinar.
fn ganancia_de(loudness_db: Option<f32>) -> f32 {
    let Some(db) = loudness_db.filter(|d| d.is_finite()) else {
        return 1.0;
    };
    10f32.powf(-db / 20.0).clamp(0.25, 2.0)
}

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
    /// Sustituye lo que viene despues de la pista actual, sin reiniciarla.
    SetUpNext { tracks: Vec<TrackInfo> },
    /// Cuela una pista justo despues de la actual, conservando el resto.
    PlayNext(TrackInfo),
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
    /// Cuanto puede ocupar la cache en disco, en bytes. 0 es sin limite.
    SetCacheLimit(u64),
    /// Iguala el volumen entre pistas. Ver [`ganancia_de`].
    SetNormalize(bool),
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
    /// Sube cada vez que el CONTENIDO de la cola cambia. Permite al anfitrion
    /// no reenviar la cola entera (cientos de pistas con una playlist) diez
    /// veces por segundo: solo cuando esta revision cambia.
    pub queue_rev: u64,
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
    /// Duracion conocida de la pista, si la hay. La interfaz pinta un hueco
    /// mientras sea `None` en vez de inventarse un numero.
    pub duration_ms: Option<u64>,
}

/// Lo que se pinta mientras no se sabe como se llama la pista.
///
/// Son constantes y no literales sueltos porque hay que poder RECONOCERLOS
/// despues: el historial guarda el titulo tal cual, y sin poder distinguir
/// "no lo sabemos" de un nombre de verdad, una pista que se guardo antes de
/// que llegaran sus metadatos se queda asi para siempre.
pub const SIN_TITULO: &str = "Sin titulo";
pub const SIN_AUTOR: &str = "Desconocido";

/// `true` si el titulo es el de relleno y merece un segundo intento.
pub fn falta_el_titulo(title: &str) -> bool {
    title.is_empty() || title == SIN_TITULO
}

impl From<&TrackInfo> for Track {
    fn from(t: &TrackInfo) -> Self {
        Self {
            video_id: t.video_id.clone(),
            title: t.title.clone().unwrap_or_else(|| SIN_TITULO.into()),
            author: t.author.clone().unwrap_or_else(|| SIN_AUTOR.into()),
            thumbnail: t.thumbnail.clone(),
            duration_ms: t.duration_ms,
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
            queue_rev: 0,
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
            cache_limit: cache::DEFAULT_LIMIT,
            normalizar: true,
            ganancia: 1.0,
            prepared: Arc::new(Mutex::new(Precargas::default())),
            partial_warned: false,
            queue_rev: 0,
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

/// Fuente de audio a traves del proveedor del anfitrion (yt-dlp o el acunador).
///
/// # Por que ya no recibe la respuesta de InnerTube
///
/// Antes esta funcion tomaba el stream de InnerTube como respaldo, lo que
/// obligaba a resolver ANTES de llamarla. Pero cuando el proveedor funciona —el
/// caso normal— ese stream no se usa para nada del audio: solo aportaba el
/// `itag`, y el `itag` es el ultimo criterio de [`decode::extension_for`], por
/// detras del mime, que yt-dlp siempre da. Separarlas permite lanzar las dos
/// peticiones a la vez. El respaldo vive ahora en el sitio que decide,
/// [`preparar`].
async fn prepare_provider(
    provider: &SourceProvider,
    http: reqwest::Client,
    video_id: &str,
) -> Result<Prepared> {
    match provider(video_id.to_string(), cache::cache_dir()).await? {
        Provided::Url { url, size, mime } => {
            let itag = param(&url, "itag").and_then(|v| v.parse().ok()).unwrap_or(0);
            let mime = mime.or_else(|| param(&url, "mime").map(|m| m.replace("%2F", "/")));
            let size = size.or_else(|| param(&url, "clen").and_then(|v| v.parse().ok()));
            let path = cache::track_path(video_id, itag);
            let cache = cache::TrackCache::start(url, size, path, http)?;
            Ok(Prepared { cache, itag, mime, meta: None, duration_ms: None, resolved: None })
        }
        Provided::External { path, size, mime, done, title, author, thumbnail, duration_ms } => {
            let cache = cache::TrackCache::start_external(path, size, done)?;
            let meta = title.is_some().then(|| TrackInfo {
                video_id: video_id.to_string(),
                title,
                author,
                thumbnail,
                duration_ms,
            });
            Ok(Prepared { cache, itag: 0, mime, meta, duration_ms, resolved: None })
        }
    }
}

/// Fuente de audio a partir de la URL directa que dio InnerTube.
fn prepare_stream(
    http: reqwest::Client,
    video_id: &str,
    f: &ytm_source::AudioStream,
) -> Result<Prepared> {
    let path = cache::track_path(video_id, f.itag);
    let cache = cache::TrackCache::start(f.url.clone(), f.size_bytes, path, http)?;
    Ok(Prepared { cache, itag: f.itag, mime: None, meta: None, duration_ms: None, resolved: None })
}

/// Deja lista una pista: resuelve y consigue el audio **a la vez**.
///
/// # Por que en paralelo
///
/// Son dos esperas independientes que antes iban una detras de otra. Medido el
/// 2026-09-05: la cascada de InnerTube tarda ~240 ms y yt-dlp entre 1,5 y 2,6 s
/// en decidir formato y ruta. En serie eso es la suma; a la vez, el maximo. La
/// respuesta de InnerTube llega de sobra antes de que haya un solo byte que
/// decodificar, asi que no se pierde ni el volumen normalizado ni la duracion.
///
/// InnerTube se pide SIEMPRE aunque el proveedor vaya a ganar, porque es lo
/// unico que trae `loudness_db`, y sin el no hay normalizacion de volumen.
///
/// Devuelve tambien lo que dijo InnerTube: `None` si fallo (no es fatal
/// mientras el proveedor entregue audio).
async fn preparar(
    provider: Option<&SourceProvider>,
    innertube: &InnerTube,
    http: reqwest::Client,
    video_id: &str,
) -> Result<Prepared> {
    let Some(provider) = provider else {
        // Sin proveedor, InnerTube es la unica fuente y no hay nada que
        // paralelizar.
        let r = ytm_source::resolve(innertube, video_id).await?;
        let mut prep = prepare_stream(http, video_id, &r.audio)?;
        prep.resolved = Some(r);
        return Ok(prep);
    };

    let (resuelto, del_proveedor) = tokio::join!(
        ytm_source::resolve(innertube, video_id),
        prepare_provider(provider, http.clone(), video_id),
    );

    let resuelto = match resuelto {
        Ok(r) => Some(r),
        Err(e) => {
            tracing::warn!(error = %e, "InnerTube no resolvio; se sigue con el proveedor");
            None
        }
    };

    match del_proveedor {
        Ok(mut prep) => {
            // El `itag` de InnerTube, si el proveedor no traia uno. Es el
            // ultimo criterio de [`decode::extension_for`], pero es gratis
            // conservarlo ahora que la respuesta ya esta aqui.
            if prep.itag == 0 {
                if let Some(r) = &resuelto {
                    prep.itag = r.audio.itag;
                }
            }
            prep.resolved = resuelto;
            Ok(prep)
        }
        Err(e) => {
            tracing::warn!(error = %e, "el proveedor fallo; se usa InnerTube");
            let r = resuelto.context("sin proveedor de audio y sin URL de InnerTube")?;
            let mut prep = prepare_stream(http, video_id, &r.audio)?;
            prep.resolved = Some(r);
            Ok(prep)
        }
    }
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
    /// Tope de la cache en disco. Ver [`cache::prune`].
    cache_limit: u64,
    /// Igualar el volumen entre pistas.
    normalizar: bool,
    /// Correccion de la pista actual, ya en factor lineal. Ver [`ganancia_de`].
    ganancia: f32,
    /// Pistas precargadas y en curso de precarga. Reproducir una que ya esta
    /// aqui reutiliza la descarga en vez de abrir otra sobre el mismo archivo.
    prepared: Arc<Mutex<Precargas>>,
    /// Ya se aviso de que esta pista quedo incompleta.
    partial_warned: bool,
    /// Revision del contenido de la cola (ver [`PlaybackState::queue_rev`]).
    queue_rev: u64,
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
                self.queue_rev += 1;
                self.queue.set_items(tracks, start);
                self.start_current().await?;
            }
            Command::PlayNow(video_id) => {
                let track = TrackInfo {
                    video_id,
                    title: None,
                    author: None,
                    thumbnail: None,
                    duration_ms: None,
                };
                self.queue_rev += 1;
                self.queue.set_items(vec![track], 0);
                self.start_current().await?;
            }
            Command::SetUpNext { tracks } => {
                // No se llama a `start_current()`: la pista actual sigue
                // sonando. Solo cambia lo que viene detras — y por eso hay que
                // rearmar la precarga: la que hubiera ya no es la siguiente.
                self.queue_rev += 1;
                self.queue.set_up_next(tracks);
                self.prefetch_ahead();
                self.publish();
            }
            Command::PlayNext(t) => {
                let vacia = self.queue.is_empty();
                self.queue_rev += 1;
                self.queue.play_next(t);
                // Con la cola vacia no hay "a continuacion" que valga: suena.
                if vacia {
                    self.start_current().await?;
                } else {
                    self.prefetch_ahead();
                    self.publish();
                }
            }
            Command::Enqueue(t) => {
                let was_empty = self.queue.is_empty();
                self.queue_rev += 1;
                self.queue.push(t);
                if was_empty {
                    self.start_current().await?;
                } else {
                    // Con la cola agotada, lo que se acaba de anadir ES la
                    // siguiente: sin esto sonaria en frio.
                    self.prefetch_ahead();
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
                self.aplicar_volumen();
            }
            Command::SetRepeat(r) => {
                // Cambia cual es la siguiente al final de la cola, y `One` la
                // convierte en la actual.
                self.queue.set_repeat(r);
                self.prefetch_ahead();
            }
            Command::SetShuffle(on) => {
                // Rebaraja: la pista precargada deja de ser la siguiente.
                self.queue.set_shuffle(on);
                self.prefetch_ahead();
            }
            Command::SetCacheLimit(bytes) => {
                self.cache_limit = bytes;
                self.podar_cache();
            }
            Command::SetNormalize(on) => {
                self.normalizar = on;
                self.aplicar_volumen();
            }
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
        // Al arrancar una pista pueden llegarle metadatos nuevos a la cola.
        self.queue_rev += 1;
        self.armed.store(false, Ordering::Release);
        self.seek_base = Duration::ZERO;
        self.player.clear();
        self.publish();

        // Si esta pista se estaba precargando, se ESPERA a esa precarga en vez
        // de lanzar una segunda. Ver [`Precargas`]. Esperar no cuesta tiempo de
        // mas: el trabajo ya empezo antes, asi que solo puede acabar antes.
        let en_vuelo = self.prepared.lock().unwrap().en_curso.remove(&track.video_id);
        if let Some(tarea) = en_vuelo {
            let _ = tarea.await;
        }

        // Si la precarga ya la dejo lista (o en curso), se reutiliza: arrancar
        // una segunda descarga sobre el mismo archivo lo corromperia. Y como la
        // precarga guarda tambien la respuesta de InnerTube, este camino no
        // toca la red: es el cambio de cancion instantaneo.
        let already = self.prepared.lock().unwrap().listas.remove(&track.video_id);
        let prepared = match already {
            Some(p) if p.cache.error().is_none() => p,
            _ => preparar(
                self.source_provider.as_ref(),
                &self.innertube,
                self.http.clone(),
                &track.video_id,
            )
            .await
            .with_context(|| format!("no se pudo resolver {}", track.video_id))?,
        };
        let resolved = prepared.resolved.clone();

        // Metadatos: InnerTube primero, yt-dlp de respaldo.
        if track.title.is_none() {
            let backfilled = if let Some(r) = &resolved {
                self.queue.set_current_meta(r.track.clone());
                true
            } else if let Some(m) = &prepared.meta {
                self.queue.set_current_meta(m.clone());
                true
            } else {
                false
            };
            // Rellenar los metadatos cambia el CONTENIDO de la cola, asi que
            // hay que subir la revision. Sin esto la interfaz, que descarta la
            // cola cuando la revision se repite, se queda para siempre con el
            // "Sin titulo" con el que se creo la entrada.
            if backfilled {
                self.queue_rev += 1;
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

        self.ganancia = if self.normalizar {
            ganancia_de(resolved.as_ref().and_then(|r| r.audio.loudness_db))
        } else {
            1.0
        };

        self.player.clear();
        self.aplicar_volumen();
        self.player.append(source);
        self.player.play();

        self.current_cache = Some(prepared.cache);
        self.loading = false;
        self.armed.store(true, Ordering::Release);

        self.prefetch_ahead();
        self.podar_cache();
        self.rellenar_metadatos(&track.video_id).await;
        Ok(())
    }

    /// Ultimo recurso para el titulo, la caratula y la duracion.
    ///
    /// Se llama DESPUES de que suene, no antes: es una peticion mas y no tiene
    /// por que retrasar la reproduccion ni un milisegundo. Y solo cuando de
    /// verdad falta el titulo, que es el caso raro — cuando el plano de audio
    /// no pudo resolver la pista y el audio vino de yt-dlp o del acunador.
    ///
    /// Antes de esto, esas pistas sonaban con "Sin titulo / Desconocido" y sin
    /// caratula para siempre: no habia ninguna segunda oportunidad.
    async fn rellenar_metadatos(&mut self, video_id: &str) {
        if self.queue.current().is_some_and(|t| t.title.is_some()) {
            return;
        }
        match ytm_source::metadata(&self.innertube, video_id).await {
            Ok(info) => {
                tracing::info!(video_id, title = ?info.title, "metadatos recuperados aparte");
                if info.duration_ms.is_some() && self.current_duration.is_zero() {
                    self.current_duration = Duration::from_millis(info.duration_ms.unwrap());
                }
                self.queue.set_current_meta(info);
                // La cola cambia de contenido: sin subir la revision, la
                // interfaz la descarta por repetida y se queda el "Sin titulo".
                self.queue_rev += 1;
                self.publish();
            }
            Err(e) => tracing::warn!(video_id, error = %e, "tampoco hubo metadatos de respaldo"),
        }
    }

    /// Volumen del usuario por la correccion de la pista.
    fn aplicar_volumen(&self) {
        let g = if self.normalizar { self.ganancia } else { 1.0 };
        self.player.set_volume(self.volume * g);
    }

    /// Recorta la cache si se ha pasado del tope.
    ///
    /// Aqui y no al terminar cada descarga porque es el unico momento en que se
    /// sabe que archivo NO se puede borrar. La pista que se precarga no hace
    /// falta protegerla: acaba de crearse, y la poda empieza por la mas antigua.
    fn podar_cache(&self) {
        let limite = self.cache_limit;
        let protegidos: Vec<std::path::PathBuf> =
            self.current_cache.iter().map(|c| c.path().to_path_buf()).collect();
        // En un hilo aparte: recorrer el directorio y borrar es E/S de disco, y
        // el bucle del motor tiene que seguir atendiendo comandos.
        tokio::task::spawn_blocking(move || cache::prune(limite, &protegidos));
    }

    /// Deja listas en segundo plano las [`PRECARGA`] pistas que vienen. Los
    /// fallos se ignoran a proposito: es una optimizacion, no una funcionalidad.
    ///
    /// # Cuando hay que llamarla
    ///
    /// **Siempre que cambie cual es la siguiente pista**, no solo al arrancar
    /// una. Ese era el fallo: se llamaba unicamente al final de
    /// [`Self::start_current`], asi que al activar el aleatorio —que rebaraja
    /// la cola— la pista precargada dejaba de ser la que iba a sonar y nadie
    /// precargaba la nueva. Resultado: la primera cancion despues de darle a
    /// aleatorio arrancaba en frio, con los segundos completos de espera.
    fn prefetch_ahead(&self) {
        let actual = self.queue.current().map(|t| t.video_id.clone());
        let siguientes: Vec<String> = self
            .queue
            .peek_ahead(PRECARGA)
            .into_iter()
            .map(|t| t.video_id.clone())
            // Repeat::One: la siguiente es la que ya esta sonando.
            .filter(|id| Some(id) != actual.as_ref())
            .collect();

        let pendientes: Vec<String> = {
            let mut reg = self.prepared.lock().unwrap();
            // Lo que ya no va a sonar pronto sobra. Soltarlo no cancela nada:
            // la descarga corre en su propia tarea y termina en la cache de
            // disco igualmente, asi que si se vuelve a esa pista sigue barata.
            reg.listas.retain(|id, _| siguientes.contains(id));
            reg.en_curso.retain(|_, tarea| !tarea.is_finished());
            siguientes
                .into_iter()
                .filter(|id| !reg.listas.contains_key(id) && !reg.en_curso.contains_key(id))
                .collect()
        };

        for id in pendientes {
            let it = self.innertube.clone();
            let http = self.http.clone();
            let provider = self.source_provider.clone();
            let registro = Arc::clone(&self.prepared);
            let suya = id.clone();
            let tarea = tokio::spawn(async move {
                match preparar(provider.as_ref(), &it, http, &suya).await {
                    Ok(p) => {
                        registro.lock().unwrap().listas.insert(suya, p);
                    }
                    Err(e) => tracing::debug!(error = %e, "precarga fallida"),
                }
            });
            self.prepared.lock().unwrap().en_curso.insert(id, tarea);
        }
    }

    async fn seek(&mut self, pos: Duration) -> Result<()> {
        match self.player.try_seek(pos) {
            Ok(()) => {
                self.seek_base = Duration::ZERO;
                self.publish();
                Ok(())
            }
            Err(e) => {
                tracing::warn!(error = %e, ?pos, "seek no soportado o fallo");
                Err(anyhow::anyhow!("no se pudo saltar a esa posicion: {e}"))
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
            queue_rev: self.queue_rev,
            queue_index: self.queue.index(),
            repeat: self.queue.repeat(),
            shuffle: self.queue.shuffle(),
            error: self.error.clone(),
        };
        let _ = self.state.send(state);
    }
}

#[cfg(test)]
mod tests {
    use super::ganancia_de;

    /// Los tres numeros son medidas reales, no inventados. Ver `ganancia_de`.
    #[test]
    fn iguala_pistas_de_volumenes_distintos() {
        let suave = ganancia_de(Some(0.99)); // Never Gonna Give You Up
        let media = ganancia_de(Some(3.41)); // Blinding Lights
        let fuerte = ganancia_de(Some(6.75)); // Fool For You

        // A mas alta la pista, mas se la baja.
        assert!(fuerte < media && media < suave && suave < 1.0);

        // Y despues de corregir, las tres suenan al mismo nivel: la diferencia
        // entre ellas era de 5,76 dB y se queda por debajo de una decima.
        let nivel = |db: f32, g: f32| db + 20.0 * g.log10();
        assert!((nivel(0.99, suave) - nivel(6.75, fuerte)).abs() < 0.1);
    }

    #[test]
    fn sin_dato_no_se_toca_nada() {
        // Adivinar es peor que no hacer nada: una pista sin medicion se queda
        // como esta en vez de moverse a un nivel supuesto.
        assert_eq!(ganancia_de(None), 1.0);
        assert_eq!(ganancia_de(Some(f32::NAN)), 1.0);
        assert_eq!(ganancia_de(Some(0.0)), 1.0);
    }

    #[test]
    fn una_pista_baja_se_sube_pero_con_tope() {
        // Subir puede recortar los picos y aqui no hay limitador. Se permite
        // hasta el doble y ni un poco mas, por absurdo que sea el dato.
        assert!(ganancia_de(Some(-3.0)) > 1.0);
        assert_eq!(ganancia_de(Some(-40.0)), 2.0);
    }

    #[test]
    fn un_dato_disparatado_no_deja_la_cancion_muda() {
        assert_eq!(ganancia_de(Some(90.0)), 0.25);
    }
}
