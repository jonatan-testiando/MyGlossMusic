import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { mockApi } from "./mock";

export interface Track {
  videoId: string;
  title: string;
  author: string;
  thumbnail: string | null;
  /** `null` mientras la pista no se ha resuelto. La interfaz deja el hueco. */
  durationMs: number | null;
}

export interface SearchResult {
  videoId: string;
  title: string;
  subtitle: string;
  duration: string | null;
  thumbnail: string | null;
}

export type Repeat = "off" | "all" | "one";

export interface PlaybackState {
  track: Track | null;
  playing: boolean;
  loading: boolean;
  positionMs: number;
  durationMs: number;
  volume: number;
  buffered: number;
  queue: Track[];
  /** Sube cuando el contenido de la cola cambia; si no sube, `queue` llega vacia y se conserva la anterior. */
  queueRev: number;
  queueIndex: number;
  repeat: Repeat;
  shuffle: boolean;
  error: string | null;
}

export interface LyricLine {
  startMs: number;
  endMs: number;
  text: string;
}

export interface Lyrics {
  lines: LyricLine[];
  plain: string | null;
  synced: boolean;
  source: string;
}

export interface SavedTrack {
  videoId: string;
  title: string;
  author: string;
  thumbnail: string | null;
  at: number;
}

/** Un color de la portada, con la superficie que ocupa (0..1). */
export interface Stop {
  color: string;
  weight: number;
}

/** Una playlist local del usuario. */
export interface Playlist {
  id: number;
  name: string;
  count: number;
  thumbnail: string | null;
}

export interface Palette {
  /** Colores de la malla ambiental, del que mas ocupa al que menos. */
  stops: Stop[];
  background: string;
  backgroundAlt: string;
  accent: string;
  foreground: string;
  isLight: boolean;
}

/** Qué es un elemento de una estantería: decide adónde lleva al pulsarlo. */
export type ItemKind = "track" | "album" | "artist" | "playlist";

export interface ShelfItem {
  kind: ItemKind;
  /** `videoId` si es una pista, `browseId` o `playlistId` en los demás casos. */
  id: string;
  title: string;
  subtitle: string;
  thumbnail: string | null;
  duration: string | null;
}

export interface Shelf {
  title: string;
  items: ShelfItem[];
}

export interface BrowsePage {
  title: string | null;
  subtitle: string | null;
  /** "20 M de visualizaciones • 88 pistas • 5 horas y 59 minutos". */
  secondSubtitle: string | null;
  description: string | null;
  thumbnail: string | null;
  shelves: Shelf[];
  /** Token de la siguiente tanda. YouTube corta las listas de 100 en 100. */
  continuation: string | null;
}

/** Un filtro del buscador, tal y como lo ofrece YouTube. */
export interface SearchChip {
  label: string;
  params: string;
}

export interface SearchPage {
  items: ShelfItem[];
  chips: SearchChip[];
  continuation: string | null;
}

export interface Radio {
  playlistId: string | null;
  tracks: SearchResult[];
  /** Canal del artista de la pista semilla, si el byline lo trae. */
  artistBrowseId: string | null;
}

export interface PlaylistResult {
  title: string | null;
  tracks: SearchResult[];
}

/** Lo que ocupa la aplicación en disco. */
export interface Storage {
  cacheBytes: number;
  cacheFiles: number;
  /** Tope de la caché en bytes. 0 es sin límite. */
  cacheLimit: number;
  dbBytes: number;
  cacheDir: string;
  dataDir: string;
}

export interface ExtractorStatus {
  available: boolean;
  version: string | null;
  program: string | null;
}

export interface ClientHealth {
  id: string;
  status: string;
  directAudio: number;
  best: string | null;
}

const realApi = {
  search: (query: string, params?: string) =>
    invoke<SearchPage>("search", { query, params }),
  searchMore: (continuation: string) =>
    invoke<SearchPage>("search_more", { continuation }),

  searchSuggestions: (query: string) =>
    invoke<string[]>("search_suggestions", { query }),

  radio: (videoId: string) => invoke<Radio>("radio", { videoId }),
  home: () => invoke<BrowsePage>("home"),
  browse: (browseId: string, params?: string) =>
    invoke<BrowsePage>("browse", { browseId, params }),
  browseMore: (continuation: string) =>
    invoke<BrowsePage>("browse_more", { continuation }),
  setUpNext: (tracks: Partial<Track>[]) => invoke<void>("set_up_next", { tracks }),

  playlist: (id: string) => invoke<PlaylistResult>("playlist", { id }),

  playQueue: (tracks: Partial<Track>[], start: number) =>
    invoke<void>("play_queue", { tracks, start }),

  playNow: (videoId: string) => invoke<void>("play_now", { videoId }),
  togglePlay: () => invoke<void>("toggle_play"),
  next: () => invoke<void>("next_track"),
  prev: () => invoke<void>("prev_track"),
  jumpTo: (index: number) => invoke<void>("jump_to", { index }),
  seek: (positionMs: number) => invoke<void>("seek", { positionMs }),
  setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
  setRepeat: (mode: Repeat) => invoke<void>("set_repeat", { mode }),
  setShuffle: (on: boolean) => invoke<void>("set_shuffle", { on }),
  getState: () => invoke<PlaybackState>("get_state"),
  getPalette: (url: string) => invoke<Palette>("get_palette", { url }),
  getLyrics: (title: string, artist: string, durationMs: number) =>
    invoke<Lyrics | null>("get_lyrics", { title, artist, durationMs }),
  toggleFavorite: () => invoke<boolean>("toggle_favorite"),
  isFavorite: (videoId: string) => invoke<boolean>("is_favorite", { videoId }),
  favorites: () => invoke<SavedTrack[]>("favorites"),

  createPlaylist: (name: string) => invoke<number>("create_playlist", { name }),
  renamePlaylist: (id: number, name: string) =>
    invoke<void>("rename_playlist", { id, name }),
  deletePlaylist: (id: number) => invoke<void>("delete_playlist", { id }),
  playlists: () => invoke<Playlist[]>("playlists"),
  playlistTracks: (id: number) => invoke<SavedTrack[]>("playlist_tracks", { id }),
  addToPlaylist: (id: number, track: Partial<Track>) =>
    invoke<void>("add_to_playlist", { id, track }),
  removeFromPlaylist: (id: number, videoId: string) =>
    invoke<void>("remove_from_playlist", { id, videoId }),
  history: () => invoke<SavedTrack[]>("history"),
  diagnose: () => invoke<ClientHealth[]>("diagnose"),
  extractorStatus: () => invoke<ExtractorStatus>("extractor_status"),
  storageInfo: () => invoke<Storage>("storage_info"),
  clearCache: () => invoke<number>("clear_cache"),
  setCacheLimit: (bytes: number) => invoke<void>("set_cache_limit", { bytes }),
  appVersion: () => invoke<string>("app_version"),

  minimize: () => (inTauri ? getCurrentWindow().minimize() : Promise.resolve()),
  toggleMaximize: () => (inTauri ? getCurrentWindow().toggleMaximize() : Promise.resolve()),
  close: () => (inTauri ? getCurrentWindow().close() : Promise.resolve()),

  onPlayback: (cb: (s: PlaybackState) => void) =>
    listen<PlaybackState>("playback", (e) => cb(e.payload)),
};

