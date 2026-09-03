//! Radio: la cola de recomendaciones que YouTube arma a partir de una pista.
//!
//! Es el endpoint `next`, y es la pieza que hace posible recomendar sin sesion.
//! Dada una cancion devuelve ~50 pistas parecidas, las mismas que sirve la
//! aplicacion oficial. Comprobado contra la referencia: su cola "A continuacion"
//! son exactamente estas.
//!
//! Con eso, el inicio puede construirse sembrando radios desde el historial
//! local en vez de depender del feed personalizado, que si exige cuenta.
//!
//! Mismo criterio que en `search.rs` y `browse.rs`: se recorre el JSON por
//! nombre de renderer, nunca por rutas fijas.
//!
//! Cuando esto deje de devolver nada: `ytm-spike radio <videoId>`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::clients::WEB_REMIX;
use crate::innertube::InnerTube;
use crate::search::{collect_by_key, extract_thumbnail, runs_text, SearchResult};

const NEXT_URL: &str = "https://music.youtube.com/youtubei/v1/next";

/// Una cola de radio ya normalizada.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Radio {
    /// Identificador de la cola, por si hace falta pedir continuaciones.
    pub playlist_id: Option<String>,
    pub tracks: Vec<SearchResult>,
    /// Canal del artista de la pista semilla.
    ///
    /// Sale del byline de la propia cola. La pestania "Relacionado" de YouTube
    /// Music cuelga de un `browseId` con prefijo `MPTR` que SOLO responde
    /// dentro del contexto de sesion de `next` — pedido suelto devuelve una
    /// respuesta vacia de 2 KB, comprobado. Con el canal del artista se puede
    /// ofrecer algo real en su lugar.
    pub artist_browse_id: Option<String>,
}

/// Prefijo de las radios de YouTube Music.
///
/// `RDAMVM` + el id de la pista es la "radio de esta cancion". Hay otros
/// prefijos (`RDAO`, `RDAT`) para radios de artista o de biblioteca, pero
/// necesitan sesion.
fn radio_id(video_id: &str) -> String {
    format!("RDAMVM{video_id}")
}

impl InnerTube {
    /// Radio a partir de una pista. Anonimo: no lleva cookies.
    pub async fn radio(&self, video_id: &str) -> Result<Radio> {
        let body = json!({
            "context": {
                "client": {
                    "clientName": WEB_REMIX.client_name,
                    "clientVersion": WEB_REMIX.client_version,
                    "hl": "es",
                    "gl": "US",
                }
            },
            "videoId": video_id,
            "playlistId": radio_id(video_id),
            // Sin esto la respuesta viene preparada para reproducir video, con
            // otros formatos y pistas que no son musicales.
            "isAudioOnly": true,
        });

        let res = self
            .http_ref()
            .post(NEXT_URL)
            .header("User-Agent", WEB_REMIX.user_agent)
            .header("X-YouTube-Client-Name", WEB_REMIX.client_name_id.to_string())
            .header("X-YouTube-Client-Version", WEB_REMIX.client_version)
            .header("Content-Type", "application/json")
            .header("Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/")
            .json(&body)
            .send()
            .await
            .with_context(|| format!("fallo la peticion de radio de {video_id}"))?
            .error_for_status()
            .with_context(|| format!("el servidor rechazo la radio de {video_id}"))?;

        let json: Value = res
            .json()
            .await
            .with_context(|| format!("respuesta de radio ilegible para {video_id}"))?;
        Ok(parse_radio(&json))
    }
}

/// Normaliza una respuesta de `next` sin depender de la forma del arbol.
pub fn parse_radio(root: &Value) -> Radio {
    let mut renderers = Vec::new();
    collect_by_key(root, "playlistPanelVideoRenderer", &mut renderers);

    let mut tracks = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for r in renderers {
        let Some(video_id) = r.get("videoId").and_then(Value::as_str) else {
            continue;
        };
        if !seen.insert(video_id.to_string()) {
            continue;
        }
        let title = runs_text(r.get("title"));
        if title.is_empty() {
            continue;
        }

        tracks.push(SearchResult {
            video_id: video_id.to_string(),
            title,
            // `shortBylineText` es solo el artista. El largo trae ademas
            // visualizaciones y "me gusta", que aqui sobran.
            subtitle: runs_text(r.get("shortBylineText")),
            duration: Some(runs_text(r.get("lengthText"))).filter(|d| !d.is_empty()),
            thumbnail: extract_thumbnail(r),
        });
    }

    let playlist_id = crate::search::find_str(root, "playlistId").map(str::to_string);
    Radio { playlist_id, tracks, artist_browse_id: artist_of_seed(root) }
}

