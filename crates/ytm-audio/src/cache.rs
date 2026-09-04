//! Cache progresivo de pistas en disco.
//!
//! # Por que un archivo y no un buffer circular
//!
//! La Fase 0 midio 25 MB/s descargando por rangos, y una cancion de 4 minutos en
//! itag 140 pesa ~3.5 MB. Es decir, la pista entera cabe en disco en ~0.15 s.
//! Un buffer circular con ventana deslizante seria complejidad pura sin ninguna
//! ganancia: complicaria el seek, no sobreviviria a un corte de red y no dejaria
//! nada reutilizable.
//!
//! En su lugar descargamos secuencialmente a un archivo mientras el decodificador
//! lo lee por detras. Eso da tres cosas gratis:
//!
//!   - **Arranque instantaneo**: se empieza a sonar con el primer trozo, sin
//!     esperar al resto (importante en conexiones lentas).
//!   - **Seek trivial**: es un `File::seek`, sin repetir peticiones.
//!   - **Reproduccion offline**: lo ya escuchado queda en cache.

use anyhow::{Context, Result};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Tamano de cada peticion por rango.
///
/// 256 KiB, no mas. Las URLs acunadas traen `cps=256` y RECHAZAN con 403
/// cualquier rango de 1 MiB o mas; medido: 64, 128, 256 y 512 KiB responden 200
/// y 1024 KiB responde 403. Se usa 256 por ser el tamano que la propia URL
/// declara, con 512 aun de margen.
///
/// En itag 251 son ~16 s de audio por peticion: de sobra para arrancar al
/// instante sin inundar la red de peticiones diminutas.
const CHUNK: u64 = 262_144;

/// Umbral a partir del cual un 403 se interpreta como el limite de poToken.
///
/// Medido con `ytm-spike limits` sobre 6 videos: 5 de 6 cortan exactamente en
/// 1 MiB. El unico que se descarga entero es `dQw4w9WgXcQ`, que resulto ser
/// anormalmente permisivo y con el que se valido (mal) la Fase 0.
const POTOKEN_WALL: u64 = 1_048_576;

/// Intentos por trozo antes de darlo por perdido. Con esperas de 400, 800 y
/// 1600 ms cubre de sobra la ventana en la que una URL recien acunada aun no
/// esta lista.
const RETRIES: u32 = 4;

/// Futuro que resuelve cuando un proceso externo termina de escribir el archivo.
pub type BoxDone =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'static>>;

/// Estado compartido de la descarga de una pista.
#[derive(Debug)]
struct Shared {
    downloaded: AtomicU64,
    total: AtomicU64,
    done: AtomicBool,
    error: Mutex<Option<String>>,
}

/// Una pista descargandose (o ya descargada) a disco.
#[derive(Clone, Debug)]
pub struct TrackCache {
    path: PathBuf,
    shared: Arc<Shared>,
}

impl TrackCache {
    /// Arranca la descarga en segundo plano y devuelve inmediatamente.
    ///
    /// Si el archivo ya esta completo en cache, no toca la red.
    pub fn start(
        url: String,
        expected_size: Option<u64>,
        path: PathBuf,
        http: reqwest::Client,
    ) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        // Acierto de cache: el archivo existe y tiene el tamano esperado.
        if let (Some(expected), Ok(meta)) = (expected_size, std::fs::metadata(&path)) {
            if meta.len() == expected {
                tracing::debug!(path = %path.display(), "cache hit");
                // La fecha de modificacion es el reloj de uso de la cache: la
                // poda borra por antiguedad, y sin tocarla al reutilizar una
                // pista, la mas escuchada seria la primera en caer.
                tocar(&path);
                return Ok(Self {
                    path,
                    shared: Arc::new(Shared {
                        downloaded: AtomicU64::new(expected),
                        total: AtomicU64::new(expected),
                        done: AtomicBool::new(true),
                        error: Mutex::new(None),
                    }),
                });
            }
        }

        let shared = Arc::new(Shared {
            downloaded: AtomicU64::new(0),
            total: AtomicU64::new(expected_size.unwrap_or(0)),
            done: AtomicBool::new(false),
            error: Mutex::new(None),
        });

        let cache = Self {
            path: path.clone(),
            shared: Arc::clone(&shared),
        };

        tokio::spawn(async move {
            if let Err(e) = download(url, path, http, Arc::clone(&shared)).await {
                tracing::error!(error = %e, "fallo la descarga");
                *shared.error.lock().unwrap() = Some(e.to_string());
            }
            shared.done.store(true, Ordering::Release);
        });

