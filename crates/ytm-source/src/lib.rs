//! Extraccion de metadatos y streams de audio de YouTube.
//!
//! Todo lo fragil del proyecto vive aqui. El resto de la aplicacion habla con
//! este crate a traves de tipos estables, de modo que cuando YouTube cambia,
//! solo cambia este crate.

pub mod clients;
pub mod innertube;
pub mod model;
pub mod select;

pub use clients::ClientConfig;
pub use innertube::InnerTube;
pub use model::{Format, PlayerResponse};

use anyhow::{bail, Result};

/// Un stream de audio listo para reproducir.
#[derive(Debug, Clone)]
pub struct AudioStream {
    pub url: String,
    pub itag: u32,
    pub codec: Option<String>,
    pub bitrate: Option<u64>,
    pub size_bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    /// Volumen de la pista en dB, para normalizacion entre canciones.
    pub loudness_db: Option<f32>,
    /// Cliente InnerTube que consiguio este stream.
    pub via_client: &'static str,
}

/// Metadatos basicos de una pista.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub video_id: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub thumbnail: Option<String>,
}

/// Resultado de resolver una pista: metadatos + stream de audio.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub track: TrackInfo,
    pub audio: AudioStream,
}

/// Resuelve una pista probando los clientes de [`clients::PREFERRED`] en orden
/// hasta que uno devuelve un stream de audio directo.
///
/// Esta cascada es el corazon de la antifragilidad: que Google cierre un cliente
/// degrada el rendimiento, no rompe la aplicacion.
pub async fn resolve(it: &InnerTube, video_id: &str) -> Result<Resolved> {
    let mut errors: Vec<String> = Vec::new();

    for client in clients::PREFERRED {
        match try_client(it, video_id, *client).await {
            Ok(resolved) => {
                tracing::info!(
                    client = client.id,
                    itag = resolved.audio.itag,
                    "stream resuelto"
                );
                return Ok(resolved);
            }
            Err(e) => {
                tracing::debug!(client = client.id, error = %e, "cliente descartado");
                errors.push(format!("  {}: {}", client.id, e));
            }
        }
    }

    bail!(
        "ningun cliente devolvio audio reproducible:\n{}",
        errors.join("\n")
    );
}

async fn try_client(it: &InnerTube, video_id: &str, client: ClientConfig) -> Result<Resolved> {
    let res = it.player(video_id, client).await?;

    if let Some(ps) = &res.playability_status {
        let status = ps.status.as_deref().unwrap_or("UNKNOWN");
        if status != "OK" {
            let reason = ps
                .reason
                .as_deref()
                .map(|r| format!(" ({r})"))
                .unwrap_or_default();
            bail!("{status}{reason}");
        }
    }

    let streaming = res
        .streaming_data
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("sin streamingData"))?;

    let fmt = select::best_audio(&streaming.adaptive_formats)
        .ok_or_else(|| anyhow::anyhow!("sin audio con URL directa"))?;

    let details = res.video_details.as_ref();

    Ok(Resolved {
        track: TrackInfo {
            video_id: video_id.to_string(),
            title: details.and_then(|d| d.title.clone()),
            author: details.and_then(|d| d.author.clone()),
            thumbnail: details
                .and_then(|d| d.thumbnail.as_ref())
                .and_then(|t| t.thumbnails.last())
                .map(|t| t.url.clone()),
        },
        audio: AudioStream {
            url: fmt.url.clone().expect("best_audio garantiza url directa"),
            itag: fmt.itag,
            codec: fmt.codec().map(str::to_string),
            bitrate: fmt.bitrate.or(fmt.average_bitrate),
            size_bytes: fmt.content_length_bytes(),
            duration_ms: fmt.approx_duration_ms.as_ref().and_then(|d| d.parse().ok()),
            loudness_db: fmt.loudness_db,
            via_client: client.id,
        },
    })
}
