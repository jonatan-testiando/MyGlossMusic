//! Letras sincronizadas via LRCLIB.
//!
//! LRCLIB es gratuita, sin clave de API y sin limites agresivos.
//!
//! # Sobre el resaltado "palabra por palabra"
//!
//! LRCLIB entrega LRC estandar, que lleva una marca de tiempo POR LINEA, no por
//! palabra. El efecto de barrido que se ve en las aplicaciones bonitas no es
//! sincronia real por palabra: es un degradado que recorre la linea interpolando
//! entre su marca y la de la siguiente. Se ve identico y no depende de datos que
//! practicamente no existen. Ese calculo lo hace la interfaz; aqui solo damos
//! los tiempos de inicio y fin de cada linea.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const API: &str = "https://lrclib.net/api";

/// Una linea de letra con su ventana temporal.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub start_ms: u64,
    /// Marca de la linea siguiente. La interfaz interpola entre ambas para el
    /// barrido del texto.
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    /// Vacio si solo hay letra sin sincronizar.
    pub lines: Vec<Line>,
    /// Texto plano, como respaldo.
    pub plain: Option<String>,
    pub synced: bool,
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct LrclibTrack {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

/// Busca letra para una pista.
///
/// Cascada deliberada: primero la busqueda exacta por artista+titulo+duracion,
/// que es la que acierta el sincronizado; si falla, busqueda difusa. La cobertura
/// de LRCLIB es irregular fuera del pop anglosajon, asi que fallar es normal y no
/// debe tratarse como un error.
pub async fn fetch(
    http: &reqwest::Client,
    title: &str,
    artist: &str,
    duration_secs: u64,
) -> Result<Option<Lyrics>> {
    let (track, artist) = clean(title, artist);

    // 1) Coincidencia exacta.
    let exact = http
        .get(format!("{API}/get"))
        .query(&[
            ("track_name", track.as_str()),
            ("artist_name", artist.as_str()),
            ("duration", &duration_secs.to_string()),
        ])
        .send()
        .await;

    if let Ok(res) = exact {
        if res.status().is_success() {
            if let Ok(t) = res.json::<LrclibTrack>().await {
                if let Some(l) = build(t, "LRCLIB") {
                    return Ok(Some(l));
                }
            }
        }
    }

    // 2) Busqueda difusa.
    let res = http
        .get(format!("{API}/search"))
        .query(&[("q", format!("{artist} {track}"))])
        .send()
        .await
        .context("fallo la busqueda en LRCLIB")?;

    if !res.status().is_success() {
        return Ok(None);
    }

    let hits: Vec<LrclibTrack> = res.json().await.unwrap_or_default();
    let found: Vec<Lyrics> = hits.into_iter().filter_map(|t| build(t, "LRCLIB")).collect();

    // LRCLIB devuelve la misma cancion varias veces, unas con sincronia y otras
    // sin ella, en orden arbitrario. Coger la primera daria letra estatica
    // aunque exista la sincronizada, asi que se prefiere explicitamente.
    Ok(found
        .iter()
        .find(|l| l.synced)
        .or_else(|| found.first())
        .cloned())
}

fn build(t: LrclibTrack, source: &str) -> Option<Lyrics> {
    let lines = t.synced_lyrics.as_deref().map(parse_lrc).unwrap_or_default();
    let plain = t.plain_lyrics.filter(|p| !p.trim().is_empty());

    if lines.is_empty() && plain.is_none() {
        return None;
    }
    Some(Lyrics {
        synced: !lines.is_empty(),
        lines,
        plain,
        source: source.into(),
    })
}

/// Quita el ruido que YouTube mete en los titulos y que impide que LRCLIB
/// encuentre nada: "(Official Video)", "[4K Remaster]", " - Topic", etc.
fn clean(title: &str, artist: &str) -> (String, String) {
    let mut t = title.to_string();
    for (open, close) in [('(', ')'), ('[', ']')] {
        while let (Some(a), Some(b)) = (t.find(open), t.rfind(close)) {
            if a < b {
                t.replace_range(a..=b, "");
            } else {
                break;
            }
        }
    }
    let t = t.trim().trim_end_matches('-').trim().to_string();

    // El subtitulo suele venir como "Artista • Album • 3:28".
    let a = artist
        .split('\u{2022}')
        .next()
        .unwrap_or(artist)
        .trim()
        .trim_end_matches(" - Topic")
        .to_string();

    (t, a)
}

/// Convierte LRC a lineas con ventana temporal.
///
/// Formato: `[mm:ss.xx] texto`. Una linea puede llevar varias marcas.
fn parse_lrc(lrc: &str) -> Vec<Line> {
    let mut out: Vec<(u64, String)> = Vec::new();

    for raw in lrc.lines() {
        let mut rest = raw;
        let mut stamps = Vec::new();

        while rest.starts_with('[') {
            let Some(close) = rest.find(']') else { break };
            let inside = &rest[1..close];
            if let Some(ms) = parse_stamp(inside) {
                stamps.push(ms);
            }
            rest = &rest[close + 1..];
        }

        let text = rest.trim().to_string();
        for ms in stamps {
            out.push((ms, text.clone()));
        }
    }

    out.sort_by_key(|(ms, _)| *ms);

    // La ventana de cada linea llega hasta la siguiente. La ultima dura 5 s,
    // que es una suposicion razonable para el cierre de una cancion.
    let mut lines = Vec::with_capacity(out.len());
    for i in 0..out.len() {
        let start = out[i].0;
        let end = out.get(i + 1).map(|(ms, _)| *ms).unwrap_or(start + 5_000);
        // Las lineas vacias del LRC marcan pausas instrumentales; se conservan
        // para que el desplazamiento respete los silencios.
        lines.push(Line {
            start_ms: start,
            end_ms: end,
            text: out[i].1.clone(),
        });
    }
    lines
}

/// `mm:ss.xx` o `mm:ss` a milisegundos.
fn parse_stamp(s: &str) -> Option<u64> {
    let (m, rest) = s.split_once(':')?;
    let minutes: u64 = m.trim().parse().ok()?;
    let (sec, frac) = rest.split_once('.').unwrap_or((rest, "0"));
    let seconds: u64 = sec.trim().parse().ok()?;
    // Las centesimas pueden venir con 2 o 3 digitos.
    let frac_ms: u64 = match frac.len() {
        0 => 0,
        1 => frac.parse::<u64>().ok()? * 100,
        2 => frac.parse::<u64>().ok()? * 10,
        _ => frac[..3].parse().ok()?,
    };
    Some(minutes * 60_000 + seconds * 1_000 + frac_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_lrc_con_ventanas() {
        let lrc = "[00:12.50]Primera linea\n[00:15.00]Segunda\n[00:18.25]Tercera";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].start_ms, 12_500);
        assert_eq!(lines[0].end_ms, 15_000, "la ventana llega a la siguiente");
        assert_eq!(lines[2].end_ms, 23_250, "la ultima dura 5 s");
    }

    #[test]
    fn acepta_marcas_multiples_en_una_linea() {
        let lines = parse_lrc("[00:10.00][01:20.00]Estribillo");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_ms, 10_000);
        assert_eq!(lines[1].start_ms, 80_000);
    }

    #[test]
    fn limpia_el_ruido_de_los_titulos_de_youtube() {
        let (t, a) = clean(
            "Never Gonna Give You Up (Official Video) [4K Remaster]",
            "Rick Astley \u{2022} Whenever You Need Somebody \u{2022} 3:33",
        );
        assert_eq!(t, "Never Gonna Give You Up");
        assert_eq!(a, "Rick Astley");
    }

    #[test]
    fn tolera_marcas_sin_centesimas() {
        assert_eq!(parse_stamp("01:30"), Some(90_000));
        assert_eq!(parse_stamp("00:05.5"), Some(5_500));
        assert_eq!(parse_stamp("00:05.123"), Some(5_123));
    }
}
