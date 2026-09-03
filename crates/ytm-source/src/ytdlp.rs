//! Extraccion delegada en `yt-dlp`.
//!
//! # Por que
//!
//! Obtener una pista ENTERA de YouTube exige reproducir su protocolo de
//! autorizacion en vivo (poToken, firma, `n`, UMP, autorizacion posicional por
//! rangos). Esta todo medido en `ytm-spike`: es un sistema activamente defendido
//! que cambia cada pocos meses. `yt-dlp` lo mantiene un equipo grande a diario;
//! reimplementarlo aqui es la garantia de que la app se rompa.
//!
//! Asi que `yt-dlp` hace la descarga y escribe el archivo de cache; todo lo
//! demas (cache progresivo, decodificacion, cola, interfaz) es nuestro y no
//! sabe que existe. Cuando YouTube cambie, se actualiza un binario.
//!
//! # Donde se busca el binario
//!
//! 1. `POSIBLE_YTDLP` (ruta explicita).
//! 2. `yt-dlp.exe` junto al ejecutable (sidecar, para distribuir).
//! 3. `yt-dlp` en el PATH.
//! 4. `python -m yt_dlp`.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Separador entre campos del `--print`. No imprimible: nunca aparece en un
/// titulo.
const SEP: char = '\u{1F}';

/// Selector de formato. Primero AAC en m4a, luego Opus en WebM, luego lo que
/// haya: todos los decodifica `ytm-audio`.
const FORMAT: &str = "bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio";

/// Como invocar yt-dlp en esta maquina.
#[derive(Debug, Clone)]
pub struct YtDlp {
    program: PathBuf,
    prefix: Vec<String>,
    pub version: String,
}

/// Lo que yt-dlp sabe de la pista antes de descargarla.
#[derive(Debug, Clone)]
pub struct Info {
    pub path: PathBuf,
    pub size: Option<u64>,
    pub ext: String,
    pub acodec: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub duration_ms: Option<u64>,
    pub thumbnail: Option<String>,
}

impl Info {
    pub fn mime(&self) -> Option<String> {
        match self.ext.as_str() {
            "m4a" | "mp4" => Some("audio/mp4".into()),
            "webm" => Some("audio/webm".into()),
            "opus" | "ogg" => Some("audio/ogg".into()),
            "mp3" => Some("audio/mpeg".into()),
            _ => None,
        }
    }
}

/// Una descarga en curso (o ya completa en cache).
pub struct Download {
    pub info: Info,
    /// `None` si la pista ya estaba en cache: no hay proceso que esperar.
    child: Option<tokio::process::Child>,
    /// Linea de metadatos tal cual la imprimio yt-dlp. Se guarda como marcador
    /// de "descarga completa" y permite reconstruir `Info` en otro arranque.
    raw: String,
}

impl Download {
    /// Espera a que yt-dlp termine. Error si el proceso fallo.
    pub async fn wait(mut self) -> Result<()> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        let status = child.wait().await.context("yt-dlp no termino")?;
        if status.success() {
            // Marcador de completitud. El tamano NO sirve como criterio: yt-dlp
            // solo conoce `filesize_approx`, que casi nunca coincide con el
            // tamano final, y sin marcador cada arranque volveria a descargar.
            let _ = std::fs::write(ok_marker(&self.info.path), &self.raw);
            return Ok(());
        }
        let mut tail = String::new();
        if let Some(mut err) = child.stderr.take() {
            use tokio::io::AsyncReadExt;
            let _ = err.read_to_string(&mut tail).await;
        }
        let tail: String = tail.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
        bail!("yt-dlp termino con {status}: {tail}")
    }
}

impl YtDlp {
    /// Localiza yt-dlp. Sincrono a proposito: se llama desde el arranque de
    /// Tauri, que corre fuera del runtime asincrono.
    pub fn detect() -> Option<Self> {
        let mut candidates: Vec<(PathBuf, Vec<String>)> = Vec::new();

        if let Ok(p) = std::env::var("POSIBLE_YTDLP") {
            candidates.push((PathBuf::from(p), vec![]));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push((dir.join("yt-dlp.exe"), vec![]));
                candidates.push((dir.join("yt-dlp"), vec![]));
            }
        }
        candidates.push((PathBuf::from("yt-dlp"), vec![]));
        candidates.push((PathBuf::from("python"), vec!["-m".into(), "yt_dlp".into()]));
        candidates.push((PathBuf::from("py"), vec!["-m".into(), "yt_dlp".into()]));

