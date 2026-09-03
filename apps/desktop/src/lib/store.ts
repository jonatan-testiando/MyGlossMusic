import { createSignal, createEffect, on } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { api, type PlaybackState, type Palette, type SearchResult, thumbAt } from "./api";

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
/** Cabecera de la vista de resultados cuando lo cargado es una playlist. */
export const [resultsLabel, setResultsLabel] = createSignal<string | null>(null);

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
        api
          .getPalette(thumb)
          .then(setPalette)
          .catch(() => setPalette(DEFAULT_PALETTE));
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

  // Un enlace de cancion suelto se reproduce directamente.
  if (c.kind === "video") {
    api.playNow(c.value);
    setView("home");
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
      setResults(await api.search(c.value));
    }
  } catch (e) {
    console.error("busqueda fallida", e);
    setResults([]);
  } finally {
    setSearching(false);
  }
}

/** Reproduce un resultado y encola el resto, que es lo que espera el usuario. */
export function playFromResults(index: number) {
  const list = results();
  if (!list.length) return;
  api.playQueue(
    list.map((r) => ({
      videoId: r.videoId,
      title: r.title,
      author: r.subtitle,
      thumbnail: r.thumbnail,
    })),
    index,
  );
}

/** Portada grande de la pista actual. */
export function coverUrl(size = 544): string | null {
  return thumbAt(playback.track?.thumbnail, size);
}
