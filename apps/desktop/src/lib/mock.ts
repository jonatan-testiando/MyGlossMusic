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
import type { ClientHealth, Lyrics, Palette, PlaybackState, Playlist, SavedTrack, SearchResult, ShelfItem, Track } from "./api";

/**
 * "3:28" a milisegundos.
 *
 * Duplica a propósito el `parseDuration` de `api.ts`. Importarlo de allí cierra
 * un círculo — `api.ts` importa este módulo para elegir entre backend real y
 * simulado — y según el orden de evaluación revienta con "Cannot access
 * 'mockApi' before initialization". Los tipos sí se importan, porque las
 * importaciones de tipo desaparecen al compilar y no crean dependencia.
 */
function duracion(texto: string): number | null {
  const partes = texto.split(":").map(Number);
  if (partes.length < 2 || partes.some((n) => !Number.isFinite(n))) return null;
  return partes.reduce((total, n) => total * 60 + n, 0) * 1000;
}

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
  track: {
    videoId: TRACKS[0].videoId,
    title: TRACKS[0].title,
    author: TRACKS[0].author,
    thumbnail: COVER,
    durationMs: duracion(TRACKS[0].d),
  },
  playing: true,
  loading: false,
  positionMs: 23_000,
  durationMs: 208_000,
  volume: 0.8,
  buffered: 1,
  queue: TRACKS.map((t) => ({
    videoId: t.videoId,
    title: t.title,
    author: t.author,
    thumbnail: COVER,
    durationMs: duracion(t.d),
  })),
  queueIndex: 0,
  queueRev: 1,
  repeat: "off",
  shuffle: false,
  error: null,
};

const listeners: ((s: PlaybackState) => void)[] = [];

/** Playlists locales de ejemplo, en memoria. */
const listas: Playlist[] = [{ id: 1, name: "Para programar", count: 2, thumbnail: COVER }];
const contenidos = new Map<number, SavedTrack[]>([
  [
    1,
    TRACKS.slice(0, 2).map((t) => ({
      videoId: t.videoId,
      title: t.title,
      author: t.author,
      thumbnail: COVER,
      at: 0,
    })),
  ],
]);
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
  // Calculados con la misma aritmetica que `palette.rs`: luminosidad dentro de
  // [0.52, 0.78] y croma empujado a 0.19. Si el mock trae colores mas apagados
  // que los que da el backend, el diseno se ajusta contra una mentira.
  stops: [
    { color: "#0085af", weight: 0.45 },
    { color: "#0073a1", weight: 0.34 },
    { color: "#c2b5ac", weight: 0.10 },
    { color: "#c1b4b1", weight: 0.06 },
    { color: "#8da1a4", weight: 0.05 },
  ],
  background: "#00232f",
  backgroundAlt: "#0a3a4a",
  accent: "#3fc9f0",
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

const saved: SavedTrack[] = TRACKS.slice(0, 4).map((t, i) => ({
  videoId: t.videoId,
  title: t.title,
  author: t.author,
  thumbnail: COVER,
  at: Math.floor(Date.now() / 1000) - i * 3600,
}));
let favs: SavedTrack[] = saved.slice(0, 2);

