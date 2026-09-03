/**
 * Datos de ejemplo para desarrollar la interfaz en el navegador.
 *
 * Se activa solo en `npm run dev` cuando NO estamos dentro de Tauri. Permite
 * iterar sobre el diseno sin recompilar Rust ni depender de la red, y hace que
 * la interfaz sea inspeccionable con las herramientas del navegador.
 *
 * En la aplicacion real nunca se carga: dentro de Tauri existe
 * `window.__TAURI_INTERNALS__` y `api.ts` usa el backend de verdad.
 */
import type { ClientHealth, Lyrics, Palette, PlaybackState, SearchResult } from "./api";

const COVER = "https://i.ytimg.com/vi/jig2aRZbHm4/maxresdefault.jpg";

const TRACKS = [
  { videoId: "jig2aRZbHm4", title: "Fool For You", author: "Kastra • Fool For You", d: "3:28" },
  { videoId: "mCoNRv9KXAo", title: "Fool For You (VIP Remix)", author: "Kastra", d: "2:49" },
  { videoId: "wlaE1ztM9EE", title: "Circles", author: "Kastra y Alex Byrne", d: "3:28" },
  { videoId: "hZAC6YACKeo", title: "Supreme", author: "Kastra (Inquisitive remix)", d: "2:40" },
  { videoId: "lbzFtlZFjpk", title: "Without You (con Linney)", author: "Kastra", d: "3:10" },
  { videoId: "vXfB_IFcZVs", title: "Be There", author: "Kastra y Lost Boy", d: "3:20" },
];

const state: PlaybackState = {
  track: { videoId: TRACKS[0].videoId, title: TRACKS[0].title, author: TRACKS[0].author, thumbnail: COVER },
  playing: true,
  loading: false,
  positionMs: 23_000,
  durationMs: 208_000,
  volume: 0.8,
  buffered: 1,
  queue: TRACKS.map((t) => ({ videoId: t.videoId, title: t.title, author: t.author, thumbnail: COVER })),
  queueIndex: 0,
  repeat: "off",
  shuffle: false,
  error: null,
};

const listeners: ((s: PlaybackState) => void)[] = [];
const emit = () => listeners.forEach((l) => l({ ...state }));

setInterval(() => {
  if (state.playing) {
    state.positionMs = (state.positionMs + 100) % state.durationMs;
    emit();
  }
}, 100);

const results: SearchResult[] = TRACKS.map((t) => ({
  videoId: t.videoId,
  title: t.title,
  subtitle: t.author,
  duration: t.d,
  thumbnail: COVER,
}));

const health: ClientHealth[] = [
  { id: "ios", status: "OK", directAudio: 2, best: "itag 140 mp4a.40.2 130 kbps" },
  { id: "android_vr", status: "OK", directAudio: 4, best: "itag 140 mp4a.40.2 130 kbps" },
  { id: "tv", status: "UNPLAYABLE", directAudio: 0, best: null },
  { id: "tv_embedded", status: "ERROR", directAudio: 0, best: null },
  { id: "mweb", status: "UNPLAYABLE", directAudio: 0, best: null },
  { id: "web_remix", status: "UNPLAYABLE", directAudio: 0, best: null },
];

/** Paleta de ejemplo, tomada de una portada violeta/neon como la de la captura. */
const palette: Palette = {
  background: "#1a1030",
  backgroundAlt: "#2a1a4a",
  accent: "#c77dff",
  foreground: "#f6f0ff",
  isLight: false,
};

const LINES = [
  "Tell me why you keep on running",
  "It is like I am constantly questioning you",
  "So if you want me",
  "Why are you tearing me apart?",
  "It is killing me slowly",
  "I am such a fool for you",
];

const lyrics: Lyrics = {
  synced: true,
  source: "LRCLIB",
  plain: null,
  lines: LINES.map((text, i) => ({
    startMs: 12_000 + i * 7_000,
    endMs: 12_000 + (i + 1) * 7_000,
    text,
  })),
};

export const mockApi = {
  search: async (_q: string) => results,
  playQueue: async (_t: unknown[], start: number) => {
    state.queueIndex = start;
    state.track = state.queue[start] ?? null;
    state.positionMs = 0;
    emit();
  },
  playNow: async () => {},
  togglePlay: async () => {
    state.playing = !state.playing;
    emit();
  },
  next: async () => {
    state.queueIndex = (state.queueIndex + 1) % state.queue.length;
    state.track = state.queue[state.queueIndex];
    state.positionMs = 0;
    emit();
  },
  prev: async () => {
    state.queueIndex = (state.queueIndex - 1 + state.queue.length) % state.queue.length;
    state.track = state.queue[state.queueIndex];
    state.positionMs = 0;
    emit();
  },
  jumpTo: async (i: number) => {
    state.queueIndex = i;
    state.track = state.queue[i];
    state.positionMs = 0;
    emit();
  },
  seek: async (ms: number) => {
    state.positionMs = ms;
    emit();
  },
  setVolume: async (v: number) => {
    state.volume = v;
    emit();
  },
  setRepeat: async (m: PlaybackState["repeat"]) => {
    state.repeat = m;
    emit();
  },
  setShuffle: async (on: boolean) => {
    state.shuffle = on;
    emit();
  },
  getState: async () => ({ ...state }),
  getPalette: async () => palette,
  getLyrics: async () => lyrics,
  diagnose: async () => health,
  minimize: async () => {},
  toggleMaximize: async () => {},
  close: async () => {},
  onPlayback: async (cb: (s: PlaybackState) => void) => {
    listeners.push(cb);
    cb({ ...state });
    return () => {};
  },
};
