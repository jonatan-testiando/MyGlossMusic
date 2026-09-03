import { createSignal, createEffect, on } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import {
  api,
  parseDuration,
  thumbAt,
  thumbUrl,
  thumbFallback,
  type Lyrics,
  type Palette,
  type PlaybackState,
  type BrowsePage,
  type ExtractorStatus,
  type Playlist,
  type SavedTrack,
  type Track,
  type SearchChip,
  type SearchResult,
  type ShelfItem,
} from "./api";

const EMPTY_STATE: PlaybackState = {
  track: null,
  playing: false,
  loading: false,
  positionMs: 0,
  durationMs: 0,
  volume: 1,
  buffered: 0,
  queue: [],
  queueRev: 0,
  queueIndex: 0,
  repeat: "off",
  shuffle: false,
  error: null,
};

const DEFAULT_PALETTE: Palette = {
  stops: [
    { color: "#3b3358", weight: 0.5 },
    { color: "#5b4a8a", weight: 0.3 },
    { color: "#2a2740", weight: 0.2 },
  ],
  background: "#12101a",
  backgroundAlt: "#1c1826",
  accent: "#8b7fd4",
  foreground: "#f4f2fa",
  isLight: false,
};

export const [playback, setPlayback] = createStore<PlaybackState>(EMPTY_STATE);
export const [palette, setPalette] = createSignal<Palette>(DEFAULT_PALETTE);
export const [results, setResults] = createSignal<ShelfItem[]>([]);
/** Los filtros que ofrece YouTube para la búsqueda actual. */
export const [searchChips, setSearchChips] = createSignal<SearchChip[]>([]);
/** Filtro activo. `null` es "todo mezclado". */
export const [searchFilter, setSearchFilter] = createSignal<string | null>(null);
/** Token de la siguiente página, o `null` si ya no hay más. */
export const [searchMoreToken, setSearchMoreToken] = createSignal<string | null>(null);
export const [loadingMore, setLoadingMore] = createSignal(false);
/**
 * Filtro con el que seguir cuando "Todo" se acaba.
 *
 * Medido sobre una búsqueda real: "Todo" devuelve 27 resultados y NO trae
 * token de continuación — el tope lo pone YouTube, no nosotros. La profundidad
 * está en los filtros: "Canciones" da 100 en cinco páginas y sigue.
 *
 * Así que al agotarse "Todo" se continúa por el filtro de canciones. Es una
 * decisión nuestra, no algo que haga YouTube: su interfaz te obliga a pulsar la
 * pestaña.
 */
const [searchOverflow, setSearchOverflow] = createSignal<string | null>(null);
export const [searching, setSearching] = createSignal(false);
export const [query, setQuery] = createSignal("");
export type View = "home" | "search" | "library" | "diagnostics" | "browse" | "playlist";

export const [view, setView] = createSignal<View>("home");

/** Página de artista, álbum o playlist que se está viendo. */
export const [browsePage, setBrowsePage] = createSignal<BrowsePage | null>(null);
export const [browseLoading, setBrowseLoading] = createSignal(false);
/** Id de la página abierta, para poder guardarla o volver a pedirla. */
export const [browseId, setBrowseId] = createSignal<string | null>(null);
export const [savingBrowse, setSavingBrowse] = createSignal(false);

/**
 * Canal del artista de lo que suena, para la pestaña SIMILARES.
 *
 * Lo deja la radio al cargarse. La pestaña "Relacionado" de YouTube Music cuelga
 * de un `browseId` con prefijo `MPTR` que solo responde dentro del contexto de
 * sesión de `next`: pedido suelto devuelve una respuesta vacía de 2 KB,
 * comprobado. Así que se ofrece la página del artista, que es real y útil, en
 * vez de una pestaña que no hace nada.
 */
export const [relatedArtistId, setRelatedArtistId] = createSignal<string | null>(null);

/* --------------------------------------------------------------- Playlists */

export const [playlists, setPlaylists] = createSignal<Playlist[]>([]);
/** Playlist abierta, con sus pistas. */
export const [openPlaylist, setOpenPlaylist] = createSignal<
  { lista: Playlist; tracks: SavedTrack[] } | null