export const mockApi = {
  playlist: async () => ({ title: "Playlist de prueba", tracks: results }),
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
  search: async (_q: string, params?: string) => ({
    items: (TRACKS.map((t) => ({
      kind: "track" as const,
      id: t.videoId,
      title: t.title,
      subtitle: t.author,
      thumbnail: COVER,
      duration: t.d,
    })) as ShelfItem[]).concat(
      params
        ? []
        : [
            { kind: "artist" as const, id: "UC1", title: "Kastra", subtitle: "Artista", thumbnail: COVER, duration: null },
            { kind: "album" as const, id: "MPREb_1", title: "Fool For You", subtitle: "Single • Kastra", thumbnail: COVER, duration: null },
            { kind: "playlist" as const, id: "VLPL1", title: "Kastra Mix", subtitle: "Lista • 1,2 M", thumbnail: COVER, duration: null },
          ],
    ),
    // El prefijo `EgWKAQII` es el que YouTube usa de verdad para "canciones";
    // el relevo de la búsqueda lo busca por ahí, no por la etiqueta.
    chips: [
      { label: "Canciones", params: "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D" },
      { label: "Vídeos", params: "EgWKAQIQAQ%3D%3D" },
      { label: "Artistas", params: "EgWKAQIgAQ%3D%3D" },
      { label: "Álbumes", params: "EgWKAQIYAQ%3D%3D" },
    ],
    // Como en la realidad: "Todo" no pagina, los filtros sí.
    continuation: params ? "token-mock" : null,
  }),
  searchMore: async () => ({
    items: TRACKS.slice(0, 3).map((t) => ({
      kind: "track" as const,
      id: `${t.videoId}-2`,
      title: `${t.title} (más)`,
      subtitle: t.author,
      thumbnail: COVER,
      duration: t.d,
    })),
    chips: [],
    continuation: null,
  }),

  home: async () => ({
    title: null,
    subtitle: null,
    thumbnail: null,
    shelves: [
      {
        title: "Listas de reproducción de la comunidad populares",
        items: TRACKS.slice(0, 5).map((t) => ({
          kind: "playlist" as const,
          id: `VL${t.videoId}`,
          title: `Mix ${t.title}`,
          subtitle: "6,8 M de visualizaciones",
          thumbnail: COVER,
          duration: null,
        })),
      },
    ],
  }),
  // Con la forma que devuelve `browse` de verdad para un artista: cabecera, una
  // lista de pistas (sin título, como los álbumes) y un carrusel.
  browse: async (browseId: string) => ({
    title: browseId.startsWith("MPRE") ? "Fool For You" : browseId.startsWith("VL") ? "viejitas" : "Kastra",
    subtitle: browseId.startsWith("MPRE")
      ? "Single • 2021"
      : browseId.startsWith("VL")
        ? "Lista de reproducción • 2023"
        : "1,25 M de oyentes mensuales",
    secondSubtitle: browseId.startsWith("VL")
      ? "20 M de visualizaciones • 88 pistas • 5 horas y 59 minutos"
      : null,
    description: browseId.startsWith("VL") ? "Las de siempre, hechas con cariño." : null,
    thumbnail: COVER,
    shelves: [
      {
        title: "Canciones populares",
        items: TRACKS.slice(0, 5).map((t) => ({
          kind: "track" as const,
          id: t.videoId,
          title: t.title,
          subtitle: t.author,
          thumbnail: COVER,
          duration: t.d,
        })),
      },
      {
        title: "Álbumes",
        items: TRACKS.slice(0, 6).map((t, i) => ({
          kind: "album" as const,
          id: `MPREb_${i}`,
          title: t.title,
          subtitle: `Single • ${2020 + i}`,
          thumbnail: COVER,
          duration: null,
        })),
      },
    ],
  }),

  radio: async () => ({
    playlistId: "RDAMVMjig2aRZbHm4",
    artistBrowseId: "UCartista123",
    tracks: TRACKS.slice(1).map((t) => ({
      videoId: t.videoId,
      title: t.title,
      subtitle: t.author,
      duration: t.d,
      thumbnail: COVER,
    })),
  }),
  setUpNext: async () => {},

  searchSuggestions: async (q: string) => {
    const base = TRACKS.map((t) => t.title.toLowerCase());
    const extra = ["remix", "en directo", "1 hora", "slowed"];
    return base
      .filter((t) => t.includes(q.toLowerCase().trim()))
      .flatMap((t) => [t, ...extra.map((e) => `${t} ${e}`)])
      .slice(0, 10);
  },

  getPalette: async () => palette,
  getLyrics: async () => lyrics,
  toggleFavorite: async () => {
    favs = favs.length ? [] : saved;
    return favs.length > 0;
  },
  isFavorite: async () => favs.length > 0,
  favorites: async () => favs,

  createPlaylist: async (name: string) => {
    const id = listas.reduce((max, l) => Math.max(max, l.id), 0) + 1;
    listas.push({ id, name, count: 0, thumbnail: null });
    contenidos.set(id, []);
    return id;
  },
  renamePlaylist: async (id: number, name: string) => {
    const l = listas.find((x) => x.id === id);
    if (l) l.name = name;
  },
  deletePlaylist: async (id: number) => {
    const i = listas.findIndex((x) => x.id === id);
    if (i >= 0) listas.splice(i, 1);
    contenidos.delete(id);
  },
  // Copias PROFUNDAS, no la referencia interna. `invoke` de Tauri deserializa
  // JSON, así que devuelve objetos nuevos en cada llamada. Solid compara por
  // referencia: si el mock reutilizara los suyos, `For` no volvería a pintar
  // una fila cuyo contador ha cambiado y el fallo solo aparecería con datos
  // simulados — que es la peor clase de fallo.
  playlists: async () => listas.map((l) => ({ ...l })),
  playlistTracks: async (id: number) => (contenidos.get(id) ?? []).map((t) => ({ ...t })),
  addToPlaylist: async (id: number, t: Partial<Track>) => {
    const lista = contenidos.get(id) ?? [];
    if (lista.some((x) => x.videoId === t.videoId)) return;
    lista.push({
      videoId: t.videoId!,
      title: t.title ?? "Sin título",
      author: t.author ?? "Desconocido",
      thumbnail: t.thumbnail ?? null,
      at: 0,
    });
    contenidos.set(id, lista);
    const l = listas.find((x) => x.id === id);
    if (l) {
      l.count = lista.length;
      l.thumbnail = l.thumbnail ?? t.thumbnail ?? null;
    }
  },
  removeFromPlaylist: async (id: number, videoId: string) => {
    const lista = (contenidos.get(id) ?? []).filter((x) => x.videoId !== videoId);
    contenidos.set(id, lista);
    const l = listas.find((x) => x.id === id);
    if (l) l.count = lista.length;
  },
  history: async () => saved,
  diagnose: async () => health,
  extractorStatus: async () => ({ available: true, version: "2026.08.19", program: "yt-dlp.exe (sidecar)" }),
  minimize: async () => {},
  toggleMaximize: async () => {},
  close: async () => {},
  onPlayback: async (cb: (s: PlaybackState) => void) => {
    listeners.push(cb);
    cb({ ...state });
    return () => {};
  },
};