        Ok(cache)
    }

    /// Sigue un archivo que escribe OTRO proceso (yt-dlp).
    ///
    /// No se descarga nada aqui: se vigila el tamano del archivo mientras `done`
    /// no resuelve, y al resolver se cierra. El lector funciona exactamente igual
    /// que con una descarga propia, asi que el resto del motor no distingue una
    /// fuente de la otra.
    pub fn start_external(path: PathBuf, expected_size: Option<u64>, done: BoxDone) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        // Acierto de cache: el externo puede haberla dejado entera en otra sesion.
        if let (Some(expected), Ok(meta)) = (expected_size, std::fs::metadata(&path)) {
            if meta.len() == expected {
                drop(done);
                return Ok(Self {
                    path,
                    shared: Arc::new(Shared {
                        downloaded: AtomicU64::new(expected),
                        total: AtomicU64::new(expected),
                        done: AtomicBool::new(true),
                        error: Mutex::new(None),
                    }),
                });
            }
        }

        let shared = Arc::new(Shared {
            downloaded: AtomicU64::new(0),
            total: AtomicU64::new(expected_size.unwrap_or(0)),
            done: AtomicBool::new(false),
            error: Mutex::new(None),
        });
        let cache = Self { path: path.clone(), shared: Arc::clone(&shared) };

        tokio::spawn(async move {
            let mut done = done;
            let mut tick = tokio::time::interval(Duration::from_millis(50));
            let result = loop {
                tokio::select! {
                    r = &mut done => break r,
                    _ = tick.tick() => {
                        if let Ok(m) = std::fs::metadata(&path) {
                            shared.downloaded.store(m.len(), Ordering::Release);
                        }
                    }
                }
            };
            if let Ok(m) = std::fs::metadata(&path) {
                shared.downloaded.store(m.len(), Ordering::Release);
                // El tamano real manda: `filesize_approx` de yt-dlp es estimado.
                if result.is_ok() || shared.total.load(Ordering::Acquire) == 0 {
                    shared.total.store(m.len(), Ordering::Release);
                }
            }
            if let Err(e) = result {
                tracing::error!(error = %e, "el proceso externo fallo");
                *shared.error.lock().unwrap() = Some(e.to_string());
            }
            shared.done.store(true, Ordering::Release);
        });
        Ok(cache)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn downloaded_bytes(&self) -> u64 {
        self.shared.downloaded.load(Ordering::Acquire)
    }

    pub fn total_bytes(&self) -> u64 {
        self.shared.total.load(Ordering::Acquire)
    }

    pub fn is_done(&self) -> bool {
        self.shared.done.load(Ordering::Acquire)
    }

    pub fn error(&self) -> Option<String> {
        self.shared.error.lock().unwrap().clone()
    }

    /// Fraccion descargada, de 0.0 a 1.0.
    pub fn progress(&self) -> f32 {
        let total = self.total_bytes();
        if total == 0 {
            return if self.is_done() { 1.0 } else { 0.0 };
        }
        (self.downloaded_bytes() as f32 / total as f32).clamp(0.0, 1.0)
    }

    /// Espera a que haya al menos `bytes` disponibles (o a que termine).
    pub fn wait_for(&self, bytes: u64, timeout: Duration) -> Result<()> {
        let start = Instant::now();
        loop {
            if let Some(e) = self.error() {
                anyhow::bail!("{e}");
            }
            if self.downloaded_bytes() >= bytes || self.is_done() {
                return Ok(());
            }
            if start.elapsed() > timeout {
                anyhow::bail!("timeout esperando {bytes} bytes");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Abre un lector bloqueante sobre el archivo en curso.
    pub fn reader(&self) -> Result<CacheReader> {
        // El archivo debe existir antes de abrirlo: si la descarga aun no ha
        // escrito nada, esperamos al primer byte.
        self.wait_for(1, Duration::from_secs(30))
            .context("la descarga no arranco")?;
        let file = std::fs::File::open(&self.path)
            .with_context(|| format!("no se pudo abrir {}", self.path.display()))?;
        Ok(CacheReader {
            file,
            pos: 0,
            cache: self.clone(),
        })
    }
}

/// Descarga secuencial por trozos.
///
/// Por rangos a proposito: un GET completo lo estrangula YouTube a ~0.03 MB/s
/// (ver `ytm-spike bench`).
async fn download(
    url: String,
    path: PathBuf,
    http: reqwest::Client,
    shared: Arc<Shared>,
) -> Result<()> {
    // Se escribe DIRECTAMENTE en la ruta final, sin `.part` intermedio, por dos
    // motivos: el lector necesita abrir el archivo mientras aun se descarga (si
    // no, adios al arranque progresivo), y en Windows renombrar un archivo con
    // un handle abierto falla. La validez de la cache se comprueba por tamano al
    // arrancar, asi que un archivo a medias de una ejecucion anterior
    // simplemente se vuelve a descargar.
    let mut file = std::fs::File::create(&path)
        .with_context(|| format!("no se pudo crear {}", path.display()))?;

    // El tamano total viene en `clen` cuando la URL la acuno el navegador.
    let declared: u64 = param(&url, "clen").and_then(|v| v.parse().ok()).unwrap_or(0);
    if declared > 0 {
        shared.total.store(declared, Ordering::Release);
    }

    let mut offset: u64 = 0;
    let mut rn: u64 = 0;
    loop {
        // El rango va como PARAMETRO DE QUERY, no como cabecera `Range:`.
        //
        // Medido sobre la misma URL acunada: con `&range=` responde 200 y los
        // bytes del medio; solo con la cabecera responde 403. El reproductor
        // real de YouTube tambien usa el parametro, y manda un `rn` que
        // incrementa en cada peticion.
        //
        // Ojo: `&range=` corta del lado del servidor, asi que NO se debe anadir
        // ademas la cabecera; combinarlas da 416 en cualquier offset que no
        // sea 0.
        let target = format!("{url}&range={offset}-{}&rn={rn}", offset + CHUNK - 1);
        rn += 1;

        // Reintento con espera creciente.
        //
        // Una URL recien acunada devuelve 403 durante los primeros instantes:
        // medido, la misma URL que falla 18 ms despues de acunarse responde 200
        // unos segundos mas tarde, con las mismas cabeceras y el mismo rango.
        // No es un fallo permanente, asi que rendirse al primer intento
        // convertiria un retraso en un error.
        let mut res = None;
        for intento in 0..RETRIES {
            let r = http
                .get(&target)
                .send()
                .await
                .context("fallo una peticion de rango")?;
            if r.status().is_success()
                || r.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE
            {
                res = Some(r);
                break;
            }
            let ultimo = intento + 1 == RETRIES;
            if ultimo {
                res = Some(r);
                break;
            }
            let espera = Duration::from_millis(400 * (1 << intento));
            tracing::debug!(
                status = r.status().as_u16(),
                offset,
                ms = espera.as_millis() as u64,
                "rango rechazado, reintentando"
            );
            tokio::time::sleep(espera).await;
        }
        let res = res.expect("el bucle siempre deja una respuesta");

        if res.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            break;
        }
        let status = res.status();
        if !status.is_success() {
            // Un 403 justo despues del primer MiB es la firma del limite de
            // poToken: googlevideo sirve ~1 MiB (~65 s en itag 140) de la
            // mayoria del contenido y rechaza el resto si la peticion no lleva
            // un token de atestacion. No es un fallo de red ni una URL caducada,
            // y no se arregla reintentando ni volviendo a resolver: esta medido
            // en `ytm-spike limits`.
            // Un 403 en el primer byte casi siempre es la URL, no el limite:
            // registrarla es lo unico que permite distinguir "caducada",
            // "mal formada" y "capada".
            if offset == 0 {
                tracing::error!(
                    status = status.as_u16(),
                    "rechazo en el primer byte"
                );
                // Volcado de la URL completa para poder reproducir el fallo
                // fuera de la aplicacion. Solo si se pide explicitamente.
                if let Ok(path) = std::env::var("POSIBLE_URL_OUT") {
                    let _ = std::fs::write(path, &url);
                }
            }
            if status == reqwest::StatusCode::FORBIDDEN && offset >= POTOKEN_WALL {
                anyhow::bail!(
                    "YouTube corta la descarga en {} KB sin poToken \
                     (403 en offset {}). Solo hay ~{} s de audio.",
                    offset / 1024,
                    offset,
                    offset / 16_000
                );
            }
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!(
                "el servidor rechazo el rango: HTTP {} en offset {} ({})",
                status.as_u16(),
                offset,
                body.chars().take(200).collect::<String>()
            );
        }

        // Con el parametro de query la respuesta es 200 sin `Content-Range`, asi
        // que solo sirve como respaldo cuando la URL no traia `clen`.
        if offset == 0 && declared == 0 {
            if let Some(total) = parse_content_range_total(&res) {
                shared.total.store(total, Ordering::Release);
            }
        }

        let chunk = res.bytes().await.context("fallo la lectura del trozo")?;
        let n = chunk.len() as u64;
        if n == 0 {
            break;
        }

        file.write_all(&chunk).context("fallo la escritura en cache")?;
        file.flush().ok();
        offset += n;
        shared.downloaded.store(offset, Ordering::Release);

        if n < CHUNK {
            break;
        }
        let total = shared.total.load(Ordering::Acquire);
        if total > 0 && offset >= total {
            break;
        }
    }

    drop(file);
    shared.total.store(offset, Ordering::Release);
    tracing::debug!(bytes = offset, path = %path.display(), "descarga completa");
    Ok(())
}

/// Lee un parametro de la query de una URL.
fn param(url: &str, key: &str) -> Option<String> {
    url.split(['?', '&'])
        .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
        .map(str::to_string)
}

fn parse_content_range_total(res: &reqwest::Response) -> Option<u64> {
    // Formato: "bytes 0-1048575/3448192"
    res.headers()
        .get(reqwest::header::CONTENT_RANGE)?
        .to_str()
        .ok()?
        .split('/')
        .nth(1)?
        .parse()
        .ok()
}

/// Lector bloqueante sobre una pista que puede estar aun descargandose.
///
/// Implementa `Read + Seek`, que es justo lo que pide `rodio::Decoder`, asi que
/// el decodificador no se entera de que hay una descarga por detras.
pub struct CacheReader {
    file: std::fs::File,
    pos: u64,
    cache: TrackCache,
}

impl CacheReader {
    /// Tamano total de la pista, o 0 si aun no se conoce.
    pub fn total_bytes(&self) -> u64 {
        self.cache.total_bytes()
    }
}

impl Read for CacheReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Espera a que los bytes pedidos esten en disco.
        let needed = self.pos + 1;
        if self.cache.downloaded_bytes() < needed && !self.cache.is_done() {
            self.cache
                .wait_for(needed, Duration::from_secs(30))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::TimedOut, e.to_string()))?;
        }

        let available = self.cache.downloaded_bytes().saturating_sub(self.pos);
        if available == 0 {
            return Ok(0); // EOF real
        }

        let want = buf.len().min(available as usize);
        self.file.seek(SeekFrom::Start(self.pos))?;
        let n = self.file.read(&mut buf[..want])?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for CacheReader {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let total = self.cache.total_bytes();
        self.pos = match from {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(d) => self.pos.saturating_add_signed(d),
            SeekFrom::End(d) => total.saturating_add_signed(d),
        };
        Ok(self.pos)
    }
}

