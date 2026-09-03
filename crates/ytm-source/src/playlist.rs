//! Playlists y reconocimiento de enlaces pegados.
//!
//! El browse de una playlist devuelve los mismos `musicResponsiveListItemRenderer`
//! que la busqueda, asi que la extraccion reutiliza el recorrido recursivo de
//! `search.rs` y hereda su resistencia a cambios de estructura.

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

use crate::clients::WEB_REMIX;
use crate::innertube::InnerTube;
use crate::search::{self, SearchResult};

const BROWSE_URL: &str = "https://music.youtube.com/youtubei/v1/browse";

/// Paginas de continuacion como maximo. 4 paginas son ~400 pistas: de sobra
/// para una cola, y un tope duro contra bucles si YouTube cambia los tokens.
const MAX_PAGES: usize = 4;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub title: Option<String>,
    pub tracks: Vec<SearchResult>,
}

/// Que es lo que el usuario pego o escribio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Un enlace o id de playlist (PL..., OLAK... para albumes, RDCLAK...).
    Playlist(String),
    /// Un enlace de video/cancion suelto.
    Video(String),
    /// Texto normal: busqueda.
    Query(String),
}

/// Clasifica la entrada del cuadro de busqueda.
///
/// Un enlace con `list=` gana sobre el `v=`: quien pega el enlace de una
/// playlist copiada espera la playlist, no una cancion suelta.
pub fn parse_input(raw: &str) -> Input {
    let s = raw.trim();
    let is_url = s.contains("youtube.com/") || s.contains("youtu.be/") || s.contains("music.youtube");

    if is_url {
        if let Some(id) = param_after(s, "list=") {
            // WL (Ver mas tarde) y LL (Me gusta) exigen sesion: mejor buscar.
            if id != "WL" && id != "LL" {
                return Input::Playlist(id);
            }
        }
        if let Some(id) = param_after(s, "v=") {
            if id.len() == 11 {
                return Input::Video(id);
            }
        }
        if let Some(pos) = s.find("youtu.be/") {
            let rest = &s[pos + 9..];
            let id: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
            if id.len() == 11 {
                return Input::Video(id);
            }
        }
    }

    // Id de playlist pegado a pelo, sin URL.
    if !s.contains(' ')
        && (s.starts_with("PL") || s.starts_with("OLAK5uy_") || s.starts_with("RDCLAK"))
        && s.len() >= 13
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Input::Playlist(s.to_string());
    }

    Input::Query(s.to_string())
}

fn param_after(s: &str, key: &str) -> Option<String> {
    let pos = s.find(key)?;
    let rest = &s[pos + key.len()..];
    let id: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    (!id.is_empty()).then_some(id)
}

impl InnerTube {
    /// Pistas de una playlist publica. Anonimo: sin cookies.
    pub async fn playlist(&self, id: &str) -> Result<Playlist> {
        // El browse espera el id con prefijo VL; se acepta con o sin el.
        let browse_id = if id.starts_with("VL") {
            id.to_string()
        } else {
            format!("VL{id}")
        };

        let mut tracks: Vec<SearchResult> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut title = None;
        let mut continuation: Option<String> = None;

        for page in 0..MAX_PAGES {
            let body = match &continuation {
                None => json!({ "context": context(), "browseId": browse_id }),
                Some(token) => json!({ "context": context(), "continuation": token }),
            };

            let res = self
                .http_ref()
                .post(BROWSE_URL)
                .header("User-Agent", WEB_REMIX.user_agent)
                .header("X-YouTube-Client-Name", WEB_REMIX.client_name_id.to_string())
                .header("X-YouTube-Client-Version", WEB_REMIX.client_version)
                .header("Content-Type", "application/json")
                .header("Origin", "https://music.youtube.com")
                .header("Referer", "https://music.youtube.com/")
                .json(&body)
                .send()
                .await
                .context("fallo la peticion de playlist")?
                .error_for_status()
                .context("el servidor rechazo la playlist")?;

            let root: Value = res.json().await.context("respuesta de playlist ilegible")?;

            if page == 0 {
                title = header_title(&root);
            }

            let before = tracks.len();
            for t in search::parse_results(&root) {
                if seen.insert(t.video_id.clone()) {
                    tracks.push(t);
                }
            }

            continuation = next_token(&root);
            // Sin token o sin pistas nuevas: se acabo (lo segundo corta bucles
            // si YouTube devolviera el mismo token una y otra vez).
            if continuation.is_none() || tracks.len() == before {
                break;
            }
        }

        anyhow::ensure!(
            !tracks.is_empty(),
            "la playlist no devolvio pistas: puede ser privada o exigir sesion"
        );
        Ok(Playlist { title, tracks })
    }
}