>(null);
/** Pista para la que se está eligiendo playlist, o `null` si el diálogo está cerrado. */
export const [addingTo, setAddingTo] = createSignal<Partial<Track> | null>(null);
/** `true` mientras se pide un nombre para una playlist nueva. */
export const [creatingPlaylist, setCreatingPlaylist] = createSignal(false);

/* ----------------------------------------------------------------- Ajustes */

export const [settingsOpen, setSettingsOpen] = createSignal(false);
export const [extractor, setExtractor] = createSignal<ExtractorStatus | null>(null);

export async function refreshExtractor() {
  try {
    setExtractor(await api.extractorStatus());
  } catch {
    setExtractor(null);
  }
}

/**
 * Fondo animado encendido o apagado.
 *
 * En localStorage y no en SQLite porque lo lee la interfaz al montar, y una
 * ida y vuelta al backend haría que el fondo arrancara y se apagara a la vista.
 */
const AMBIENT_KEY = "myglossmusic.ambient";

export const [ambientEnabled, setAmbientEnabledSignal] = createSignal(
  (() => {
    try {
      return localStorage.getItem(AMBIENT_KEY) !== "0";
    } catch {
      return true;
    }
  })(),
);

export function setAmbientEnabled(on: boolean) {
  setAmbientEnabledSignal(on);
  try {
    localStorage.setItem(AMBIENT_KEY, on ? "1" : "0");
  } catch {
    // Sin persistencia se pierde entre sesiones; la sesión actual funciona.
  }
}

export async function refreshPlaylists() {
  try {
    setPlaylists(await api.playlists());
  } catch (e) {
    console.error("no se pudieron leer las playlists", e);
  }
}

export async function createPlaylist(name: string): Promise<number | null> {
  try {
    const id = await api.createPlaylist(name);
    await refreshPlaylists();
    return id;
  } catch (e) {
    console.error("no se pudo crear la playlist", e);
    return null;
  }
}

export async function deletePlaylist(id: number) {
  try {
    await api.deletePlaylist(id);
    // Si estaba abierta, se cierra: dejarla en pantalla mostraría una lista
    // que ya no existe.
    if (openPlaylist()?.lista.id === id) setOpenPlaylist(null);
    await refreshPlaylists();
  } catch (e) {
    console.error("no se pudo borrar la playlist", e);
  }
}

export async function showPlaylist(lista: Playlist) {
  navegar({ view: "playlist" });
  try {
    setOpenPlaylist({ lista, tracks: await api.playlistTracks(lista.id) });
  } catch (e) {
    console.error("no se pudo abrir la playlist", e);
    setOpenPlaylist({ lista, tracks: [] });
  }
}

export async function addTrackToPlaylist(id: number, track: Partial<Track>) {
  try {
    await api.addToPlaylist(id, track);
    await refreshPlaylists();
    // Si es la que está abierta, se refresca para que la pista aparezca ya.
    const abierta = openPlaylist();
    if (abierta?.lista.id === id) showPlaylist(abierta.lista);
  } catch (e) {
    console.error("no se pudo anadir a la playlist", e);
  }
}

export async function removeTrackFromPlaylist(id: number, videoId: string) {
  try {
    await api.removeFromPlaylist(id, videoId);
    await refreshPlaylists();
    const abierta = openPlaylist();
    if (abierta?.lista.id === id) showPlaylist(abierta.lista);
  } catch (e) {
    console.error("no se pudo quitar de la playlist", e);
  }
}

/**
 * Historial de navegación.
 *
 * Hace falta de verdad desde que hay páginas de artista y de álbum: sin él,
 * "atrás" no puede devolverte a la búsqueda de la que saliste. Las flechas de
 * la cabecera hacían antes un apaño — atrás llevaba al inicio, pasara lo que
 * pasara.
 */
type Destino = { view: View; browseId?: string; titulo?: string };

const [historial, setHistorial] = createSignal<Destino[]>([{ view: "home" }]);
const [posicion, setPosicion] = createSignal(0);

export const puedeAtras = () => posicion() > 0;
export const puedeAdelante = () => posicion() < historial().length - 1;

