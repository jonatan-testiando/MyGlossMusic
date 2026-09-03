//! Endpoint `browse` de InnerTube: inicio, artista y album.
//!
//! Es la superficie que alimenta todo lo que no es reproducir: el feed de
//! inicio, la pagina de un artista, la de un album. Sigue la misma regla que
//! `search.rs` — se recorre el JSON buscando NOMBRES DE RENDERER, nunca rutas
//! fijas. Las rutas de InnerTube tienen ~10 niveles y cambian; los nombres de
//! renderer llevan aniios estables.
//!
//! # Vocabulario de YouTube, traducido
//!
//! - `musicCarouselShelfRenderer`: una fila con flechas ("Volver a escuchar").
//! - `musicShelfRenderer`: una lista vertical ("Canciones populares").
//! - `musicTwoRowItemRenderer`: una tarjeta — portada arriba, texto debajo.
//! - `musicResponsiveListItemRenderer`: una fila — miniatura y columnas.
//!
//! Los dos primeros son estanterias; los dos ultimos, lo que hay dentro. Un
//! `Shelf` puede traer cualquiera de los dos tipos de elemento, asi que ambos
//! se normalizan al mismo [`ShelfItem`].
//!
//! Cuando esto deje de devolver nada: `ytm-spike browse FEmusic_home`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::clients::WEB_REMIX;
use crate::innertube::InnerTube;
use crate::search::{collect_by_key, extract_thumbnail, flex_column_texts, runs_text};

const BROWSE_URL: &str = "https://music.youtube.com/youtubei/v1/browse";

/// Feed de inicio de YouTube Music.
pub const HOME: &str = "FEmusic_home";

/// Que representa un elemento: decide adonde lleva al pulsarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    /// Se reproduce. `id` es un `videoId`.
    Track,
    /// Se abre. `id` es un `browseId`.
    Album,
    Artist,
    /// Se abre. `id` es un `playlistId` o un `browseId` con prefijo `VL`.
    Playlist,
}

/// Un elemento de una estanteria, ya normalizado.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfItem {
    pub kind: ItemKind,
    /// `videoId` si es una pista, `browseId` o `playlistId` en los demas casos.
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub thumbnail: Option<String>,
    pub duration: Option<String>,
}

/// Una fila del feed, con su titulo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shelf {
    pub title: String,
    pub items: Vec<ShelfItem>,
}

/// Una pagina de `browse` ya normalizada.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowsePage {
    /// Nombre del artista o del album. El feed de inicio no tiene.
    pub title: Option<String>,
    /// "1,25 M de oyentes mensuales", "Album · 2023", y demas.
    pub subtitle: Option<String>,
    /// Imagen de cabecera.
    pub thumbnail: Option<String>,
    pub shelves: Vec<Shelf>,
}

impl InnerTube {
    /// Pide una pagina a `browse`. Anonimo: no lleva cookies.
    ///
    /// `params` es opaco y lo define YouTube (lo usan las pestanias de artista).
    pub async fn browse(&self, browse_id: &str, params: Option<&str>) -> Result<BrowsePage> {
        let mut body = json!({
            "context": {
                "client": {
                    "clientName": WEB_REMIX.client_name,
                    "clientVersion": WEB_REMIX.client_version,
                    "hl": "es",
                    "gl": "US",
                }
            },
            "browseId": browse_id,
        });
        if let Some(p) = params {
            body["params"] = json!(p);
        }

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
            .with_context(|| format!("fallo la peticion browse de {browse_id}"))?
            .error_for_status()
            .with_context(|| format!("el servidor rechazo browse de {browse_id}"))?;

        let json: Value = res
            .json()
            .await
            .with_context(|| format!("respuesta ilegible de browse {browse_id}"))?;
        Ok(parse_page(&json))
    }

    /// Feed de inicio.
    pub async fn home(&self) -> Result<BrowsePage> {
        self.browse(HOME, None).await
    }
}

/// Normaliza una respuesta de `browse` sin depender de la forma del arbol.
pub fn parse_page(root: &Value) -> BrowsePage {
    let mut page = BrowsePage {
        shelves: collect_shelves(root)
            .into_iter()
            .filter_map(|s| {
                let shelf = parse_shelf(s);
                // Una estanteria sin nada dentro solo pinta un titulo huerfano.
                (!shelf.items.is_empty()).then_some(shelf)
            })
            .collect(),
        ..Default::default()
    };

    if let Some(header) = find_header(root) {
        page.title = non_empty(runs_text(header.get("title")));
        page.subtitle = non_empty(header_subtitle(header));
        page.thumbnail = extract_thumbnail(header);
    }
    page
}