/**
 * Fuera de Tauri (navegador, `npm run dev`) se usan datos de ejemplo para poder
 * iterar sobre el diseno sin recompilar Rust. En la aplicacion real siempre
 * existe `__TAURI_INTERNALS__`, asi que esto nunca se activa.
 */
const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api: typeof realApi = inTauri
  ? realApi
  : (mockApi as unknown as typeof realApi);

/**
 * "3:28" o "1:02:33" a milisegundos.
 *
 * La busqueda ya trae la duracion como texto; convertirla evita que la cola se
 * pinte sin duraciones hasta que cada pista se resuelva una por una.
 */
export function parseDuration(text: string | null | undefined): number | null {
  if (!text) return null;
  const parts = text.split(":").map((p) => Number(p.trim()));
  if (parts.length < 2 || parts.length > 3 || parts.some((n) => !Number.isFinite(n))) return null;
  const seconds = parts.reduce((total, n) => total * 60 + n, 0);
  return seconds * 1000;
}

/** Milisegundos a "m:ss". */
export function fmtTime(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) ms = 0;
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

/**
 * Reduce una miniatura de YouTube a un tamano concreto.
 *
 * Se usa para el fondo: pedimos la portada a 48px y la estiramos a pantalla
 * completa. El reescalado del navegador ES el desenfoque, y cuesta cero, a
 * diferencia de `backdrop-filter: blur()`, que repinta cada fotograma.
 */
/**
 * Miniaturas de video: `https://i.ytimg.com/vi/<id>/<variante>.jpg`.
 *
 * Este servidor no admite ancho ni alto, solo un juego de variantes con nombre
 * fijo. Y la mayoria — `hqdefault`, `sddefault`, `default` — son 4:3 con las
 * barras negras DENTRO del JPEG, no un recorte que se pueda deshacer con CSS.
 * Las unicas limpias en 16:9 son `maxresdefault` (1280x720) y `mqdefault`
 * (320x180).
 */
const YTIMG_VARIANT = /^(https?:\/\/i\.ytimg\.com\/vi\/[\w-]{11}\/)[\w]+\.jpg/;

/** La variante limpia de respaldo, para cuando `maxresdefault` no existe. */
export function thumbFallback(url: string): string | null {
  const base = YTIMG_VARIANT.exec(url)?.[1];
  return base ? `${base}mqdefault.jpg` : null;
}

/**
 * Reescribe la URL de una miniatura al tamanio pedido, SIN pasarla por la
 * cache de Rust.
 *
 * Hace falta suelto porque `get_palette` descarga la imagen desde Rust: si le
 * llegara la URL del proxy, pediria `thumb://` a googleusercontent.
 */
export function thumbUrl(
  url: string | null | undefined,
  width: number,
  height = width,
): string | null {
  if (!url) return null;
  return url
    .replace(/=w\d+-h\d+/, `=w${width}-h${height}`)
    .replace(/\/w\d+-h\d+/, `/w${width}-h${height}`)
    // `maxresdefault` no existe para todo; el `onError` de la portada cae a
    // `mqdefault`, que si esta siempre.
    .replace(YTIMG_VARIANT, (_m, base) =>
      `${base}${width >= 480 ? "maxresdefault" : "mqdefault"}.jpg`,
    );
}

export function thumbAt(
  url: string | null | undefined,
  width: number,
  height = width,
): string | null {
  const resized = thumbUrl(url, width, height);
  if (!resized) return null;
  if (!inTauri) return resized;
  // Dentro de la app las imagenes las sirve Rust con cache en disco
  // (`thumbs.rs`): googleusercontent responde 429 a las rafagas de 20
  // miniaturas que dispara una busqueda. WebView2 expone los esquemas
  // propios como `http://<esquema>.localhost/`.
  const base = navigator.userAgent.includes("Windows")
    ? "http://thumb.localhost/"
    : "thumb://localhost/";
  return `${base}?u=${encodeURIComponent(resized)}`;
}
