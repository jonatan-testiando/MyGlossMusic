//! Decodificacion de audio con Symphonia, incluido Opus.
//!
//! # Por que no usamos `rodio::Decoder`
//!
//! YouTube Music sirve el audio **solo** en WebM/Opus (itag 251), y ni rodio ni
//! Symphonia decodifican Opus de serie. `moosicbox_opus` aporta un decodificador
//! de Opus para Symphonia, pero contra la version 0.5, mientras que rodio 0.22
//! usa la 0.6: los traits no son compatibles.
//!
//! La salida es montar nuestra propia fuente sobre Symphonia 0.5 con el registro
//! de codecs de `moosicbox_opus`, que trae Opus **y** todo lo demas (AAC, MP3,
//! FLAC...). Asi queda una unica ruta de decodificacion para cualquier formato
//! que nos llegue, en vez de dos caminos segun el itag.

use std::num::NonZero;
use std::time::Duration;

use anyhow::{Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

use crate::cache::CacheReader;

/// `CacheReader` ya es `Read + Seek`; Symphonia solo necesita saber ademas si se
/// puede buscar y cuanto mide.
impl MediaSource for CacheReader {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        match self.total_bytes() {
            0 => None,
            n => Some(n),
        }
    }
}

/// Fuente de audio para rodio respaldada por Symphonia.
pub struct SymphoniaSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    /// Muestras ya decodificadas, intercaladas por canal.
    buffer: Vec<f32>,
    cursor: usize,
    channels: NonZero<u16>,
    sample_rate: NonZero<u32>,
    duration: Option<Duration>,
    finished: bool,
}

impl SymphoniaSource {
    /// Construye la fuente a partir de un lector de cache.
    ///
    /// `extension` orienta al detector de formato (`webm`, `m4a`...). Si no
    /// acierta, Symphonia lo deduce igualmente del contenido.
    pub fn new(reader: CacheReader, extension: &str) -> Result<Self> {
        let mss = MediaSourceStream::new(Box::new(reader), Default::default());

        let mut hint = Hint::new();
        hint.with_extension(extension);

        let opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &opts, &MetadataOptions::default())
            .context("formato de audio no reconocido")?;

        let format = probed.format;
        let track = format
            .default_track()
            .context("el archivo no tiene ninguna pista")?;
        let track_id = track.id;
        let params = track.codec_params.clone();

        // El registro de `moosicbox_opus` trae Opus ademas de los codecs
        // habituales de Symphonia.
        let registry = moosicbox_opus::create_opus_registry();
        let decoder = registry
            .make(&params, &DecoderOptions::default())
            .context("no hay decodificador para este codec")?;

        let channels = params
            .channels
            .map(|c| c.count() as u16)
            .and_then(NonZero::new)
            .unwrap_or(NonZero::new(2).unwrap());
        let sample_rate = params
            .sample_rate
            .and_then(NonZero::new)
            .unwrap_or(NonZero::new(48_000).unwrap());

        let duration = params.n_frames.map(|frames| {
            Duration::from_secs_f64(frames as f64 / sample_rate.get() as f64)
        });

        Ok(Self {
            format,
            decoder,
            track_id,
            buffer: Vec::new(),
            cursor: 0,
            channels,
            sample_rate,
            duration,
            finished: false,
        })
    }

    /// Decodifica el siguiente paquete al buffer. `false` si se acabo.
    fn fill(&mut self) -> bool {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(_) => {
                    // Cualquier error aqui es fin de flujo en la practica: el
                    // archivo se acabo, o la descarga se corto.
                    self.finished = true;
                    return false;
                }
            };
            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let frames = decoded.capacity() as u64;
                    if frames == 0 {
                        continue;
                    }
                    let mut sb = SampleBuffer::<f32>::new(frames, spec);
                    sb.copy_interleaved_ref(decoded);
                    self.buffer.clear();
                    self.buffer.extend_from_slice(sb.samples());
                    self.cursor = 0;
                    return true;
                }
                // Un paquete corrupto suelto no debe cortar la cancion.
                Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                Err(_) => {
                    self.finished = true;
                    return false;
                }
            }
        }
    }

    /// Salta a una posicion. Devuelve error si el formato no lo permite.
    pub fn seek(&mut self, pos: Duration) -> Result<()> {
        let time = Time::from(pos.as_secs_f64());
        let res = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        );
        if res.is_err() {
            self.format
                .seek(
                    SeekMode::Coarse,
                    SeekTo::Time {
                        time,
                        track_id: Some(self.track_id),
                    },
                )
                .context("el formato no permite saltar a esa posicion")?;
        }
        self.decoder.reset();
        self.buffer.clear();
        self.cursor = 0;
        self.finished = false;
        Ok(())
    }
}

impl Iterator for SymphoniaSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.cursor >= self.buffer.len() {
            if self.finished || !self.fill() {
                return None;
            }
        }
        let s = self.buffer.get(self.cursor).copied();
        self.cursor += 1;
        s
    }
}

impl rodio::Source for SymphoniaSource {
    fn current_span_len(&self) -> Option<usize> {
        // Los paquetes tienen tamanos distintos, asi que se anuncia lo que queda
        // del actual: rodio vuelve a preguntar cuando se agota.
        Some(self.buffer.len().saturating_sub(self.cursor))
    }

    fn channels(&self) -> NonZero<u16> {
        self.channels
    }

    fn sample_rate(&self) -> NonZero<u32> {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.duration
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.seek(pos).map_err(|e| {
            rodio::source::SeekError::Other(std::sync::Arc::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })
    }
}

/// Extension a partir del tipo MIME, el codec o el itag, para orientar al
/// detector de formato.
///
/// El orden importa: cuando la URL viene del acunador, el codec que devolvio
/// InnerTube ya NO corresponde (InnerTube ofrecio AAC y el navegador acuno
/// Opus). Por eso el MIME de la propia URL manda sobre todo lo demas.
pub fn extension_for(itag: u32, mime: Option<&str>, codec: Option<&str>) -> &'static str {
    if let Some(m) = mime {
        if m.contains("webm") {
            return "webm";
        }
        if m.contains("mp4") {
            return "m4a";
        }
    }
    if let Some(c) = codec {
        if c.starts_with("opus") {
            return "webm";
        }
        if c.starts_with("mp4a") {
            return "m4a";
        }
    }
    match itag {
        249 | 250 | 251 => "webm",
        _ => "m4a",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_mime_manda_sobre_el_codec() {
        // El caso real: InnerTube dijo AAC, pero el acunador trajo WebM/Opus.
        assert_eq!(
            extension_for(140, Some("audio/webm; codecs=opus"), Some("mp4a.40.2")),
            "webm"
        );
    }

    #[test]
    fn deduce_la_extension_por_codec_si_no_hay_mime() {
        assert_eq!(extension_for(0, None, Some("opus")), "webm");
        assert_eq!(extension_for(0, None, Some("mp4a.40.2")), "m4a");
    }

    #[test]
    fn deduce_la_extension_por_itag_como_ultimo_recurso() {
        assert_eq!(extension_for(251, None, None), "webm");
        assert_eq!(extension_for(140, None, None), "m4a");
    }
}