/// La cabecera de artista o album. El feed de inicio no trae ninguna.
fn find_header(root: &Value) -> Option<&Value> {
    for key in [
        "musicImmersiveHeaderRenderer",
        "musicDetailHeaderRenderer",
        "musicResponsiveHeaderRenderer",
        "musicVisualHeaderRenderer",
    ] {
        let mut found = Vec::new();
        collect_by_key(root, key, &mut found);
        if let Some(h) = found.into_iter().next() {
            return Some(h);
        }
    }
    None
}

/// El subtitulo aparece con nombres distintos segun el tipo de cabecera.
fn header_subtitle(header: &Value) -> String {
    for key in ["subtitle", "secondSubtitle", "description"] {
        let text = runs_text(header.get(key));
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

/// Recorre el arbol EN ORDEN y recoge las estanterias sin bajar dentro de ellas.
///
/// Lo segundo es lo importante: los elementos de una estanteria son a su vez
/// renderers, y si el recorrido siguiera bajando, una fila de canciones se
/// contaria ademas como estanteria vacia. Y el orden importa porque el feed de
/// inicio se pinta en el orden en que YouTube lo manda.
fn collect_shelves(v: &Value) -> Vec<&Value> {
    let mut out = Vec::new();
    walk_shelves(v, &mut out);
    out
}

fn walk_shelves<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                if k == "musicCarouselShelfRenderer" || k == "musicShelfRenderer" {
                    out.push(val);
                    continue; // no se baja: lo de dentro son elementos, no estanterias
                }
                walk_shelves(val, out);
            }
        }
        Value::Array(items) => {
            for val in items {
                walk_shelves(val, out);
            }
        }
        _ => {}
    }
}

fn parse_shelf(shelf: &Value) -> Shelf {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut cards = Vec::new();
    collect_by_key(shelf, "musicTwoRowItemRenderer", &mut cards);
    let mut rows = Vec::new();
    collect_by_key(shelf, "musicResponsiveListItemRenderer", &mut rows);

    for item in cards
        .into_iter()
        .filter_map(parse_card)
        .chain(rows.into_iter().filter_map(parse_row))
    {
        if seen.insert((item.kind, item.id.clone())) {
            items.push(item);
        }
    }

    Shelf { title: shelf_title(shelf), items }
}

fn shelf_title(shelf: &Value) -> String {
    // Los carruseles llevan el titulo dentro de `header`, con un renderer que
    // cambia de nombre segun la seccion; las listas lo llevan suelto.
    if let Some(header) = shelf.get("header") {
        let mut titles = Vec::new();
        collect_by_key(header, "title", &mut titles);
        if let Some(t) = titles.into_iter().map(|t| runs_text(Some(t))).find(|t| !t.is_empty()) {
            return t;
        }
    }
    runs_text(shelf.get("title"))
}

/// Tarjeta: portada arriba, titulo y subtitulo debajo.
fn parse_card(r: &Value) -> Option<ShelfItem> {
    let title = runs_text(r.get("title"));
    if title.is_empty() {
        return None;
    }
    let (kind, id) = target(r.get("navigationEndpoint")?)?;
    Some(ShelfItem {
        kind,
        id,
        title,
        subtitle: runs_text(r.get("subtitle")),
        thumbnail: extract_thumbnail(r),
        duration: None,
    })
}

/// Fila: miniatura y columnas de texto. Mismo formato que la busqueda.
fn parse_row(r: &Value) -> Option<ShelfItem> {
    let columns = flex_column_texts(r);
    let title = columns.first()?.clone();
    if title.is_empty() {
        return None;
    }
    let subtitle = columns.get(1).cloned().unwrap_or_default();

    // Una fila con `videoId` es una pista, venga como venga envuelta. Si no lo
    // tiene, es un enlace a otra pagina y hay que mirar su endpoint.
    let (kind, id) = match crate::search::extract_video_id(r) {
        Some(id) => (ItemKind::Track, id),
        None => {
            let mut endpoints = Vec::new();
            collect_by_key(r, "navigationEndpoint", &mut endpoints);
            endpoints.into_iter().find_map(target)?
        }
    };

    Some(ShelfItem {
        kind,
        id,
        title,
        duration: crate::search::extract_duration(&subtitle),
        subtitle: crate::search::clean_subtitle(&subtitle),
        thumbnail: extract_thumbnail(r),
    })
}