        for (program, prefix) in candidates {
            let mut cmd = std::process::Command::new(&program);
            cmd.args(&prefix).arg("--version");
            cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
            hide_console_std(&mut cmd);
            if let Ok(out) = cmd.output() {
                if out.status.success() {
                    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !version.is_empty() {
                        tracing::info!(program = %program.display(), %version, "yt-dlp detectado");
                        return Some(Self { program, prefix, version });
                    }
                }
            }
        }
        None
    }

    /// Arranca la descarga de una pista al directorio dado.
    ///
    /// Devuelve en cuanto yt-dlp ha decidido formato y ruta (antes de bajar un
    /// solo byte), de modo que el reproductor puede abrir el archivo y empezar a
    /// sonar mientras se descarga.
    pub async fn download(&self, video_id: &str, dir: &Path) -> Result<Download> {
        std::fs::create_dir_all(dir).ok();

        // Acierto de cache: marcador de una descarga completa anterior.
        if let Some(hit) = cached(video_id, dir) {
            tracing::debug!(video_id, "yt-dlp: en cache");
            return Ok(hit);
        }
        // Sin marcador, cualquier `<id>.*` es una descarga a medias. Fuera:
        // yt-dlp lo tomaria por "ya descargado" y dejaria la pista cortada.
        purge_partial(video_id, dir);

        let template = dir.join(format!("{video_id}.%(ext)s"));

        // Campos que necesitamos, impresos ANTES de descargar.
        let print = format!(
            "before_dl:%(filename)s{SEP}%(filesize,filesize_approx)s{SEP}%(ext)s{SEP}\
             %(acodec)s{SEP}%(title)s{SEP}%(artist,uploader,channel)s{SEP}\
             %(duration)s{SEP}%(thumbnail)s"
        );

        let mut cmd = tokio::process::Command::new(&self.program);
        cmd.args(&self.prefix)
            .arg("--no-playlist")
            .arg("--no-warnings")
            .arg("--no-progress")
            .arg("--quiet")
            .arg("--no-simulate")
            // Sin `.part`: el lector abre el archivo mientras se escribe, y en
            // Windows renombrar un archivo abierto falla.
            .arg("--no-part")
            .arg("--no-mtime")
            // Sin post-procesado. El "fixup" del contenedor escribe
            // `<nombre>.temp.m4a` y lo renombra sobre el original, que nosotros
            // ya tenemos abierto reproduciendo: en Windows eso es "Acceso
            // denegado" y yt-dlp termina en error. El original decodifica bien.
            .arg("--fixup")
            .arg("never")
            .arg("-f")
            .arg(FORMAT)
            .arg("-o")
            .arg(&template)
            .arg("--print")
            .arg(&print)
            .arg(format!("https://music.youtube.com/watch?v={video_id}"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        hide_console_tokio(&mut cmd);

        let mut child = cmd.spawn().context("no se pudo lanzar yt-dlp")?;
        let stdout = child.stdout.take().context("yt-dlp sin stdout")?;
        let mut lines = BufReader::new(stdout).lines();

        // Primera linea util = nuestros campos. Con un margen: si YouTube no
        // responde, mejor un error claro que colgarse.
        let line = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            while let Ok(Some(l)) = lines.next_line().await {
                if l.contains(SEP) {
                    return Some(l);
                }
            }
            None
        })
        .await
        .context("yt-dlp tardo demasiado en resolver la pista")?;

        let Some(line) = line else {
            // Ha muerto antes de imprimir: recoge el motivo.
            let status = child.wait().await.ok();
            let mut tail = String::new();
            if let Some(mut err) = child.stderr.take() {
                use tokio::io::AsyncReadExt;
                let _ = err.read_to_string(&mut tail).await;
            }
            let tail: String = tail.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
            bail!("yt-dlp no resolvio la pista ({status:?}): {tail}");
        };

        // El resto de stdout no interesa, pero hay que drenarlo para que el
        // proceso no se bloquee al escribir.
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });

        let info = parse_info(&line)?;
        tracing::info!(video_id, ext = %info.ext, size = ?info.size, "yt-dlp descargando");
        Ok(Download {
            info,
            child: Some(child),
            raw: line,
        })
    }
}

/// Convierte la linea del `--print` en [`Info`].
fn parse_info(line: &str) -> Result<Info> {
    let f: Vec<&str> = line.split(SEP).collect();
    let get = |i: usize| {
        f.get(i)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && *s != "NA")
    };

    let path = PathBuf::from(get(0).context("yt-dlp no dio ruta")?);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .or_else(|| get(2).map(str::to_lowercase))
        .unwrap_or_else(|| "m4a".into());

    Ok(Info {
        path,
        size: get(1).and_then(|s| s.parse::<f64>().ok()).map(|s| s as u64),
        ext,
        acodec: get(3).map(str::to_string),
        title: get(4).map(str::to_string),
        author: get(5).map(str::to_string),
        duration_ms: get(6)
            .and_then(|s| s.parse::<f64>().ok())
            .map(|s| (s * 1000.0) as u64),
        thumbnail: get(7).map(str::to_string),
    })
}

/// Ruta del marcador de "descarga completa" de una pista.
fn ok_marker(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".ok");
    PathBuf::from(s)
}

/// Descarga ya completa en cache, reconstruida desde su marcador.
fn cached(video_id: &str, dir: &Path) -> Option<Download> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(&format!("{video_id}.")) || !name.ends_with(".ok") {
            continue;
        }
        let raw = std::fs::read_to_string(entry.path()).ok()?;
        let mut info = parse_info(&raw).ok()?;
        let meta = std::fs::metadata(&info.path).ok()?;
        if meta.len() == 0 {
            continue;
        }
        // El tamano real manda sobre el aproximado que dio yt-dlp.
        info.size = Some(meta.len());
        return Some(Download {
            info,
            child: None,
            raw,
        });
    }
    None
}

/// Borra restos `<id>.*` sin marcador: descargas interrumpidas.
fn purge_partial(video_id: &str, dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&format!("{video_id}.")) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(windows)]
fn hide_console_std(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
fn hide_console_std(_: &mut std::process::Command) {}

#[cfg(windows)]
fn hide_console_tokio(cmd: &mut tokio::process::Command) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
fn hide_console_tokio(_: &mut tokio::process::Command) {}
