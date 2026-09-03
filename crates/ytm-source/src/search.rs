//! Busqueda en YouTube Music.
//!
//! # Por que recorremos el JSON en vez de indexar rutas fijas
//!
//! La respuesta de InnerTube anida los resultados a ~10 niveles de profundidad,
//! y esa ruta cambia entre versiones, entre tipos de busqueda e incluso entre
//! secciones de una misma respuesta. Indexarla a mano
//! (`contents.tabbedSearchResultsRenderer.tabs[0]...`) es exactamente el tipo de
//! codigo que rompe estas aplicaciones cada pocos meses.
//!
//! En su lugar buscamos recursivamente los nodos por su nombre de renderer, que
//! es lo unico estable. Si YouTube reordena el arbol, esto sigue funcionando.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::clients::WEB_REMIX;
use crate::innertube::InnerTube;

const SEARCH_URL: &str = "https://music.youtube.com/youtubei/v1/search";

/// Un resultado de busqueda.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub video_id: String,
    pub title: String,
    /// Artista, album y demas, tal y como los muestra YouTube Music.
    pub subtitle: String,
    pub duration: Option<String>,
    pub thumbnail: Option<String>,
}

/// Filtros de busqueda. El `params` es opaco y lo define YouTube.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// Todo mezclado.
    All,
    /// Solo canciones. Es lo que quiere un reproductor de musica el 95% del
    /// tiempo: evita videos, directos y podcasts.
    Songs,
    Videos,
    Albums,
}

impl Filter {
    fn params(self) -> Option<&'static str> {
        match self {
            Filter::All => None,
            Filter::Songs => Some("EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D"),
            Filter::Videos => Some("EgWKAQIQAWoKEAkQChAFEAMQBA%3D%3D"),
            Filter::Albums => Some("EgWKAQIYAWoKEAkQChAFEAMQBA%3D%3D"),
        }
    }
}

impl InnerTube {
    /// Busca en YouTube Music. Anonimo: no lleva cookies.
    pub async fn search(&self, query: &str, filter: Filter) -> Result<Vec<SearchResult>> {
        let mut body = json!({
            "context": {
                "client": {
                    "clientName": WEB_REMIX.client_name,
                    "clientVersion": WEB_REMIX.client_version,
                    "hl": "es",
                    "gl": "US",
                }
            },
            "query": query,
        });
        if let Some(p) = filter.params() {
            body["params"] = json!(p);
        }

        let res = self
            .http_ref()
            .post(SEARCH_URL)
            .header("User-Agent", WEB_REMIX.user_agent)
            .header("X-YouTube-Client-Name", WEB_REMIX.client_name_id.to_string())
            .header("X-YouTube-Client-Version", WEB_REMIX.client_version)
            .header("Content-Type", "application/json")
            .header("Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/")
            .json(&body)
            .send()
            .await
            .context("fallo la peticion de busqueda")?
            .error_for_status()
            .context("el servidor rechazo la busqueda")?;

        let json: Value = res.json().await.context("respuesta de busqueda ilegible")?;
        Ok(parse_results(&json))
    }
}

/// Extrae los resultados sin depender de la forma del arbol.
pub fn parse_results(root: &Value) -> Vec<SearchResult> {
    let mut renderers = Vec::new();
    collect_by_key(root, "musicResponsiveListItemRenderer", &mut renderers);

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for r in renderers {
        let Some(video_id) = extract_video_id(r) else {
            continue; // artistas, albums y playlists no tienen videoId
        };
        if !seen.insert(video_id.clone()) {
            continue;
        }

        let columns = flex_column_texts(r);
        let title = columns.first().cloned().unwrap_or_default();
        if title.is_empty() {
            continue;
        }
        let subtitle = columns.get(1).cloned().unwrap_or_default();

        out.push(SearchResult {
            video_id,
            title,
            duration: extract_duration(&subtitle),
            subtitle: clean_subtitle(&subtitle),
            thumbnail: extract_thumbnail(r),
        });
    }
    out
}

/// Recolecta todos los valores asociados a `key`, a cualquier profundidad.
pub(crate) fn collect_by_key<'a>(v: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                if k == key {
                    out.push(val);
                }
                collect_by_key(val, key, out);
            }
        }
        Value::Array(items) => {
            for val in items {
                collect_by_key(val, key, out);
            }
        }
        _ => {}
    }
}

/// Primer valor de cadena asociado a `key`, a cualquier profundidad.
pub(crate) fn find_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                if k == key {
                    if let Some(s) = val.as_str() {
                        return Some(s);
                    }
                }
                if let Some(found) = find_str(val, key) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(|val| find_str(val, key)),
        _ => None,
    }
}