/// Directorio de cache de audio de la aplicacion.
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("posible-ytmusic")
        .join("audio")
}

/// Ruta en cache para una pista concreta.
pub fn track_path(video_id: &str, itag: u32) -> PathBuf {
    cache_dir().join(format!("{video_id}-{itag}.m4a"))
}

/// Marca una pista como recien usada.
///
/// No hay `utime` en la biblioteca estandar y la fecha de ULTIMO ACCESO de NTFS
/// viene desactivada de fabrica en Windows, asi que no sirve. Abrir el archivo
/// para escritura sin escribir nada tampoco actualiza nada. Lo que si funciona
/// en los dos sistemas es reescribir el ultimo byte por su mismo valor: el
/// contenido no cambia y la fecha de modificacion pasa a ser ahora.
fn tocar(path: &Path) {
    let Ok(mut f) = std::fs::OpenOptions::new().read(true).write(true).open(path) else {
        return;
    };
    let Ok(len) = f.seek(SeekFrom::End(-1)) else {
        return; // archivo vacio: no hay byte que reescribir, y tampoco importa
    };
    let mut b = [0u8; 1];
    if f.read_exact(&mut b).is_ok() && f.seek(SeekFrom::Start(len)).is_ok() {
        let _ = f.write_all(&b);
    }
}