/// Adonde lleva un endpoint: a reproducir algo, o a otra pagina.
fn target(endpoint: &Value) -> Option<(ItemKind, String)> {
    if let Some(id) = endpoint
        .get("watchEndpoint")
        .and_then(|w| w.get("videoId"))
        .and_then(Value::as_str)
    {
        return Some((ItemKind::Track, id.to_string()));
    }

    let browse = endpoint.get("browseEndpoint")?;
    let id = browse.get("browseId").and_then(Value::as_str)?;

    // `pageType` es lo que dice YouTube que es. El prefijo del id es el
    // respaldo para cuando la respuesta no lo trae.
    let page_type = crate::search::find_str(browse, "pageType").unwrap_or("");
    let kind = match page_type {
        t if t.ends_with("_ALBUM") => ItemKind::Album,
        t if t.ends_with("_ARTIST") => ItemKind::Artist,
        t if t.ends_with("_PLAYLIST") => ItemKind::Playlist,
        _ if id.starts_with("MPRE") => ItemKind::Album,
        _ if id.starts_with("UC") => ItemKind::Artist,
        _ if id.starts_with("VL") => ItemKind::Playlist,
        _ => return None, // tipos que aun no sabemos abrir
    };
    Some((kind, id.to_string()))
}

fn non_empty(s: String) -> Option<String> {
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carrusel_con(titulo: &str, contenido: Value) -> Value {
        json!({ "musicCarouselShelfRenderer": {
            "header": { "musicCarouselShelfBasicHeaderRenderer": {
                "title": { "runs": [{ "text": titulo }] }
            }},
            "contents": contenido,
        }})
    }

    fn tarjeta(titulo: &str, video_id: &str) -> Value {
        json!({ "musicTwoRowItemRenderer": {
            "title": { "runs": [{ "text": titulo }] },
            "subtitle": { "runs": [{ "text": "Cancion \u{2022} Un Artista" }] },
            "thumbnailRenderer": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                { "url": "https://ejemplo/pequena.jpg" },
                { "url": "https://ejemplo/grande.jpg" }
            ]}}},
            "navigationEndpoint": { "watchEndpoint": { "videoId": video_id } },
        }})
    }

    #[test]
    fn lee_un_carrusel_de_tarjetas() {
        let page = parse_page(&json!({ "da": { "igual": { "lo": { "hondo": [
            carrusel_con("Volver a escuchar", json!([tarjeta("Mi Cancion", "abcdefghijk")]))
        ]}}}}));

        assert_eq!(page.shelves.len(), 1);
        assert_eq!(page.shelves[0].title, "Volver a escuchar");
        let item = &page.shelves[0].items[0];
        assert_eq!(item.kind, ItemKind::Track);
        assert_eq!(item.id, "abcdefghijk");
        assert_eq!(item.title, "Mi Cancion");
        assert_eq!(item.thumbnail.as_deref(), Some("https://ejemplo/grande.jpg"));
    }

    #[test]
    fn conserva_el_orden_de_las_estanterias() {
        // El feed de inicio se pinta en el orden en que YouTube lo manda.
        let page = parse_page(&json!({ "contents": [
            carrusel_con("Primera", json!([tarjeta("A", "aaaaaaaaaaa")])),
            carrusel_con("Segunda", json!([tarjeta("B", "bbbbbbbbbbb")])),
            carrusel_con("Tercera", json!([tarjeta("C", "ccccccccccc")])),
        ]}));

        let titulos: Vec<&str> = page.shelves.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titulos, ["Primera", "Segunda", "Tercera"]);
    }

    #[test]
    fn los_elementos_no_se_cuentan_ademas_como_estanterias() {
        // El recorrido no baja dentro de una estanteria. Si bajara, esta
        // respuesta daria estanterias de mas, vacias y sin titulo.
        let page = parse_page(&json!({ "contents": [
            carrusel_con("Unica", json!([tarjeta("A", "aaaaaaaaaaa"), tarjeta("B", "bbbbbbbbbbb")]))
        ]}));
        assert_eq!(page.shelves.len(), 1);
        assert_eq!(page.shelves[0].items.len(), 2);
    }

    #[test]
    fn distingue_album_artista_y_playlist() {
        let destino = |page_type: &str, id: &str| {
            target(&json!({ "browseEndpoint": {
                "browseId": id,
                "browseEndpointContextSupportedConfigs": {
                    "browseEndpointContextMusicConfig": { "pageType": page_type }
                }
            }}))
        };

        assert_eq!(destino("MUSIC_PAGE_TYPE_ALBUM", "MPREb_x").unwrap().0, ItemKind::Album);
        assert_eq!(destino("MUSIC_PAGE_TYPE_ARTIST", "UCxxxx").unwrap().0, ItemKind::Artist);
        assert_eq!(destino("MUSIC_PAGE_TYPE_PLAYLIST", "VLPLxx").unwrap().0, ItemKind::Playlist);
    }

    #[test]
    fn sin_page_type_se_deduce_del_prefijo_del_id() {
        // YouTube no siempre manda `pageType`; el prefijo del id es el respaldo.
        let destino =
            |id: &str| target(&json!({ "browseEndpoint": { "browseId": id } })).map(|t| t.0);

        assert_eq!(destino("MPREb_abc"), Some(ItemKind::Album));
        assert_eq!(destino("UC123"), Some(ItemKind::Artist));
        assert_eq!(destino("VLPL123"), Some(ItemKind::Playlist));
        assert_eq!(destino("FEmusic_algo_nuevo"), None);
    }

    #[test]
    fn lee_una_lista_vertical_y_le_saca_la_duracion() {
        let page = parse_page(&json!({ "musicShelfRenderer": {
            "title": { "runs": [{ "text": "Canciones populares" }] },
            "contents": [{ "musicResponsiveListItemRenderer": {
                "playlistItemData": { "videoId": "abcdefghijk" },
                "flexColumns": [
                    { "musicResponsiveListItemFlexColumnRenderer":
                        { "text": { "runs": [{ "text": "Una Pista" }] } } },
                    { "musicResponsiveListItemFlexColumnRenderer":
                        { "text": { "runs": [{ "text": "Artista \u{2022} 3:45" }] } } }
                ]
            }}]
        }}));

        assert_eq!(page.shelves[0].title, "Canciones populares");
        let item = &page.shelves[0].items[0];
        assert_eq!(item.title, "Una Pista");
        assert_eq!(item.duration.as_deref(), Some("3:45"));
        assert_eq!(item.subtitle, "Artista");
    }

    #[test]
    fn descarta_estanterias_vacias() {
        // Una estanteria sin nada dentro solo pintaria un titulo huerfano.
        let page = parse_page(&json!({ "contents": [
            carrusel_con("Vacia", json!([])),
            carrusel_con("Con algo", json!([tarjeta("A", "aaaaaaaaaaa")])),
        ]}));
        let titulos: Vec<&str> = page.shelves.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titulos, ["Con algo"]);
    }

    #[test]
    fn lee_la_cabecera_de_un_artista() {
        let page = parse_page(&json!({ "header": { "musicImmersiveHeaderRenderer": {
            "title": { "runs": [{ "text": "SABAI" }] },
            "subtitle": { "runs": [{ "text": "1,25 M de oyentes mensuales" }] },
            "thumbnail": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                { "url": "https://ejemplo/artista.jpg" }
            ]}}}
        }}}));

        assert_eq!(page.title.as_deref(), Some("SABAI"));
        assert_eq!(page.subtitle.as_deref(), Some("1,25 M de oyentes mensuales"));
        assert_eq!(page.thumbnail.as_deref(), Some("https://ejemplo/artista.jpg"));
    }

    #[test]
    fn una_pista_no_se_duplica_dentro_de_la_misma_estanteria() {
        let page = parse_page(&json!({ "contents": [carrusel_con(
            "Repetida",
            json!([tarjeta("A", "aaaaaaaaaaa"), tarjeta("A", "aaaaaaaaaaa")])
        )]}));
        assert_eq!(page.shelves[0].items.len(), 1);
    }
}