/** Va a un destino nuevo, descartando lo que hubiera hacia delante. */
export function navegar(d: Destino) {
  const hasta = historial().slice(0, posicion() + 1);
  setHistorial([...hasta, d]);
  setPosicion(hasta.length);
  aplicar(d);
}

export function atras() {
  if (!puedeAtras()) return;
  setPosicion(posicion() - 1);
  aplicar(historial()[posicion()]);
}

export function adelante() {
  if (!puedeAdelante()) return;
  setPosicion(posicion() + 1);
  aplicar(historial()[posicion()]);
}

function aplicar(d: Destino) {
  setPlayerViewOpen(false);
  setView(d.view);
  if (d.view === "browse" && d.browseId) cargarBrowse(d.browseId);
}

/** Abre una página de artista, álbum o playlist. */
export function openBrowse(browseId: string, titulo?: string) {
  navegar({ view: "browse", browseId, titulo });
}

async function cargarBrowse(id: string) {
  setBrowseId(id);
  setBrowseLoading(true);
  // Se limpia antes de pedir: si no, se ve la página anterior con el título
  // nuevo mientras carga, que parece un fallo.
  setBrowsePage(null);
  try {
    setBrowsePage(await api.browse(id));
  } catch (e) {
    console.error("no se pudo abrir la pagina", e);
    setBrowsePage(null);
  } finally {
    setBrowseLoading(false);
  }
}

/**
 * Copia la página abierta a una playlist local.
 *
 * Copia, no enlaza: la lista queda tuya y en tu equipo, así que sobrevive a que
 * YouTube la borre o su autor la haga privada. El precio es que no se actualiza
 * si el original cambia.
 */
export async function saveBrowseAsPlaylist(nombre?: string) {
  const pagina = browsePage();
  if (!pagina || savingBrowse()) return;

  const pistas = pagina.shelves.flatMap((e) => e.items.filter((i) => i.kind === "track"));
  if (!pistas.length) return;

  setSavingBrowse(true);
  try {
    const id = await api.createPlaylist(nombre ?? pagina.title ?? "Playlist guardada");
    // En serie y no en paralelo: `position` sale de un MAX sobre la tabla, y
    // en paralelo varias inserciones leerían el mismo máximo y el orden se
    // barajaría.
    for (const t of pistas) {
      await api.addToPlaylist(id, {
        videoId: t.id,
        title: t.title,
        author: t.subtitle,
        thumbnail: t.thumbnail,
      });
    }
    await refreshPlaylists();
    const lista = playlists().find((l) => l.id === id);
    if (lista) showPlaylist(lista);
  } catch (e) {
    console.error("no se pudo guardar la playlist", e);
  } finally {
    setSavingBrowse(false);
  }
}
export const [isFavorite, setIsFavorite] = createSignal(false);
export const [resultsLabel, setResultsLabel] = createSignal<string | null>(null);

export const [lyrics, setLyrics] = createSignal<Lyrics | null>(null);
export const [lyricsLoading, setLyricsLoading] = createSignal(false);
export const [fullLyricsOpen, setFullLyricsOpen] = createSignal(false);
export const [playerViewOpen, setPlayerViewOpen] = createSignal(true);

/**
 * Barra lateral abierta o encogida a raíl de iconos.
 *
 * Lo decide el usuario con el botón de menú, no la vista. En la referencia el
 * estado se conserva al navegar y mientras suena una canción; atarlo a la vista
 * hacía que la barra se encogiera sola y dejara de servir para navegar.
 */
const SIDEBAR_KEY = "posible.sidebar-open";

function readSidebarPref(): boolean {
  // En un WebView con el almacenamiento capado esto lanza; abierta es el
  // estado que espera alguien que abre la app por primera vez.
  try {
    return localStorage.getItem(SIDEBAR_KEY) !== "0";
  } catch {
    return true;
  }
}

export const [sidebarOpen, setSidebarOpen] = createSignal(readSidebarPref());

export function toggleSidebar() {
  const next = !sidebarOpen();
  setSidebarOpen(next);
  try {
    localStorage.setItem(SIDEBAR_KEY, next ? "1" : "0");
  } catch {
    // Sin persistencia se pierde entre sesiones, pero la sesión actual funciona.
  }
}

