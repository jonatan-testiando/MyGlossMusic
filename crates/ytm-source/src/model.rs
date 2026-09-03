//! Tipos de la respuesta del endpoint `player` de InnerTube.
//!
//! Deliberadamente permisivos: casi todo es `Option`. YouTube anade y quita
//! campos sin avisar, y un `null` inesperado no debe tumbar la reproduccion.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerResponse {
    pub playability_status: Option<PlayabilityStatus>,
    pub streaming_data: Option<StreamingData>,
    pub video_details: Option<VideoDetails>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayabilityStatus {
    /// `OK`, `UNPLAYABLE`, `LOGIN_REQUIRED`, `ERROR`, `AGE_CHECK_REQUIRED`...
    pub status: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingData {
    /// Segundos que la URL sigue siendo valida (tipicamente ~21600 = 6h).
    pub expires_in_seconds: Option<String>,
    #[serde(default)]
    pub formats: Vec<Format>,
    /// Pistas separadas de audio y video. Es lo unico que nos interesa.
    #[serde(default)]
    pub adaptive_formats: Vec<Format>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Format {
    pub itag: u32,
    /// URL directa. Si falta, hay que descifrar `signature_cipher`.
    pub url: Option<String>,
    /// Presente cuando la URL viene ofuscada: requiere ejecutar JS del player.
    pub signature_cipher: Option<String>,
    pub mime_type: Option<String>,
    pub bitrate: Option<u64>,
    pub average_bitrate: Option<u64>,
    pub content_length: Option<String>,
    pub approx_duration_ms: Option<String>,
    pub audio_quality: Option<String>,
    pub audio_sample_rate: Option<String>,
    pub audio_channels: Option<u8>,
    /// Volumen medido de la pista, en dB. Nos servira para normalizar niveles
    /// entre canciones sin recalcular nada.
    pub loudness_db: Option<f32>,
}

impl Format {
    /// `true` si es una pista de solo audio.
    pub fn is_audio(&self) -> bool {
        self.mime_type.as_deref().is_some_and(|m| m.starts_with("audio/"))
    }

    /// Codec declarado en el `mimeType`, p.ej. `mp4a.40.2` u `opus`.
    pub fn codec(&self) -> Option<&str> {
        let mime = self.mime_type.as_deref()?;
        let start = mime.find("codecs=\"")? + 8;
        let rest = &mime[start..];
        Some(&rest[..rest.find('"')?])
    }

    /// `true` si la URL es utilizable tal cual, sin descifrado.
    pub fn is_playable_directly(&self) -> bool {
        self.url.is_some()
    }

    pub fn content_length_bytes(&self) -> Option<u64> {
        self.content_length.as_ref()?.parse().ok()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoDetails {
    pub video_id: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub length_seconds: Option<String>,
    pub thumbnail: Option<Thumbnails>,
}

#[derive(Debug, Deserialize)]
pub struct Thumbnails {
    #[serde(default)]
    pub thumbnails: Vec<Thumbnail>,
}

#[derive(Debug, Deserialize)]
pub struct Thumbnail {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}