fn context() -> Value {
    json!({
        "client": {
            "clientName": WEB_REMIX.client_name,
            "clientVersion": WEB_REMIX.client_version,
            "hl": "es",
            "gl": "US",
        }
    })
}

/// Titulo de la cabecera, sin depender de la variante exacta del renderer.
///
/// OJO: el `header` de primer nivel es el marco de la pagina (con el boton de
/// "Iniciar sesion" cuando no hay cuenta), no la cabecera de la playlist. Hay
/// que ir a los renderers de cabecera de contenido, esten donde esten, y dentro
/// de ellos al campo `title` concreto: su primer "text" a secas puede ser otra
/// cosa.
fn header_title(root: &Value) -> Option<String> {
    for renderer in [
        "musicResponsiveHeaderRenderer",
        "musicDetailHeaderRenderer",
        "musicEditablePlaylistDetailHeaderRenderer",
        "musicImmersiveHeaderRenderer",
    ] {
        let mut found = Vec::new();
        search::collect_by_key(root, renderer, &mut found);
        if let Some(text) = found
            .first()
            .and_then(|h| h.get("title"))
            .and_then(|t| search::find_str(t, "text"))
        {
            return Some(text.to_string());
        }
    }
    None
}

/// Primer token de continuacion del arbol.
fn next_token(root: &Value) -> Option<String> {
    let mut cmds = Vec::new();
    search::collect_by_key(root, "continuationCommand", &mut cmds);
    cmds.first()
        .and_then(|c| c.get("token"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            // Formato viejo: continuations[].nextContinuationData.continuation
            let mut datas = Vec::new();
            search::collect_by_key(root, "nextContinuationData", &mut datas);
            datas
                .first()
                .and_then(|c| c.get("continuation"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_enlace_con_list_es_playlist_aunque_lleve_v() {
        assert_eq!(
            parse_input("https://music.youtube.com/watch?v=abcdefghijk&list=PLxx11--_yy22"),
            Input::Playlist("PLxx11--_yy22".into())
        );
    }

    #[test]
    fn un_enlace_de_video_suelto_es_video() {
        assert_eq!(
            parse_input("https://www.youtube.com/watch?v=jig2aRZbHm4"),
            Input::Video("jig2aRZbHm4".into())
        );
        assert_eq!(
            parse_input("https://youtu.be/jig2aRZbHm4?si=xyz"),
            Input::Video("jig2aRZbHm4".into())
        );
    }

    #[test]
    fn ver_mas_tarde_no_se_trata_como_playlist() {
        // WL exige sesion; forzarlo daria un error criptico.
        assert_eq!(
            parse_input("https://www.youtube.com/watch?v=jig2aRZbHm4&list=WL"),
            Input::Video("jig2aRZbHm4".into())
        );
    }

    #[test]
    fn un_id_de_playlist_pegado_a_pelo_cuenta() {
        assert_eq!(
            parse_input("PLS5XXQa2IUO8GaqdYq7Aeh8KH4op2r1Ud"),
            Input::Playlist("PLS5XXQa2IUO8GaqdYq7Aeh8KH4op2r1Ud".into())
        );
    }

    #[test]
    fn el_texto_normal_sigue_siendo_busqueda() {
        assert_eq!(parse_input("lonely miku hardstyle"), Input::Query("lonely miku hardstyle".into()));
        assert_eq!(parse_input("PLaylist de fiesta"), Input::Query("PLaylist de fiesta".into()));
    }
}