/**
 * Posicion local interpolada.
 *
 * El backend publica cada 100 ms, que basta para no desincronizarse, pero una
 * barra que solo se mueve 10 veces por segundo se ve a saltos. Interpolamos
 * localmente entre publicaciones y resincronizamos con cada una.
 */
const [localPos, setLocalPos] = createSignal(0);
export const position = localPos;

let lastSync = { at: 0, ms: 0 };

/**
 * El backend manda la cola solo cuando su revision cambia (con una playlist
 * grande seria carisimo reenviarla en cada tick); si la revision se repite, se
 * conserva la cola que ya tenemos.
 */
let lastQueueRev = -1;
function adoptQueue(s: PlaybackState): PlaybackState {
  if (s.queueRev === lastQueueRev) return { ...s, queue: playback.queue };
  lastQueueRev = s.queueRev;
  return s;
}

export function initStore() {
  refreshPlaylists();
  api.getState().then((s) => setPlayback(reconcile(adoptQueue(s))));

  api.onPlayback((s) => {
    setPlayback(reconcile(adoptQueue(s)));
    lastSync = { at: performance.now(), ms: s.positionMs };
    setLocalPos(s.positionMs);
  });

  let lastTick = 0;
  const tick = (now: number) => {
    if (playback.playing && now - lastTick >= 100) {
      lastTick = now;
      setLocalPos(lastSync.ms + (now - lastSync.at));
    }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);

  // El estado de favorito se consulta al cambiar de pista, no en cada tick.
  createEffect(
    on(
      () => playback.track?.videoId,
      (id) => {
        if (!id) return setIsFavorite(false);
        api.isFavorite(id).then(setIsFavorite).catch(() => setIsFavorite(false));
      },
    ),
  );

  // La paleta se recalcula solo cuando cambia la portada, no en cada estado.
  createEffect(
    on(
      () => playback.track?.thumbnail,
      (thumb) => {
        if (!thumb) {
          setPalette(DEFAULT_PALETTE);
          return;
        }
        // Limpia y grande, no la que venga. `hqdefault.jpg` es 4:3 con las
        // barras negras incrustadas: medido sobre una portada real, ese negro
        // era el 23% de los pixeles y se llevaba el foco principal de la malla.
        api
          .getPalette(thumbUrl(thumb, 640, 360) ?? thumb)
          .then(setPalette)
          .catch(() => setPalette(DEFAULT_PALETTE));
      },
    ),
  );

  // La letra se pide una sola vez por pista y se almacena en el store.
  let lastLyricsVideoId = "";
  createEffect(
    on(
      () => playback.track?.videoId,
      async (id) => {
        if (!id || id === lastLyricsVideoId) return;
        lastLyricsVideoId = id;
        const t = playback.track;
        if (!t) {
          setLyrics(null);
          return;
        }
        setLyricsLoading(true);
        try {
          const res = await api.getLyrics(t.title, t.author, playback.durationMs);
          if (lastLyricsVideoId === id) {
            setLyrics(res);
          }
        } catch {
          if (lastLyricsVideoId === id) {
            setLyrics(null);
          }
        } finally {
          if (lastLyricsVideoId === id) {
            setLyricsLoading(false);
          }
        }
      },
    ),
  );
}

export async function toggleFavorite() {
  try {
    setIsFavorite(await api.toggleFavorite());
  } catch (e) {
    console.error("no se pudo marcar como favorito", e);
  }
}

/** Reproduce una lista guardada (favoritos o historial) desde una posicion. */
export function playSaved(list: { videoId: string; title: string; author: string; thumbnail: string | null }[], index: number) {
  api.playQueue(list, index);
}

/**
 * Clasifica lo que hay en el cuadro: enlace de playlist, enlace de video o
 * texto de busqueda. Espejo de `ytm_source::parse_input` (con tests alli).
 */