/// Limite por defecto de la cache en disco: 3 GiB.
///
/// Una pista en itag 140 pesa ~3,5 MB, asi que son unas 850 canciones. Pasado
/// eso, lo que queda cacheado ya no es "lo que escucho" sino "todo lo que he
/// escuchado alguna vez", y eso no acelera nada.
pub const DEFAULT_LIMIT: u64 = 3 * 1024 * 1024 * 1024;

/// Borra las pistas menos usadas hasta que la cache cabe en `limit`.
///
/// Devuelve cuantas se borraron. `limit` en 0 significa sin limite.
///
/// El criterio es la fecha de modificacion, que [`tocar`] refresca en cada
/// acierto: se va la que lleva mas tiempo sin sonar, no la mas vieja. Los
/// archivos de `protegidos` no se tocan aunque sean los mas antiguos — son la
/// pista que suena y la que se esta precargando, y borrarlas seria cortar la
/// reproduccion para hacer sitio a la reproduccion.
pub fn prune(limit: u64, protegidos: &[PathBuf]) -> u64 {
    prune_dir(&cache_dir(), limit, protegidos)
}

/// Igual que [`prune`] pero sobre un directorio cualquiera, para poder probarlo.
pub fn prune_dir(dir: &Path, limit: u64, protegidos: &[PathBuf]) -> u64 {
    if limit == 0 {
        return 0;
    }
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return 0;
    };

    // Plano y sin recursion, como el resto de operaciones sobre la cache: un
    // borrado recursivo sobre una ruta inesperada no tiene vuelta atras.
    let mut archivos: Vec<(PathBuf, u64, std::time::SystemTime)> = entradas
        .filter_map(Result::ok)
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some((e.path(), meta.len(), meta.modified().ok()?))
        })
        .collect();

    let mut total: u64 = archivos.iter().map(|(_, len, _)| len).sum();
    if total <= limit {
        return 0;
    }

    archivos.sort_by_key(|(_, _, t)| *t); // el mas antiguo primero
    let mut borrados = 0;
    for (path, len, _) in archivos {
        if total <= limit {
            break;
        }
        if protegidos.iter().any(|p| *p == path) {
            continue;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
            borrados += 1;
        }
    }
    if borrados > 0 {
        tracing::info!(borrados, restante = total, limite = limit, "cache podada");
    }
    borrados
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Directorio temporal propio: `prune_dir` borra archivos, y una prueba que
    /// borra no puede compartir carpeta con otra.
    fn carpeta(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ytm-cache-test-{nombre}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Crea un archivo de `bytes`.
    ///
    /// Las pistas se crean en orden de mas antigua a mas reciente, con una
    /// pausa entre medias: es lo que da la fecha de modificacion sin depender
    /// de ninguna biblioteca para falsearla. 20 ms sobran — NTFS y ext4 guardan
    /// la fecha con resolucion muy por debajo del milisegundo.
    fn pista(dir: &Path, nombre: &str, bytes: usize) -> PathBuf {
        let p = dir.join(nombre);
        std::fs::write(&p, vec![7u8; bytes]).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        p
    }

    #[test]
    fn se_va_la_que_lleva_mas_tiempo_sin_sonar() {
        let d = carpeta("antiguedad");
        pista(&d, "vieja.m4a", 600);
        pista(&d, "media.m4a", 600);
        pista(&d, "nueva.m4a", 600);

        // Caben dos de 600 B en 1300, asi que sobra una: la mas antigua.
        assert_eq!(prune_dir(&d, 1_300, &[]), 1);
        assert!(!d.join("vieja.m4a").exists());
        assert!(d.join("media.m4a").exists());
        assert!(d.join("nueva.m4a").exists());
    }

    #[test]
    fn no_borra_la_que_esta_sonando() {
        // Aunque sea la mas antigua: borrarla seria cortar la reproduccion
        // para hacer sitio a la reproduccion.
        let d = carpeta("protegida");
        let sonando = pista(&d, "sonando.m4a", 600);
        pista(&d, "otra.m4a", 600);

        assert_eq!(prune_dir(&d, 700, &[sonando.clone()]), 1);
        assert!(sonando.exists());
        assert!(!d.join("otra.m4a").exists());
    }

    #[test]
    fn por_debajo_del_tope_no_toca_nada() {
        let d = carpeta("holgada");
        pista(&d, "a.m4a", 600);
        assert_eq!(prune_dir(&d, 10_000, &[]), 0);
        assert!(d.join("a.m4a").exists());
    }

    #[test]
    fn cero_es_sin_limite() {
        let d = carpeta("sin-limite");
        pista(&d, "a.m4a", 600);
        assert_eq!(prune_dir(&d, 0, &[]), 0);
        assert!(d.join("a.m4a").exists());
    }

    #[test]
    fn reutilizar_una_pista_la_pone_al_final_de_la_cola() {
        // `tocar` es lo que convierte la poda en "la menos usada" en vez de "la
        // mas vieja". Sin esto, la cancion favorita seria la primera en caer.
        let d = carpeta("tocar");
        let favorita = pista(&d, "favorita.m4a", 600);
        pista(&d, "de-paso.m4a", 600);

        std::thread::sleep(Duration::from_millis(20));
        tocar(&favorita);
        assert_eq!(std::fs::read(&favorita).unwrap(), vec![7u8; 600], "el contenido no cambia");

        assert_eq!(prune_dir(&d, 700, &[]), 1);
        assert!(favorita.exists());
        assert!(!d.join("de-paso.m4a").exists());
    }
}
