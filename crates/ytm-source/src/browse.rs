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
use crate::search::{
    collect_by_key, extract_thumbnail, flex_column_texts, parse_continuation, runs_text,
};

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
    /// Canal del artista, si la fila lo enlaza. Para "Ir al artista".
    #[serde(default)]
    pub artist_id: Option<String>,
    /// Album al que pertenece, si la fila lo enlaza.
    #[serde(default)]
    pub album_id: Option<String>,
}

/// Una fila del feed, con su titulo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shelf {
    pub title: String,
    pub items: Vec<ShelfItem>,
}

/// Un boton de navegacion: las pastillas de colores de Explorar.
///
/// Son su propio renderer y no tarjetas: llevan texto, un destino y una franja
/// de color, y nada mas. Los tres de arriba —Novedades, Rankings, Estados de
/// animo— son los mismos pero sin franja.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavButton {
    pub label: String,
    pub browse_id: String,
    /// Opaco, lo define YouTube. Distingue una categoria de otra.
    pub params: Option<String>,
    /// `#rrggbb` de la franja izquierda, si la tiene.
    pub stripe: Option<String>,
}

/// Una pagina de `browse` ya normalizada.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowsePage {
    /// Nombre del artista o del album. El feed de inicio no tiene.
    pub title: Option<String>,
    /// "1,25 M de oyentes mensuales", "Album · 2023", y demas.
    pub subtitle: Option<String>,
    /// La segunda linea: "356 k vistas · 551 pistas · Mas de 31 horas".
    pub second_subtitle: Option<String>,
    /// Descripcion que escribio quien hizo la playlist.
    pub description: Option<String>,
    /// Imagen de cabecera.
    pub thumbnail: Option<String>,
    pub shelves: Vec<Shelf>,
    /// Botones de navegacion. Solo Explorar y las categorias los traen.
    #[serde(default)]
    pub buttons: Vec<NavButton>,
    /// Filtros de estado de animo del inicio ("Energia", "Dormir"...).
    #[serde(default)]
    pub chips: Vec<crate::search::SearchChip>,
    /// Token de la siguiente tanda de pistas, si la lista no cabe en una.
    ///
    /// YouTube corta las playlists en paginas de 100. Una de 551 pistas llega
    /// en seis respuestas, y sin esto se quedaba en las 100 primeras.
    pub continuation: Option<String>,
}

impl InnerTube {
    /// Contexto de `browse`, con el identificador de sesion si ya lo tenemos.
    fn contexto_browse(&self) -> Value {
        let mut client = json!({
            "clientName": WEB_REMIX.client_name,
            "clientVersion": WEB_REMIX.client_version,
            "hl": "es",
            "gl": "US",
        });
        if let Some(v) = self.visitor() {
            client["visitorData"] = json!(v);
        }
        json!({ "client": client })
    }