/// El `videoId` puede colgar de `playlistItemData` o del `watchEndpoint`.
pub(crate) fn extract_video_id(r: &Value) -> Option<String> {
    let id = find_str(r, "videoId")?;
    // Los ids de YouTube son siempre 11 caracteres.
    (id.len() == 11).then(|| id.to_string())
}

/// Concatena un nodo de texto de InnerTube: `{ "runs": [{ "text": ... }] }`.
///
/// Los tramos vienen partidos por razones de estilo (el artista, el separador y
/// el album van sueltos), no de contenido, asi que siempre se quieren juntos.
pub(crate) fn runs_text(node: Option<&Value>) -> String {
    node.and_then(|n| n.get("runs"))
        .and_then(Value::as_array)
        .map(|runs| {
            runs.iter()
                .filter_map(|r| r.get("text").and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default()
}

/// Texto de cada columna flexible (titulo, subtitulo, ...).
pub(crate) fn flex_column_texts(r: &Value) -> Vec<String> {
    let mut cols = Vec::new();
    let Some(flex) = r.get("flexColumns").and_then(Value::as_array) else {
        return cols;
    };

    for col in flex {
        let text = col
            .get("musicResponsiveListItemFlexColumnRenderer")
            .and_then(|c| c.get("text"));
        cols.push(runs_text(text));
    }
    cols
}

pub(crate) fn extract_thumbnail(r: &Value) -> Option<String> {
    let mut arrays = Vec::new();
    collect_by_key(r, "thumbnails", &mut arrays);
    // La ultima miniatura del primer conjunto es la de mayor resolucion.
    arrays
        .first()?
        .as_array()?
        .last()?
        .get("url")?
        .as_str()
        .map(str::to_string)
}

/// Saca "3:45" del subtitulo, que YouTube mete entre separadores.
pub(crate) fn extract_duration(subtitle: &str) -> Option<String> {
    subtitle
        .split('\u{2022}')
        .map(str::trim)
        .find(|part| {
            let mut chunks = part.split(':');
            let ok = chunks.clone().count() >= 2
                && chunks.all(|c| !c.is_empty() && c.chars().all(|ch| ch.is_ascii_digit()));
            ok
        })
        .map(str::to_string)
}

/// Quita la duracion del subtitulo: se muestra aparte.
pub(crate) fn clean_subtitle(subtitle: &str) -> String {
    let parts: Vec<&str> = subtitle
        .split('\u{2022}')
        .map(str::trim)
        .filter(|p| {
            !p.is_empty()
                && !(p.contains(':') && p.chars().all(|c| c.is_ascii_digit() || c == ':'))
        })
        .collect();
    parts.join(" \u{2022} ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encuentra_resultados_anidados_a_cualquier_profundidad() {
        // La gracia del recorrido recursivo: da igual como este envuelto.
        let payload = json!({
            "un": { "envoltorio": { "cualquiera": [{ "y": { "otro": {
                "musicResponsiveListItemRenderer": {
                    "playlistItemData": { "videoId": "abcdefghijk" },
                    "flexColumns": [
                        { "musicResponsiveListItemFlexColumnRenderer":
                            { "text": { "runs": [{ "text": "Mi Cancion" }] } } },
                        { "musicResponsiveListItemFlexColumnRenderer":
                            { "text": { "runs": [{ "text": "Artista \u{2022} Album \u{2022} 3:45" }] } } }
                    ],
                    "thumbnail": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                        { "url": "https://ejemplo/pequena.jpg" },
                        { "url": "https://ejemplo/grande.jpg" }
                    ]}}}
                }
            }}}]}}
        });

        let out = parse_results(&payload);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].video_id, "abcdefghijk");
        assert_eq!(out[0].title, "Mi Cancion");
        assert_eq!(out[0].duration.as_deref(), Some("3:45"));
        assert_eq!(out[0].subtitle, "Artista \u{2022} Album");
        assert_eq!(out[0].thumbnail.as_deref(), Some("https://ejemplo/grande.jpg"));
    }

    #[test]
    fn descarta_entradas_sin_video_id() {
        let payload = json!({
            "musicResponsiveListItemRenderer": {
                "flexColumns": [{ "musicResponsiveListItemFlexColumnRenderer":
                    { "text": { "runs": [{ "text": "Un Artista" }] } } }]
            }
        });
        assert!(parse_results(&payload).is_empty());
    }

    #[test]
    fn deduplica() {
        let item = json!({
            "musicResponsiveListItemRenderer": {
                "playlistItemData": { "videoId": "abcdefghijk" },
                "flexColumns": [{ "musicResponsiveListItemFlexColumnRenderer":
                    { "text": { "runs": [{ "text": "Repetida" }] } } }]
            }
        });
        let payload = json!({ "a": item.clone(), "b": item });
        assert_eq!(parse_results(&payload).len(), 1);
    }
}
