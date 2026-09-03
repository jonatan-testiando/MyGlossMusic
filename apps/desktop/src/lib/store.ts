import { createSignal, createEffect, on } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { api, parseDuration, thumbAt, thumbUrl, thumbFallback, type PlaybackState, type Palette, type SearchResult, type Lyrics } from "./api";

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
export const [results, setResults] = createSignal<SearchResult[]>([]);
export const [searching, setSearching] = createSignal(false);
export const [query, setQuery] = createSignal("");
export const [view, setView] = createSignal<"home" | "search" | "library" | "diagnostics">("home");
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
        const resto = r.tracks.filter((t) => t.videoId !== c.value);
        if (resto.length) api.setUpNext(resto.map(toTrack));
      })
      .catch((e) => console.error("no se pudo cargar la radio", e));
    return;
  }

  setSearching(true);
  setView("search");
  setResultsLabel(null);
  try {
    if (c.kind === "playlist") {
      const p = await api.playlist(c.value);
      setResults(p.tracks);
      setResultsLabel(`${p.title ?? "Playlist"} \u2022 ${p.tracks.length} pistas`);
    } else {
      // Sin filtrar por "solo canciones": ese filtro mira unicamente la
      // pestania Songs del catalogo y deja fuera videos, subidas de usuario,
      // directos y remixes, que es justo lo que no se encontraba.
      setResults(await api.search(c.value, false));
    }
  } catch (e) {
    console.error("busqueda fallida", e);
    setResults([]);
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
  try {
    const r = await api.radio(track.videoId);
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
  const list = results();
  const elegido = list[index];
  if (!elegido) return;
  playWithRadio(elegido);
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
