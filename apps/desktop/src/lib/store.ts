import { createSignal, createEffect, on } from "solid-js";
import { cambiarConPortada } from "./transicion";
import { avisar, avisarError, motivo, vigilarConexion } from "./toast";
import { instalarAtajos } from "./atajos";
import { createStore, reconcile } from "solid-js/store";
import {
  EXPLORE,
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

/**
 * Paleta de la casa, la misma que `Palette::default()` en Rust.
 *
 * Está repetida aquí a propósito: es lo que se pinta durante el primer
 * fotograma, y esperar a que el backend la mande haría que el fondo arrancara
 * apagado y se encendiera a la vista. Si se cambia una, hay que cambiar la otra.
 */
const DEFAULT_PALETTE: Palette = {
  stops: [
    { color: "#9470cd", weight: 0.5 },
    { color: "#6e64c8", weight: 0.3 },
    { color: "#c585bf", weight: 0.2 },
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
/**
 * Nombre con el que se llegó a la página.
 *
 * Explorar y las categorías de género no traen cabecera propia, así que sin
 * esto se abrían con un "Sin título" enorme arriba. El de la respuesta manda
 * cuando existe; este es el respaldo.
 */
export const [browseTitulo, setBrowseTitulo] = createSignal<string | null>(null);
export const [savingBrowse, setSavingBrowse] = createSignal(false);
/** Sigue habiendo tandas en camino de la lista abierta. */
export const [browseCompleting, setBrowseCompleting] = createSignal(false);

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
    avisarError(motivo(e, "No se pudo crear la playlist."));
    return null;
  }
}

export async function deletePlaylist(id: number) {
  const nombre = playlists().find((l) => l.id === id)?.name;
  try {
    await api.deletePlaylist(id);
    avisar(nombre ? `«${nombre}» eliminada` : "Playlist eliminada");
    // Si estaba abierta, se cierra: dejarla en pantalla mostraría una lista
    // que ya no existe.
    if (openPlaylist()?.lista.id === id) setOpenPlaylist(null);
    await refreshPlaylists();
  } catch (e) {
    console.error("no se pudo borrar la playlist", e);
    avisarError(motivo(e, "No se pudo borrar la playlist."));
  }
}

export async function renamePlaylist(id: number, name: string) {
  const limpio = name.trim();
  if (!limpio) return;
  try {
    await api.renamePlaylist(id, limpio);
    await refreshPlaylists();
    // La abierta lleva su propia copia del nombre: sin esto, la cabecera
    // seguiría mostrando el viejo hasta salir y volver a entrar.
    const abierta = openPlaylist();
    if (abierta?.lista.id === id) {
      setOpenPlaylist({ ...abierta, lista: { ...abierta.lista, name: limpio } });
    }
  } catch (e) {
    console.error("no se pudo renombrar la playlist", e);
    avisarError(motivo(e, "No se pudo cambiar el nombre."));
  }
}

export async function showPlaylist(lista: Playlist) {
  navegar({ view: "playlist" });
  try {
    setOpenPlaylist({ lista, tracks: await api.playlistTracks(lista.id) });
  } catch (e) {
    console.error("no se pudo abrir la playlist", e);
    avisarError(motivo(e, "No se pudo abrir la playlist."));
    setOpenPlaylist({ lista, tracks: [] });
  }
}

export async function addTrackToPlaylist(id: number, track: Partial<Track>) {
  try {
    await api.addToPlaylist(id, track);
    await refreshPlaylists();
    // Es la única acción de la aplicación que no deja ni rastro en pantalla:
    // el diálogo se cierra y la canción sigue donde estaba.
    const lista = playlists().find((l) => l.id === id);
    avisar(`Añadida a «${lista?.name ?? "la playlist"}»`);
    // Si es la que está abierta, se refresca para que la pista aparezca ya.
    const abierta = openPlaylist();
    if (abierta?.lista.id === id) showPlaylist(abierta.lista);
  } catch (e) {
    console.error("no se pudo anadir a la playlist", e);
    avisarError(motivo(e, "No se pudo añadir a la playlist."));
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
    avisarError(motivo(e, "No se pudo quitar de la playlist."));
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
type Destino = { view: View; browseId?: string; params?: string; titulo?: string };

const [historial, setHistorial] = createSignal<Destino[]>([{ view: "home" }]);
const [posicion, setPosicion] = createSignal(0);

export const puedeAtras = () => posicion() > 0;
export const puedeAdelante = () => posicion() < historial().length - 1;

/**
 * Los botones laterales del ratón, atrás y adelante.
 *
 * Van en `mouseup` y no en `mousedown`: en `mousedown` el navegador todavía
 * puede convertirlos en su propia navegación. Los botones 3 y 4 son los
 * laterales; el 2 es la rueda y no se toca.
 */
function instalarBotonesDelRaton() {
  window.addEventListener("mouseup", (e) => {
    if (e.button === 3) {
      e.preventDefault();
      atras();
    } else if (e.button === 4) {
      e.preventDefault();
      adelante();
    }
  });
  // Sin esto, Windows deja además el menú contextual de retroceso del WebView.
  window.addEventListener("mousedown", (e) => {
    if (e.button === 3 || e.button === 4) e.preventDefault();
  });
}

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
  togglePlayerView(false);
  setView(d.view);
  if (d.view === "browse" && d.browseId) {
    setBrowseTitulo(d.titulo ?? null);
    cargarBrowse(d.browseId, d.params);
  }
}

/** Abre una página de artista, álbum o playlist. */
export function openBrowse(browseId: string, titulo?: string, params?: string) {
  navegar({ view: "browse", browseId, params, titulo });
}

/** Explorar: novedades, rankings y las categorías de género. */
export function openExplore() {
  openBrowse(EXPLORE, "Explorar");
}

/**
 * Tope de tandas encadenadas.
 *
 * YouTube manda las listas de 100 en 100, así que son 5.000 pistas. Una lista
 * de 551 son seis peticiones; el tope está para que una lista absurda no deje
 * la aplicación pidiendo páginas para siempre.
 */
const MAX_TANDAS = 50;

/**
 * Generación de la carga en curso.
 *
 * Encadenar tandas tarda varios segundos, y en ese rato el usuario puede
 * haberse ido a otra página. Sin esto, las pistas de la lista anterior
 * aparecerían dentro de la nueva.
 */
let generacionBrowse = 0;

/** Encadenado en curso, para que "Guardar" no copie media lista. */
let tandasEnCurso: Promise<void> | null = null;

async function cargarBrowse(id: string, params?: string) {
  const gen = ++generacionBrowse;
  setBrowseId(id);
  setBrowseLoading(true);
  // Se limpia antes de pedir: si no, se ve la página anterior con el título
  // nuevo mientras carga, que parece un fallo.
  setBrowsePage(null);
  try {
    const primera = await api.browse(id, params);
    if (gen !== generacionBrowse) return;
    setBrowsePage(primera);
    setBrowseLoading(false);
    tandasEnCurso = completarBrowse(gen, primera.continuation);
    await tandasEnCurso;
  } catch (e) {
    console.error("no se pudo abrir la pagina", e);
    avisarError(motivo(e, "No se pudo abrir esta página."));
    if (gen === generacionBrowse) setBrowsePage(null);
  } finally {
    if (gen === generacionBrowse) setBrowseLoading(false);
  }
}

/**
 * Pide el resto de tandas y las va pegando a la lista.
 *
 * Se pintan según llegan en vez de esperar a tenerlas todas: en una lista de
 * 551 pistas son seis peticiones, y ver las 100 primeras al instante es mejor
 * que mirar un hueco durante varios segundos.
 */
async function completarBrowse(gen: number, primerToken: string | null) {
  let token = primerToken;
  if (!token) return;

  setBrowseCompleting(true);
  try {
    for (let tanda = 0; token && tanda < MAX_TANDAS; tanda++) {
      let siguiente: BrowsePage;
      try {
        siguiente = await api.browseMore(token);
      } catch (e) {
        // Media lista es mejor que ninguna: se deja lo que haya llegado.
        console.error("se cortó la lista al pedir más pistas", e);
        avisarError(motivo(e, "La lista se ha quedado a medias."));
        return;
      }
      if (gen !== generacionBrowse) return;

      const nuevas = siguiente.shelves.flatMap((e) => e.items);
      if (!nuevas.length) return;
      setBrowsePage((actual) => (actual ? pegarPistas(actual, nuevas) : actual));
      token = siguiente.continuation;
    }
  } finally {
    if (gen === generacionBrowse) setBrowseCompleting(false);
  }
}

/**
 * Añade pistas al final de la última estantería que ya tenía pistas.
 *
 * Objeto nuevo y no mutación: las señales de Solid comparan por referencia, y
 * mutando el que ya está dentro no se repintaría nada.
 */
function pegarPistas(pagina: BrowsePage, nuevas: BrowsePage["shelves"][number]["items"]) {
  let destino = -1;
  pagina.shelves.forEach((e, i) => {
    if (e.items.some((t) => t.kind === "track")) destino = i;
  });

  const vistos = new Set(
    pagina.shelves.flatMap((e) => e.items.map((i) => `${i.kind}:${i.id}`)),
  );
  const sinRepetir = nuevas.filter((i) => vistos.has(`${i.kind}:${i.id}`) === false);
  if (!sinRepetir.length) return pagina;

  const shelves =
    destino === -1
      ? [...pagina.shelves, { title: "", items: sinRepetir }]
      : pagina.shelves.map((e, i) =>
          i === destino ? { ...e, items: [...e.items, ...sinRepetir] } : e,
        );

  return { ...pagina, shelves };
}

/**
 * Copia la página abierta a una playlist local.
 *
 * Copia, no enlaza: la lista queda tuya y en tu equipo, así que sobrevive a que
 * YouTube la borre o su autor la haga privada. El precio es que no se actualiza
 * si el original cambia.
 */
export async function saveBrowseAsPlaylist(nombre?: string) {
  if (!browsePage() || savingBrowse()) return;

  setSavingBrowse(true);
  // Antes de copiar nada hay que tener la lista entera: si el usuario pulsa
  // Guardar a los dos segundos de abrir una lista de 551 pistas, sin esto se
  // llevaría las 100 primeras y creería que están todas.
  await tandasEnCurso?.catch(() => {});

  const pagina = browsePage();
  if (!pagina) {
    setSavingBrowse(false);
    return;
  }

  const pistas = pagina.shelves.flatMap((e) => e.items.filter((i) => i.kind === "track"));
  if (!pistas.length) {
    setSavingBrowse(false);
    return;
  }

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
    avisar(`Guardadas ${pistas.length} canciones`);
    const lista = playlists().find((l) => l.id === id);
    if (lista) showPlaylist(lista);
  } catch (e) {
    console.error("no se pudo guardar la playlist", e);
    avisarError(motivo(e, "No se pudo guardar la playlist."));
  } finally {
    setSavingBrowse(false);
  }
}
/**
 * Ultima pista escuchada, recuperada del historial al arrancar.
 *
 * # Por que no se reanuda sola
 *
 * Al abrir la aplicacion NO suena nada. Un programa que empieza a hacer ruido
 * en cuanto se abre es de las cosas mas molestas que puede hacer un
 * reproductor, y basta con que lo abras una vez en una reunion para no
 * perdonarlo.
 *
 * Pero dejar la ventana en negro tampoco vale: la portada, sus colores y el
 * titulo se restauran, y el boton de reproducir arranca justo esa cancion. Se
 * ve como la dejaste, sin sonar.
 *
 * Desde el principio, no por donde ibas: guardar la posicion exacta exigiria
 * escribirla en disco constantemente, y volver a una cancion por la mitad rara
 * vez es lo que uno quiere al dia siguiente.
 */
export const [pendiente, setPendiente] = createSignal<Track | null>(null);

/**
 * La pista que la interfaz debe MOSTRAR.
 *
 * Lo que suena si hay algo sonando; si no, lo ultimo que sono. Todo lo visual
 * —portada, paleta, fondo, titulo— tira de aqui; los controles siguen mirando
 * `playback`, porque una pista recuperada no esta cargada en el motor.
 */
export const trackVisible = (): Track | null => playback.track ?? pendiente();

/** Arranca la pista recuperada del historial. */
export function reanudarPendiente() {
  const t = pendiente();
  if (t) api.playQueue([t], 0);
}

/**
 * Abre o cierra la vista de reproducción.
 *
 * Pasa por aquí todo el mundo, incluida la navegación: cambiar a Inicio con una
 * canción sonando también minimiza, y esa era justo la transición que se veía
 * de golpe.
 *
 * `animar` en falso para los sitios que ADEMÁS empiezan a reproducir otra cosa:
 * ahí la portada de origen es la de la canción anterior, y verla volar para
 * cambiar a mitad de vuelo es peor que no animar nada.
 */
export function togglePlayerView(abrir: boolean, animar = true) {
  if (abrir === playerViewOpen()) return;
  const cambiar = () => setPlayerViewOpen(abrir);
  if (animar) cambiarConPortada(abrir, cambiar);
  else cambiar();
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
  vigilarConexion();
  instalarAtajos();
  instalarBotonesDelRaton();
  refreshPlaylists();
  api.getState().then((s) => setPlayback(reconcile(adoptQueue(s))));

  // La ultima escuchada, para que la ventana no se abra en negro. No se
  // comprueba si el motor tiene algo: `trackVisible` ya da preferencia a lo que
  // suene de verdad.
  api
    .history()
    .then(([ultima]) => {
      if (ultima) {
        setPendiente({
          videoId: ultima.videoId,
          title: ultima.title,
          author: ultima.author,
          thumbnail: ultima.thumbnail,
          durationMs: null,
        });
      }
    })
    .catch(() => {});

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

  // Un fallo del motor no se veía por ningún sitio: la canción simplemente no
  // sonaba. `on` con el mensaje como fuente para que el mismo error repetido en
  // varios estados seguidos no saque un aviso por cada uno.
  createEffect(
    on(
      () => playback.error,
      (e) => {
        if (e) avisarError(motivo(e, "No se pudo reproducir esta canción."));
      },
    ),
  );

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
      () => trackVisible()?.thumbnail,
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
    avisarError(motivo(e, "No se pudo marcar como favorito."));
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
    togglePlayerView(true, false);
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
    avisarError(motivo(e, "La búsqueda ha fallado."));
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
    avisarError(motivo(e, "No se pudo aplicar el filtro."));
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
    avisarError(motivo(e, "No se pudieron cargar más resultados."));
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
  return thumbAt(trackVisible()?.thumbnail, width, Math.round((width * 9) / 16));
}

/** Respaldo de la portada cuando el servidor no tiene la variante grande. */
export function coverFallbackUrl(): string | null {
  const raw = trackVisible()?.thumbnail;
  if (!raw) return null;
  const alt = thumbFallback(raw);
  return alt ? thumbAt(alt, 320, 180) : null;
}
