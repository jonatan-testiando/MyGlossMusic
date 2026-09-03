import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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

export interface Palette {
  background: string;
  backgroundAlt: string;
  accent: string;
  foreground: string;
  isLight: boolean;
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
  diagnose: () => invoke<ClientHealth[]>("diagnose"),

  minimize: () => invoke<void>("window_minimize"),
  toggleMaximize: () => invoke<void>("window_toggle_maximize"),
  close: () => invoke<void>("window_close"),

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
  return url.replace(/=w\d+-h\d+/, `=w${size}-h${size}`).replace(/\/w\d+-h\d+/, `/w${size}-h${size}`);
}