    /// Pide una pagina a `browse`. Anonimo: no lleva cookies.
    ///
    /// `params` es opaco y lo define YouTube (lo usan las pestanias de artista).
    pub async fn browse(&self, browse_id: &str, params: Option<&str>) -> Result<BrowsePage> {
        let mut body = json!({
            "context": self.contexto_browse(),
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
        // Antes de nada: las continuaciones de esta pagina lo necesitaran.
        self.remember_visitor(&json);
        Ok(parse_page(&json))
    }

    /// Siguiente tanda de una pagina ya abierta.
    ///
    /// Igual que en la busqueda, la continuacion viaja en la QUERY y no en el
    /// cuerpo: mandada en el JSON, este endpoint devuelve la primera pagina
    /// otra vez sin dar error, que es peor que fallar.
    pub async fn browse_more(&self, continuation: &str) -> Result<BrowsePage> {
        let url = format!("{BROWSE_URL}?ctoken={c}&continuation={c}&type=next", c = continuation);
        let body = json!({ "context": self.contexto_browse() });

        let mut peticion = self
            .http_ref()
            .post(&url)
            .header("User-Agent", WEB_REMIX.user_agent)
            .header("X-YouTube-Client-Name", WEB_REMIX.client_name_id.to_string())
            .header("X-YouTube-Client-Version", WEB_REMIX.client_version)
            .header("Content-Type", "application/json")
            .header("Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/");
        // La sesion viaja tambien en la cabecera. Con una sola de las dos, el
        // inicio devuelve una cascara vacia en vez de la siguiente tanda.
        if let Some(v) = self.visitor() {
            peticion = peticion.header("X-Goog-Visitor-Id", v);
        }

        let res = peticion
            .json(&body)
            .send()
            .await
            .context("fallo la continuacion de browse")?
            .error_for_status()
            .context("el servidor rechazo la continuacion de browse")?;

        let json: Value = res.json().await.context("continuacion de browse ilegible")?;
        Ok(parse_continued(&json))
    }

    /// Feed de inicio.
    pub async fn home(&self) -> Result<BrowsePage> {
        self.browse(HOME, None).await
    }
}

/// Normaliza una respuesta de `browse` sin depender de la forma del arbol.
pub fn parse_page(root: &Value) -> BrowsePage {
    let crudas = collect_shelves(root);

    // Primero dentro de las estanterias: ahi esta el token de una lista larga,
    // el que sigue leyendo pistas. Si ninguna lo trae, se mira la pagina
    // entera, que es donde el inicio cuelga el suyo — el que trae MAS
    // ESTANTERIAS. Son cosas distintas y por eso se buscan en este orden: una
    // lista de 551 pistas tiene los dos, y el que importa es el de las pistas.
    let continuation = crudas
        .iter()
        .find_map(|s| parse_continuation(s))
        .or_else(|| parse_continuation(root));

    let mut page = BrowsePage {
        shelves: crudas
            .into_iter()
            .filter_map(|s| {
                let shelf = parse_shelf(s);
                // Una estanteria sin nada dentro solo pinta un titulo huerfano.
                (!shelf.items.is_empty()).then_some(shelf)
            })
            .collect(),
        buttons: parse_buttons(root),
        chips: parse_chips_browse(root),
        continuation,
        ..Default::default()
    };

    if let Some(header) = find_header(root) {
        page.title = non_empty(runs_text(header.get("title")));
        page.subtitle = non_empty(header_subtitle(header));
        page.second_subtitle = non_empty(runs_text(header.get("secondSubtitle")));
        page.description = non_empty(runs_text(header.get("description")))
            .or_else(|| non_empty(runs_text(header.get("descriptionShelfRenderer"))));
        page.thumbnail = extract_thumbnail(header);
    }
    page
}

/// Normaliza una respuesta de CONTINUACION, que no tiene la forma de una pagina.
///
/// Comprobado contra la lista real de 551 pistas: la tanda 2 no llega envuelta
/// en ninguna estanteria, sino como
/// `onResponseReceivedActions[].appendContinuationItemsAction.continuationItems`,
/// un array pelado de filas. Por eso [`parse_page`] la leia vacia y la lista se
/// quedaba en 100.
///
/// Como aqui no hay estanterias que respetar, se recogen las filas alla donde
/// esten y se devuelven en una sola. Eso vale para las dos formas que usa
/// YouTube — esta y la antigua `musicPlaylistShelfContinuation` — sin tener que
/// distinguirlas.
pub fn parse_continued(root: &Value) -> BrowsePage {
    // Las dos formas que usa YouTube, y hay que distinguirlas:
    //
    //   - El inicio manda ESTANTERIAS enteras, con su titulo. Van detras de las
    //     que ya habia, cada una la suya.
    //   - Una lista larga manda FILAS sueltas, sin envoltorio. Van al final de
    //     la lista que ya se estaba leyendo.
    //
    // Aplanarlo todo, como se hacia, juntaba "Sube el volumen", "Playlists de
    // Urbano Latino" y "Exitos de hoy" en una sola estanteria sin nombre.
    let estanterias = collect_shelves(root);
    if !estanterias.is_empty() {
        return BrowsePage {
            shelves: estanterias
                .into_iter()
                .map(parse_shelf)
                .filter(|s| !s.items.is_empty())
                .collect(),
            continuation: parse_continuation(root),
            ..Default::default()
        };
    }

    let mut rows = Vec::new();
    collect_by_key(root, "musicResponsiveListItemRenderer", &mut rows);
    let mut cards = Vec::new();
    collect_by_key(root, "musicTwoRowItemRenderer", &mut cards);

    let items: Vec<ShelfItem> = rows
        .into_iter()
        .filter_map(parse_row)
        .chain(cards.into_iter().filter_map(parse_card))
        .collect();

    BrowsePage {
        shelves: if items.is_empty() { Vec::new() } else { vec![Shelf { title: String::new(), items }] },
        continuation: parse_continuation(root),
        ..Default::default()
    }
}

/// Los filtros de arriba del inicio.
///
/// Son el mismo renderer que los del buscador pero con OTRO destino: alli el
/// `params` cuelga de un `searchEndpoint` y aqui de un `browseEndpoint` que
/// vuelve a `FEmusic_home`. Por eso no sirve el parser de `search.rs`: mira el
/// sitio equivocado y devolvia una lista vacia.
fn parse_chips_browse(root: &Value) -> Vec<crate::search::SearchChip> {
    let mut nodos = Vec::new();
    collect_by_key(root, "chipCloudChipRenderer", &mut nodos);

    let mut out = Vec::new();
    let mut vistos = std::collections::HashSet::new();
    for c in nodos {
        let label = runs_text(c.get("text"));
        let params = c
            .get("navigationEndpoint")
            .and_then(|e| e.get("browseEndpoint"))
            .and_then(|e| e.get("params"))
            .and_then(Value::as_str);
        if let (false, Some(params)) = (label.is_empty(), params) {
            if vistos.insert(params.to_string()) {
                out.push(crate::search::SearchChip { label, params: params.to_string() });
            }
        }
    }
    out
}

/// Los botones de navegacion, en el orden en que llegan.
fn parse_buttons(root: &Value) -> Vec<NavButton> {
    let mut crudos = Vec::new();
    collect_by_key(root, "musicNavigationButtonRenderer", &mut crudos);

    crudos
        .into_iter()
        .filter_map(|b| {
            let label = runs_text(b.get("buttonText"));
            if label.is_empty() {
                return None;
            }
            let destino = b.get("clickCommand")?.get("browseEndpoint")?;
            Some(NavButton {
                label,
                browse_id: destino.get("browseId").and_then(Value::as_str)?.to_string(),
                params: destino.get("params").and_then(Value::as_str).map(str::to_string),
                stripe: b
                    .get("solid")
                    .and_then(|s| s.get("leftStripeColor"))
                    .and_then(Value::as_u64)
                    // Llega como un entero ARGB. El alfa sobra: siempre es opaco.
                    .map(|c| format!("#{:06x}", c & 0x00FF_FFFF)),
            })
        })
        .collect()
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
    for key in ["subtitle", "straplineTextOne", "secondSubtitle"] {
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
                // `musicPlaylistShelfRenderer` es el de las pistas de una
                // playlist. Sin el, una lista abierta salia con cabecera y sin
                // una sola cancion.
                if k == "musicCarouselShelfRenderer"
                    || k == "musicShelfRenderer"
                    || k == "musicPlaylistShelfRenderer"
                {
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
    let (artist_id, album_id) = enlaces_de_columnas(r.get("subtitle"));

    Some(ShelfItem {
        kind,
        id,
        title,
        subtitle: runs_text(r.get("subtitle")),
        thumbnail: extract_thumbnail(r),
        duration: None,
        artist_id,
        album_id,
    })
}

/// Fila: miniatura y columnas de texto. Mismo formato que la busqueda.
pub(crate) fn parse_row(r: &Value) -> Option<ShelfItem> {
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

    // Las columnas enlazan al artista y al album de la pista. Se distinguen
    // por su `pageType`, no por el orden: no siempre estan los dos.
    let (artist_id, album_id) = enlaces_de_columnas(r.get("flexColumns"));

    Some(ShelfItem {
        kind,
        id,
        title,
        duration: crate::search::extract_duration(&subtitle),
        subtitle: crate::search::clean_subtitle(&subtitle),
        thumbnail: extract_thumbnail(r),
        artist_id,
        album_id,
    })
}

/// Canal del artista y album que enlazan las columnas de una fila.
fn enlaces_de_columnas(columnas: Option<&Value>) -> (Option<String>, Option<String>) {
    let Some(columnas) = columnas else {
        return (None, None);
    };
    // `navigationEndpoint` y no `browseEndpoint`: `target` espera el envoltorio,
    // que es donde distingue entre reproducir algo y abrir una pagina.
    let mut endpoints = Vec::new();
    collect_by_key(columnas, "navigationEndpoint", &mut endpoints);

    let mut artista = None;
    let mut album = None;
    for e in endpoints {
        let Some((kind, id)) = target(e) else { continue };
        match kind {
            ItemKind::Artist if artista.is_none() => artista = Some(id),
            ItemKind::Album if album.is_none() => album = Some(id),
            _ => {}
        }
    }
    (artista, album)
}

/// Adonde lleva un endpoint: a reproducir algo, o a otra pagina.
pub(crate) fn target(endpoint: &Value) -> Option<(ItemKind, String)> {
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
    fn una_fila_trae_el_enlace_a_su_artista() {
        // Forma real, comprobada contra una busqueda: el canal del artista
        // cuelga de una de las columnas de texto, no de la fila.
        let artista = json!({ "browseEndpoint": {
            "browseId": "UClYV6hHlupm_S_ObS1W-DYw",
            "browseEndpointContextSupportedConfigs": {
                "browseEndpointContextMusicConfig": { "pageType": "MUSIC_PAGE_TYPE_ARTIST" }
            }
        }});
        let album = json!({ "browseEndpoint": {
            "browseId": "MPREb_despues",
            "browseEndpointContextSupportedConfigs": {
                "browseEndpointContextMusicConfig": { "pageType": "MUSIC_PAGE_TYPE_ALBUM" }
            }
        }});

        let page = parse_page(&json!({ "musicShelfRenderer": {
            "contents": [{ "musicResponsiveListItemRenderer": {
                "playlistItemData": { "videoId": "abcdefghijk" },
                "flexColumns": [
                    { "musicResponsiveListItemFlexColumnRenderer":
                        { "text": { "runs": [{ "text": "Blinding Lights" }] } } },
                    { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [
                        { "text": "The Weeknd", "navigationEndpoint": artista },
                        { "text": " - " },
                        { "text": "After Hours", "navigationEndpoint": album }
                    ]}}}
                ]
            }}]
        }}));

        let item = &page.shelves[0].items[0];
        assert_eq!(item.kind, ItemKind::Track, "sigue siendo una pista");
        assert_eq!(item.id, "abcdefghijk");
        assert_eq!(item.artist_id.as_deref(), Some("UClYV6hHlupm_S_ObS1W-DYw"));
        assert_eq!(item.album_id.as_deref(), Some("MPREb_despues"));
    }

    #[test]
    fn una_fila_sin_enlaces_no_se_inventa_ninguno() {
        let page = parse_page(&json!({ "musicShelfRenderer": {
            "contents": [{ "musicResponsiveListItemRenderer": {
                "playlistItemData": { "videoId": "abcdefghijk" },
                "flexColumns": [{ "musicResponsiveListItemFlexColumnRenderer":
                    { "text": { "runs": [{ "text": "Suelta" }] } } }]
            }}]
        }}));
        let item = &page.shelves[0].items[0];
        assert!(item.artist_id.is_none() && item.album_id.is_none());
    }

    #[test]
    fn los_filtros_del_inicio_no_son_los_del_buscador() {
        // Mismo renderer, otro destino: en el inicio el `params` cuelga de un
        // `browseEndpoint` y en el buscador de un `searchEndpoint`. Usar el
        // parser del buscador aqui devolvia una lista vacia.
        let page = parse_page(&json!({ "chipCloudChipRenderer": {
            "text": { "runs": [{ "text": "Energia" }] },
            "navigationEndpoint": { "browseEndpoint": {
                "browseId": "FEmusic_home",
                "params": "ggM8SgQICRAD"
            }}
        }}));
        assert_eq!(page.chips.len(), 1);
        assert_eq!(page.chips[0].label, "Energia");
        assert_eq!(page.chips[0].params, "ggM8SgQICRAD");
    }

    #[test]
    fn lee_las_pastillas_de_explorar() {
        // Forma real, comprobada contra FEmusic_explore: las de arriba llevan
        // icono y las de generos, una franja de color en un entero ARGB.
        let page = parse_page(&json!({ "contents": [
            { "musicNavigationButtonRenderer": {
                "buttonText": { "runs": [{ "text": "Novedades" }] },
                "clickCommand": { "browseEndpoint": { "browseId": "FEmusic_new_releases" } },
                "iconStyle": { "icon": { "iconType": "MUSIC_NEW_RELEASE" } }
            }},
            { "musicNavigationButtonRenderer": {
                "buttonText": { "runs": [{ "text": "Dormir" }] },
                "solid": { "leftStripeColor": 4286267099u32 },
                "clickCommand": { "browseEndpoint": {
                    "browseId": "FEmusic_moods_and_genres_category",
                    "params": "ggMPOg1uX1MxaFQ3Z0JMZkN4"
                }}
            }}
        ]}));

        assert_eq!(page.buttons.len(), 2);
        assert_eq!(page.buttons[0].label, "Novedades");
        assert_eq!(page.buttons[0].browse_id, "FEmusic_new_releases");
        assert!(page.buttons[0].stripe.is_none(), "las de arriba no llevan franja");

        let dormir = &page.buttons[1];
        assert_eq!(dormir.params.as_deref(), Some("ggMPOg1uX1MxaFQ3Z0JMZkN4"));
        // El alfa del entero ARGB se descarta: solo interesa el color.
        assert_eq!(dormir.stripe.as_deref(), Some("#7b3edb")); // 0xFF7B3EDB
    }

    #[test]
    fn un_boton_sin_destino_no_se_pinta() {
        // Una pastilla que no lleva a ningun sitio es decoracion.
        let page = parse_page(&json!({ "musicNavigationButtonRenderer": {
            "buttonText": { "runs": [{ "text": "Suelto" }] }
        }}));
        assert!(page.buttons.is_empty());
    }

    #[test]
    fn una_lista_larga_deja_token_para_la_siguiente_tanda() {
        // YouTube corta las playlists en paginas de 100. Sin leer esto, una
        // lista de 551 pistas se quedaba en las 100 primeras.
        let page = parse_page(&json!({ "musicPlaylistShelfRenderer": {
            "contents": [{ "musicResponsiveListItemRenderer": {
                "playlistItemData": { "videoId": "abcdefghijk" },
                "flexColumns": [{ "musicResponsiveListItemFlexColumnRenderer":
                    { "text": { "runs": [{ "text": "Pista 100" }] } } }]
            }}],
            "continuations": [{ "nextContinuationData": { "continuation": "SIGUE" } }]
        }}));
        assert_eq!(page.continuation.as_deref(), Some("SIGUE"));
    }

    #[test]
    fn la_segunda_tanda_llega_sin_estanteria() {
        // Forma real de la respuesta, comprobada contra una lista de 551
        // pistas: filas peladas dentro de `appendContinuationItemsAction`. Sin
        // un parser propio, `parse_page` no veia ninguna estanteria y la lista
        // se quedaba en las 100 primeras.
        let page = parse_continued(&json!({ "onResponseReceivedActions": [{
            "appendContinuationItemsAction": { "continuationItems": [
                { "musicResponsiveListItemRenderer": {
                    "playlistItemData": { "videoId": "bbbbbbbbbbb" },
                    "flexColumns": [{ "musicResponsiveListItemFlexColumnRenderer":
                        { "text": { "runs": [{ "text": "Pista 101" }] } } }]
                }},
                { "continuationItemRenderer": { "continuationEndpoint": {
                    "continuationCommand": { "token": "TANDA_3" }
                }}}
            ]}
        }]}));

        assert_eq!(page.shelves.len(), 1);
        assert_eq!(page.shelves[0].items[0].title, "Pista 101");
        assert_eq!(page.continuation.as_deref(), Some("TANDA_3"));
    }

    #[test]
    fn la_ultima_tanda_no_deja_token() {
        let page = parse_continued(&json!({ "onResponseReceivedActions": [{
            "appendContinuationItemsAction": { "continuationItems": [
                { "musicResponsiveListItemRenderer": {
                    "playlistItemData": { "videoId": "ccccccccccc" },
                    "flexColumns": [{ "musicResponsiveListItemFlexColumnRenderer":
                        { "text": { "runs": [{ "text": "Pista 551" }] } } }]
                }}
            ]}
        }]}));
        assert_eq!(page.shelves[0].items.len(), 1);
        assert!(page.continuation.is_none());
    }

    #[test]
    fn el_inicio_usa_el_token_de_la_pagina_para_pedir_mas_estanterias() {
        let page = parse_page(&json!({
            "contents": [carrusel_con("Con algo", json!([tarjeta("A", "aaaaaaaaaaa")]))],
            "continuations": [{ "nextContinuationData": { "continuation": "MAS_FILAS" } }]
        }));
        assert_eq!(page.continuation.as_deref(), Some("MAS_FILAS"));
    }

    #[test]
    fn en_una_lista_larga_manda_el_token_de_las_pistas() {
        // Una playlist trae los dos: el de la lista de secciones y el de sus
        // propias pistas. Coger el equivocado dejaba la lista en 100.
        let page = parse_page(&json!({
            "musicPlaylistShelfRenderer": {
                "contents": [
                    { "musicResponsiveListItemRenderer": {
                        "playlistItemData": { "videoId": "aaaaaaaaaaa" },
                        "flexColumns": [{ "musicResponsiveListItemFlexColumnRenderer":
                            { "text": { "runs": [{ "text": "Pista" }] } } }]
                    }},
                    { "continuationItemRenderer": { "continuationEndpoint": {
                        "continuationCommand": { "token": "MAS_PISTAS" }
                    }}}
                ]
            },
            "continuations": [{ "nextContinuationData": { "continuation": "MAS_SECCIONES" } }]
        }));
        assert_eq!(page.continuation.as_deref(), Some("MAS_PISTAS"));
    }

    #[test]
    fn la_continuacion_del_inicio_llega_como_estanterias_con_nombre() {
        // Y no aplanada en una sola: son filas distintas del feed.
        let page = parse_continued(&json!({ "continuationContents": {
            "sectionListContinuation": { "contents": [
                carrusel_con("Sube el volumen", json!([tarjeta("A", "aaaaaaaaaaa")])),
                carrusel_con("Exitos de hoy", json!([tarjeta("B", "bbbbbbbbbbb")])),
            ]}
        }}));
        let titulos: Vec<&str> = page.shelves.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titulos, ["Sube el volumen", "Exitos de hoy"]);
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
