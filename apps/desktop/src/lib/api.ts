import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { mockApi } from "./mock";

export interface Track {
  videoId: string;
  title: string;
  author: string;
  thumbnail: string | null;
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

export interface Palette {
  background: string;
  backgroundAlt: string;
  accent: string;
  foreground: string;
  isLight: boolean;
}

export interface PlaylistResult {
  title: string | null;
  tracks: SearchResult[];
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
  search: (query: string, onlySongs = true) =>
    invoke<SearchResult[]>("search", { query, onlySongs }),

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
  history: () => invoke<SavedTrack[]>("history"),
  diagnose: () => invoke<ClientHealth[]>("diagnose"),
  extractorStatus: () => invoke<ExtractorStatus>("extractor_status"),

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
export function thumbAt(url: string | null | undefined, size: number): string | null {
  if (!url) return null;
  const resized = url
    .replace(/=w\d+-h\d+/, `=w${size}-h${size}`)
    .replace(/\/w\d+-h\d+/, `/w${size}-h${size}`);
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