function classify(raw: string): { kind: "playlist" | "video" | "query"; value: string } {
  const s = raw.trim();
  const isUrl = /youtube\.com\/|youtu\.be\//.test(s);
  if (isUrl) {
    const list = /[?&]list=([\w-]+)/.exec(s)?.[1];
    // WL y LL exigen sesion; se cae al video o a la busqueda.
    if (list && list !== "WL" && list !== "LL") return { kind: "playlist", value: list };
    const v = /[?&]v=([\w-]{11})/.exec(s)?.[1] ?? /youtu\.be\/([\w-]{11})/.exec(s)?.[1];
    if (v) return { kind: "video", value: v };
  }
  if (/^(PL|OLAK5uy_|RDCLAK)[\w-]{10,}$/.test(s)) return { kind: "playlist", value: s };
  return { kind: "query", value: s };
}

export async function runSearch(q: string) {
  if (!q.trim()) return;
  const c = classify(q);

  // Un enlace de cancion suelto se reproduce directamente, y detras va su
  // radio igual que si se hubiera elegido en la busqueda.
  if (c.kind === "video") {
    api.playNow(c.value);
    setView("home");
    setPlayerViewOpen(true);
    api
      .radio(c.value)
      .then((r) => {
        setRelatedArtistId(r.artistBrowseId);
        const resto = r.tracks.filter((t) => t.videoId !== c.value);
        if (resto.length) api.setUpNext(resto.map(toTrack));
      })
      .catch((e) => console.error("no se pudo cargar la radio", e));
    return;
  }

  setSearching(true);
  // Deja huella en el historial: "atrás" desde una página de artista tiene que
  // devolver a la búsqueda, no al inicio.
  if (view() !== "search") navegar({ view: "search" });
  setResultsLabel(null);
  setSearchFilter(null);
  setSearchMoreToken(null);
  try {
    if (c.kind === "playlist") {
      // A su página, no a una lista pelada: `browse` trae portada, autor,
      // número de pistas y duración, que es lo que la hace reconocible.
      // El prefijo `VL` es el que YouTube usa para el `browseId` de una lista.
      setSearching(false);
      openBrowse(`VL${c.value}`);
      return;
    } else {
      // Sin filtrar por "solo canciones": ese filtro mira unicamente la
      // pestania Songs del catalogo y deja fuera videos, subidas de usuario,
      // directos y remixes, que es justo lo que no se encontraba.
      const page = await api.search(c.value);
      setResults(page.items);
      // Los filtros los define YouTube en la respuesta, no nosotros.
      setSearchChips(page.chips);
      setSearchMoreToken(page.continuation);
      setSearchOverflow(
        page.continuation ? null : (chipDeCanciones(page.chips)?.params ?? null),
      );
    }
  } catch (e) {
    console.error("busqueda fallida", e);
    setResults([]);
    setSearchChips([]);
  } finally {
    setSearching(false);
  }
}

/** Convierte un resultado de búsqueda o de radio en una pista de la cola. */
function toTrack(r: SearchResult) {
  return {
    videoId: r.videoId,
    title: r.title,
    author: r.subtitle,
    thumbnail: r.thumbnail,
    durationMs: parseDuration(r.duration),
  };
}

/**
 * Reproduce una pista y llena la cola con su radio.
 *
 * Es lo que hace YouTube Music: eliges una canción y detrás vienen ~50
 * recomendadas, no el resto de la búsqueda.
 *
 * La radio se pide DESPUÉS de arrancar el audio, no antes: la respuesta pesa
 * más de un mega y esperarla dejaría un silencio de medio segundo cada vez que
 * se pulsa una canción. Y se aplica con `setUpNext`, que sustituye lo que viene
 * detrás sin tocar la pista actual — con `playQueue` la canción se reiniciaría
 * justo cuando llegan las recomendaciones.
 */
export async function playWithRadio(track: SearchResult) {
  api.playQueue([toTrack(track)], 0);
  setRelatedArtistId(null);
  try {
    const r = await api.radio(track.videoId);
    setRelatedArtistId(r.artistBrowseId);
    // La semilla suele venir la primera en su propia radio.
    const resto = r.tracks.filter((t) => t.videoId !== track.videoId);
    if (resto.length) api.setUpNext(resto.map(toTrack));
  } catch (e) {
    // Sin radio se queda la pista suelta, que es lo que había antes.
    console.error("no se pudo cargar la radio", e);
  }
}

