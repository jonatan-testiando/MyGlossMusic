//! Seleccion de la mejor pista de audio disponible.

use crate::model::Format;

/// Preferencia de itags de audio, de mejor a peor.
///
/// 140 (AAC-LC 128k) va primero a proposito: `symphonia` lo decodifica de forma
/// nativa, mientras que Opus (251) exige enlazar libopus. La diferencia audible
/// es despreciable. Cuando el motor soporte Opus, basta reordenar esto.
pub const AUDIO_ITAG_PREFERENCE: &[u32] = &[140, 251, 250, 249, 139];

/// Elige la mejor pista de audio *reproducible directamente* (sin descifrado).
pub fn best_audio(formats: &[Format]) -> Option<&Format> {
    best_audio_with(formats, AUDIO_ITAG_PREFERENCE)
}

/// Igual que [`best_audio`] pero con un orden de itags explicito.
pub fn best_audio_with<'a>(formats: &'a [Format], preference: &[u32]) -> Option<&'a Format> {
    for &itag in preference {
        if let Some(f) = formats
            .iter()
            .find(|f| f.itag == itag && f.is_audio() && f.is_playable_directly())
        {
            return Some(f);
        }
    }
    // Ningun itag conocido: cae al audio directo de mayor bitrate.
    formats
        .iter()
        .filter(|f| f.is_audio() && f.is_playable_directly())
        .max_by_key(|f| f.bitrate.or(f.average_bitrate).unwrap_or(0))
}
