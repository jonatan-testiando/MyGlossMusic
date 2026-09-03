//! Miniaturas servidas desde Rust, con cache en disco.
//!
//! # Por que no dejar que el webview las pida directamente
//!
//! Una busqueda pinta 20 miniaturas de golpe, y googleusercontent responde
//! **429** a parte de la rafaga: unas cargan y otras salen rotas, sin patron.
//! Medido: la misma URL que falla en la interfaz devuelve 200 desde fuera.
//!
//! Servirlas desde aqui resuelve tres cosas a la vez: se limita la concurrencia
//! (no hay rafaga), se reintenta con espera ante un 429, y cada imagen se baja
//! UNA vez en la vida y queda en disco (portadas offline, y la paleta se calcula
//! sobre los mismos bytes sin volver a pedirlos).
//!
//! La interfaz pide `thumb://localhost/?u=<url>` (en Windows,
//! `http://thumb.localhost/?u=<url>`, que es como WebView2 expone los esquemas
//! propios).

use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tauri::http;

/// Descargas simultaneas como maximo. Cuatro es suficiente para que una lista
/// aparezca rapido y demasiado poco para disparar el 429.
static GATE: LazyLock<tokio::sync::Semaphore> = LazyLock::new(|| tokio::sync::Semaphore::new(4));

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
        )
        .build()
        .expect("cliente HTTP de miniaturas")
});

/// Hosts de los que aceptamos servir imagenes. Es un proxy: sin lista blanca,
/// la interfaz podria hacer que Rust descargara cualquier cosa.
const ALLOWED_HOSTS: &[&str] = &[
    "yt3.googleusercontent.com",
    "lh3.googleusercontent.com",
    "yt3.ggpht.com",
    "i.ytimg.com",
    "i9.ytimg.com",
];

/// Registra el esquema `thumb` en el builder de Tauri.
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.register_asynchronous_uri_scheme_protocol("thumb", |_ctx, request, responder| {
        let query = request.uri().query().unwrap_or("").to_string();
        tauri::async_runtime::spawn(async move {
            let response = match serve(&query).await {
                Ok(bytes) => http::Response::builder()
                    .status(200)
                    .header("Content-Type", sniff_mime(&bytes))
                    // Inmutable: la URL ya lleva el tamano; si cambia, cambia la URL.
                    .header("Cache-Control", "public, max-age=31536000, immutable")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(bytes),
                Err(e) => {
                    tracing::debug!(error = %e, "miniatura no servida");
                    http::Response::builder()
                        .status(502)
                        .header("Content-Type", "text/plain")
                        .body(e.to_string().into_bytes())
                }
            };
            if let Ok(r) = response {
                responder.respond(r);
            }
        });
    })
}

/// Bytes de una imagen remota, de cache si ya se bajo.
///
/// Publico para que la paleta se calcule sobre los mismos bytes que ve la
/// interfaz, sin una segunda peticion.
pub async fn cached_bytes(url: &str) -> Result<Vec<u8>> {
    let host = url
        .strip_prefix("https://")
        .and_then(|r| r.split('/').next())
        .context("URL de miniatura sin host")?;
    if !ALLOWED_HOSTS.contains(&host) {
        bail!("host no permitido para miniaturas: {host}");
    }

    let path = cache_path(url);
    if let Ok(bytes) = tokio::fs::read(&path).await {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }

    let _permit = GATE.acquire().await.context("semaforo cerrado")?;
    // Segunda comprobacion: otra peticion identica pudo bajarla mientras
    // esperabamos el turno.
    if let Ok(bytes) = tokio::fs::read(&path).await {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }

    let bytes = fetch_with_retry(url).await?;
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await.ok();
    }
    // Escritura atomica: nunca queda una imagen a medias en cache.
    let tmp = path.with_extension("tmp");
    if tokio::fs::write(&tmp, &bytes).await.is_ok() {
        let _ = tokio::fs::rename(&tmp, &path).await;
    }
    Ok(bytes)
}

async fn serve(query: &str) -> Result<Vec<u8>> {
    let encoded = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("u="))
        .context("falta el parametro u")?;
    let url = percent_decode(encoded);
    cached_bytes(&url).await
}

/// Reintenta ante 429 y 5xx con espera creciente: 300, 600, 1200 ms.
async fn fetch_with_retry(url: &str) -> Result<Vec<u8>> {
    let mut last = None;
    for intento in 0..4u32 {
        match HTTP.get(url).send().await {
            Ok(res) if res.status().is_success() => {
                return Ok(res.bytes().await.context("cuerpo ilegible")?.to_vec());
            }
            Ok(res) => {
                let status = res.status();
                if status.as_u16() != 429 && !status.is_server_error() {
                    bail!("HTTP {status}");
                }
                last = Some(format!("HTTP {status}"));
            }
            Err(e) => last = Some(e.to_string()),
        }
        tokio::time::sleep(Duration::from_millis(300 * (1 << intento))).await;
    }
    bail!("agotados los reintentos: {}", last.unwrap_or_default())
}

fn cache_path(url: &str) -> PathBuf {
    crate::db::data_dir()
        .join("thumbs")
        .join(format!("{:016x}.img", fnv1a64(url.as_bytes())))
}

/// Hash estable entre versiones de Rust (el `DefaultHasher` de std no lo es, y
/// una cache que se invalida al actualizar el compilador es una cache mala).
fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn sniff_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

pub(crate) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_hash_es_estable() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn detecta_el_tipo_de_imagen() {
        assert_eq!(sniff_mime(&[0xFF, 0xD8, 0xFF]), "image/jpeg");
        assert_eq!(sniff_mime(b"\x89PNG\r\n"), "image/png");
        assert_eq!(sniff_mime(b"RIFF\0\0\0\0WEBPVP8 "), "image/webp");
    }

    #[test]
    fn decodifica_la_url() {
        assert_eq!(
            percent_decode("https%3A%2F%2Fa.com%2Fx%3D1%26y%3D2"),
            "https://a.com/x=1&y=2"
        );
    }
}