/** Reproduce un resultado y encola su radio. */
export function playFromResults(index: number) {
  const elegido = results()[index];
  if (!elegido || elegido.kind !== "track") return;
  playWithRadio({
    videoId: elegido.id,
    title: elegido.title,
    subtitle: elegido.subtitle,
    duration: elegido.duration,
    thumbnail: elegido.thumbnail,
  });
}

/** Aplica uno de los filtros que ofrece YouTube, o los quita todos. */
export async function applySearchFilter(params: string | null) {
  const q = query().trim();
  if (!q) return;
  setSearchFilter(params);
  setSearching(true);
  setSearchMoreToken(null);
  try {
    const page = await api.search(q, params ?? undefined);
    setResults(page.items);
    // Al filtrar, YouTube devuelve los chips otra vez; si vinieran vacíos se
    // conservan los que había, para no dejar la fila en blanco.
    if (page.chips.length) setSearchChips(page.chips);
    setSearchMoreToken(page.continuation);
    // Solo "Todo" necesita el relevo; un filtro concreto ya pagina solo.
    setSearchOverflow(
      params === null && !page.continuation
        ? (chipDeCanciones(page.chips.length ? page.chips : searchChips())?.params ?? null)
        : null,
    );
  } catch (e) {
    console.error("no se pudo filtrar", e);
  } finally {
    setSearching(false);
  }
}

/** Siguiente página de resultados, si la hay. */
export async function loadMoreResults() {
  if (loadingMore()) return;
  const token = searchMoreToken();
  const relevo = searchOverflow();
  if (!token && !relevo) return;

  setLoadingMore(true);
  try {
    let page;
    if (token) {
      page = await api.searchMore(token);
    } else {
      // "Todo" se acabó: se sigue por el filtro de canciones. Se consume una
      // sola vez; a partir de aquí manda su propia continuación.
      page = await api.search(query().trim(), relevo!);
      setSearchOverflow(null);
    }
    // Se anexa, no se sustituye: es paginación, no una búsqueda nueva. Y se
    // deduplica porque el relevo repite lo que "Todo" ya había mostrado.
    setResults(sinRepetir([...results(), ...page.items]));
    setSearchMoreToken(page.continuation);
  } catch (e) {
    console.error("no se pudieron cargar mas resultados", e);
    // Sin token no se reintenta en bucle contra un servidor que ya dijo que no.
    setSearchMoreToken(null);
    setSearchOverflow(null);
  } finally {
    setLoadingMore(false);
  }
}

/**
 * El filtro de canciones, buscado por sus `params` y no por su etiqueta.
 *
 * La etiqueta viene traducida al idioma de la petición; los `params` no. El
 * prefijo `EgWKAQII` es el que YouTube usa para "solo canciones".
 */
function chipDeCanciones(chips: SearchChip[]): SearchChip | undefined {
  return chips.find((c) => c.params.startsWith("EgWKAQII"));
}

/** Quita repetidos conservando el orden de aparición. */
function sinRepetir(items: ShelfItem[]): ShelfItem[] {
  const vistos = new Set<string>();
  return items.filter((i) => {
    const clave = `${i.kind}:${i.id}`;
    return vistos.has(clave) ? false : vistos.add(clave);
  });
}


/** Portada grande de la pista actual. */
export function coverUrl(width = 1280): string | null {
  // Sin forzar proporcion: la portada se pinta con la suya, cuadrada si es de
  // album y 16:9 si es de video. Forzar una relacion aqui obliga al servidor a
  // rellenar con barras negras, y esas barras van dentro del JPEG.
  return thumbAt(playback.track?.thumbnail, width, Math.round((width * 9) / 16));
}

/** Respaldo de la portada cuando el servidor no tiene la variante grande. */
export function coverFallbackUrl(): string | null {
  const raw = playback.track?.thumbnail;
  if (!raw) return null;
  const alt = thumbFallback(raw);
  return alt ? thumbAt(alt, 320, 180) : null;
}