/// Canal del artista de la primera pista de la cola.
///
/// El byline trae el nombre del artista enlazado a su canal. Se filtra por el
/// prefijo `UC` porque en esos mismos `runs` hay tambien enlaces a otras cosas.
fn artist_of_seed(root: &Value) -> Option<String> {
    let mut renderers = Vec::new();
    collect_by_key(root, "playlistPanelVideoRenderer", &mut renderers);
    let primera = renderers.first()?;

    let mut endpoints = Vec::new();
    collect_by_key(primera.get("longBylineText")?, "browseEndpoint", &mut endpoints);
    endpoints
        .iter()
        .filter_map(|e| e.get("browseId").and_then(Value::as_str))
        .find(|id| id.starts_with("UC"))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pista(id: &str, titulo: &str) -> Value {
        json!({ "playlistPanelVideoRenderer": {
            "videoId": id,
            "title": { "runs": [{ "text": titulo }] },
            "shortBylineText": { "runs": [{ "text": "Un Artista" }] },
            "longBylineText": { "runs": [
                { "text": "Un Artista", "navigationEndpoint": {
                    "browseEndpoint": { "browseId": "UCartista123" }
                }},
                { "text": " \u{2022} " },
                { "text": "3,7 M de visualizaciones" }
            ]},
            "lengthText": { "runs": [{ "text": "4:22" }] },
            "thumbnail": { "thumbnails": [
                { "url": "https://ejemplo/pequena.jpg" },
                { "url": "https://ejemplo/grande.jpg" }
            ]}
        }})
    }

    #[test]
    fn lee_la_cola_de_radio() {
        let payload = json!({ "da": { "igual": { "lo": { "hondo": [
            pista("aaaaaaaaaaa", "Primera"), pista("bbbbbbbbbbb", "Segunda")
        ]}}}});
        let radio = parse_radio(&payload);

        assert_eq!(radio.tracks.len(), 2);
        let t = &radio.tracks[0];
        assert_eq!(t.video_id, "aaaaaaaaaaa");
        assert_eq!(t.title, "Primera");
        assert_eq!(t.duration.as_deref(), Some("4:22"));
        assert_eq!(t.thumbnail.as_deref(), Some("https://ejemplo/grande.jpg"));
    }

    #[test]
    fn saca_el_canal_del_artista_de_la_semilla() {
        // Es el repuesto de la pestania "Relacionado": su `browseId` MPTR solo
        // responde dentro del contexto de `next`, asi que se usa el canal.
        let radio = parse_radio(&json!({ "x": [pista("aaaaaaaaaaa", "Semilla")] }));
        assert_eq!(radio.artist_browse_id.as_deref(), Some("UCartista123"));
    }

    #[test]
    fn sin_canal_en_el_byline_no_se_inventa_uno() {
        let sin = json!({ "playlistPanelVideoRenderer": {
            "videoId": "aaaaaaaaaaa",
            "title": { "runs": [{ "text": "Suelta" }] }
        }});
        assert!(parse_radio(&sin).artist_browse_id.is_none());
    }

    #[test]
    fn el_subtitulo_es_solo_el_artista() {
        // El texto largo trae ademas visualizaciones y "me gusta". En una cola
        // eso es ruido: la fila solo tiene sitio para el artista.
        let radio = parse_radio(&json!({ "x": pista("aaaaaaaaaaa", "Una") }));
        assert_eq!(radio.tracks[0].subtitle, "Un Artista");
    }

    #[test]
    fn la_cola_no_repite_pistas() {
        let una = pista("aaaaaaaaaaa", "Repetida");
        let payload = json!({ "a": una.clone(), "b": una });
        assert_eq!(parse_radio(&payload).tracks.len(), 1);
    }

    #[test]
    fn descarta_entradas_sin_titulo() {
        let payload = json!({ "playlistPanelVideoRenderer": { "videoId": "aaaaaaaaaaa" } });
        assert!(parse_radio(&payload).tracks.is_empty());
    }

    #[test]
    fn la_radio_de_una_pista_lleva_su_id() {
        assert_eq!(radio_id("dQw4w9WgXcQ"), "RDAMVMdQw4w9WgXcQ");
    }
}
