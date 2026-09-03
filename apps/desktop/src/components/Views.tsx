import { For, Show, createEffect, createSignal, on, onMount } from "solid-js";
import { api, thumbAt, type ClientHealth, type ExtractorStatus, type SavedTrack } from "../lib/api";
import {
  playback,
  results,
  searching,
  view,
  setView,
  query,
  setQuery,
  runSearch,
  resultsLabel,
  playFromResults,
  coverUrl,
  playSaved,
} from "../lib/store";
import * as I from "./Icons";

/* ---------------------------------------------------------------- Titlebar */

export function TitleBar() {
  return (
    <header data-tauri-drag-region class="drag-region flex h-14 shrink-0 items-center justify-between px-4 z-20">
      {/* Izquierda: Menú hamburguesa + Logo YouTube Music */}
      <div class="no-drag flex items-center gap-3">
        <button class="icon-btn size-9 text-white/80 hover:text-white hover:bg-white/10" title="Menú">
          <I.Menu size={20} />
        </button>
        <div
          class="flex items-center gap-2 cursor-pointer select-none"
          onClick={() => setView("home")}
          title="Posible Music"
        >
          <div class="size-7 rounded-full bg-[#ff0000] flex items-center justify-center shadow-[0_0_15px_rgba(255,0,0,0.5)]">
            <svg viewBox="0 0 24 24" class="size-4 fill-white translate-x-[1px]">
              <polygon points="6 4 18 12 6 20 6 4" />
            </svg>
          </div>
          <span class="text-[19px] font-bold tracking-tight text-white font-sans">Music</span>
        </div>
      </div>

      {/* Centro: Buscador estilo píldora ancha */}
      <SearchBox />

      {/* Derecha: Avatar + Controles de ventana de Windows */}
      <div class="no-drag flex items-center gap-4">
        <div class="size-8 rounded-full bg-gradient-to-tr from-amber-500 via-rose-500 to-purple-600 ring-2 ring-white/20 flex items-center justify-center text-xs font-bold text-white shadow-md cursor-pointer hover:scale-105 transition-transform">
          A
        </div>
        <div class="flex items-center gap-0.5">
          <button class="icon-btn size-8 text-white/70 hover:text-white" onClick={() => api.minimize()} title="Minimizar">
            <I.Minimize size={14} />
          </button>
          <button class="icon-btn size-8 text-white/70 hover:text-white" onClick={() => api.toggleMaximize()} title="Maximizar">
            <I.Maximize size={12} />
          </button>
          <button class="icon-btn size-8 text-white/70 hover:!bg-red-600 hover:text-white" onClick={() => api.close()} title="Cerrar">
            <I.Close size={15} />
          </button>
        </div>
      </div>
    </header>
  );
}

function SearchBox() {
  let input!: HTMLInputElement;
  return (
    <form
      class="no-drag flex-1 max-w-[540px] mx-auto px-4"
      onSubmit={(e) => {
        e.preventDefault();
        runSearch(query());
        input.blur();
      }}
    >
      <div class="flex items-center gap-3 px-4 py-2 rounded-full bg-white/[0.08] hover:bg-white/[0.12] focus-within:bg-white/[0.16] focus-within:ring-1 focus-within:ring-white/25 transition-all border border-white/5">
        <I.Search size={16} class="shrink-0 text-white/50" />
        <input
          ref={input}
          value={query()}
          onInput={(e) => setQuery(e.currentTarget.value)}
          placeholder="Search songs, albums, artists, podcasts"
          class="w-full bg-transparent text-sm text-white placeholder:text-white/40 outline-none"
        />
      </div>
    </form>
  );
}

/* ----------------------------------------------------------------- Sidebar */

export function Sidebar() {
  const items = [
    { id: "home" as const, label: "Home", icon: I.Home },
    { id: "explore" as const, label: "Explore", icon: I.Compass },
    { id: "library" as const, label: "Library", icon: I.Library },
  ];

  return (
    <nav class="flex w-[210px] shrink-0 flex-col gap-1 py-2 pl-3 pr-2 z-10 select-none">
      {/* Navegación principal */}
      <div class="space-y-1">
        <For each={items}>
          {(item) => {
            const isActive = () =>
              (item.id === "home" && view() === "home") || (item.id === "library" && view() === "library");
            return (
              <button
                class="flex w-full items-center gap-4 rounded-xl px-3.5 py-2.5 text-left text-sm font-medium transition-all"
                classList={{
                  "bg-white/15 text-white shadow-sm font-semibold": isActive(),
                  "text-white/70 hover:text-white hover:bg-white/10": !isActive(),
                }}
                onClick={() => {
                  if (item.id === "home") setView("home");
                  else if (item.id === "library") setView("library");
                  else setView("search");
                }}
              >
                <item.icon size={20} class={isActive() ? "text-white" : "text-white/70"} />
                {item.label}
              </button>
            );
          }}
        </For>
      </div>

      {/* Botón + New playlist */}
      <div class="pt-4 pb-2 px-1">
        <button
          class="flex w-full items-center justify-center gap-2 rounded-full bg-white/10 hover:bg-white/15 active:scale-95 text-xs font-semibold py-2 px-3 text-white transition-all border border-white/5"
          onClick={() => setView("library")}
        >
          <I.Plus size={15} />
          New playlist
        </button>
      </div>

      <div class="my-2 h-[1px] bg-white/10 mx-2" />

      {/* Listas de reproducción estilo glassy-music */}
      <div class="scroll-area flex-1 px-1 space-y-1 overflow-y-auto">
        <button
          class="flex w-full flex-col rounded-xl px-2.5 py-2 text-left transition-colors hover:bg-white/10"
          onClick={() => setView("library")}
        >
          <div class="flex items-center gap-1.5 text-xs font-semibold text-white">
            <I.Pin size={12} class="text-[var(--accent)] shrink-0" />
            <span>Liked Music</span>
          </div>
          <span class="text-[10.5px] text-white/45 pl-4">Auto playlist</span>
        </button>

        <button
          class="flex w-full flex-col rounded-xl px-2.5 py-2 text-left transition-colors hover:bg-white/10"
          onClick={() => setView("diagnostics")}
        >
          <div class="flex items-center gap-1.5 text-xs font-medium text-white/80">
            <I.Stethoscope size={14} class="text-white/50 shrink-0" />
            <span>Diagnóstico</span>
          </div>
          <span class="text-[10.5px] text-white/45 pl-5">Estado de red y audio</span>
        </button>
      </div>

      <div class="mt-auto px-3 pb-2 text-[11px] leading-relaxed text-white/30">
        Modo anónimo
      </div>
    </nav>
  );
}

/* -------------------------------------------------------------- Home Feed */

export function HomeFeed() {
  const chips = [
    "Podcasts", "Energize", "Feel good", "Relax", "Commute", "Workout", "Party", "Focus", "Sad", "Romance", "Sleep"
  ];
  const [activeChip, setActiveChip] = createSignal("Relax");
  const [historyTracks, setHistoryTracks] = createSignal<SavedTrack[]>([]);

  onMount(async () => {
    try {
      const hist = await api.history();
      setHistoryTracks(hist);
    } catch {}
  });

  return (
    <div class="scroll-area flex-1 h-full px-8 py-6 space-y-8 overflow-y-auto">
      {/* Fila de píldoras de estado de ánimo estilo 1.mp4 / 3.webp */}
      <div class="flex items-center gap-2 overflow-x-auto pb-2 scrollbar-none">
        <For each={chips}>
          {(c) => (
            <button
              class="shrink-0 rounded-full px-4 py-1.5 text-xs font-semibold transition-all border"
              classList={{
                "bg-white/20 border-white/25 text-white shadow-sm": activeChip() === c,
                "bg-white/[0.05] border-white/10 text-white/60 hover:text-white hover:bg-white/10": activeChip() !== c,
              }}
              onClick={() => setActiveChip(c)}
            >
              {c}
            </button>
          )}
        </For>
      </div>

      {/* Sección 1: Listen again */}
      <div>
        <div class="flex items-center justify-between mb-4">
          <div>
            <span class="text-[10.5px] uppercase font-bold tracking-widest text-white/40">NANIKILL</span>
            <h2 class="text-2xl font-bold text-white tracking-tight">Listen again</h2>
          </div>
          <button class="rounded-full bg-white/10 hover:bg-white/15 px-3.5 py-1 text-xs font-semibold text-white border border-white/10">
            More
          </button>
        </div>

        <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-4">
          <Show
            when={historyTracks().length > 0}
            fallback={
              <div class="col-span-full py-12 text-center text-sm text-white/40">
                Pega el enlace de una canción o busca arriba para empezar a escuchar.
              </div>
            }
          >
            <For each={historyTracks().slice(0, 6)}>
              {(track) => (
                <div
                  class="group flex flex-col cursor-pointer select-none"
                  onClick={() => api.playNow(track.videoId)}
                >
                  <div class="relative aspect-square w-full rounded-xl overflow-hidden ring-1 ring-white/10 shadow-lg group-hover:shadow-2xl transition-all">
                    <img
                      src={thumbAt(track.thumbnail, 320)!}
                      alt=""
                      class="size-full object-cover group-hover:scale-105 transition-transform duration-300"
                    />
                    <div class="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                      <div class="size-11 rounded-full bg-white text-black flex items-center justify-center shadow-xl">
                        <I.Play size={20} class="translate-x-[1px]" />
                      </div>
                    </div>
                  </div>
                  <div class="mt-2.5 truncate text-xs font-bold text-white group-hover:text-[var(--accent)]">
                    {track.title}
                  </div>
                  <div class="truncate text-[11px] text-white/50 font-medium">
                    {track.author}
                  </div>
                </div>
              )}
            </For>
          </Show>
        </div>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------ Search results */

export function SearchView() {
  const searchChips = ["Songs", "Videos", "Featured playlists", "Albums", "Artists", "Community playlists", "Episodes", "Podcasts"];
  const [activeChip, setActiveChip] = createSignal("Songs");

  return (
    <div class="scroll-area h-full px-8 py-6 space-y-5">
      {/* Selector de chips estilo 6.webp */}
      <div class="flex items-center gap-2 overflow-x-auto pb-1 scrollbar-none">
        <For each={searchChips}>
          {(c) => (
            <button
              class="shrink-0 rounded-full px-3.5 py-1 text-xs font-semibold transition-all border"
              classList={{
                "bg-white/20 border-white/25 text-white shadow-sm": activeChip() === c,
                "bg-white/[0.04] border-white/10 text-white/60 hover:text-white hover:bg-white/10": activeChip() !== c,
              }}
              onClick={() => setActiveChip(c)}
            >
              {c}
            </button>
          )}
        </For>
      </div>

      <Show when={!searching()} fallback={<SkeletonList />}>
        <Show
          when={results().length > 0}
          fallback={
            <p class="mt-16 text-center text-sm text-white/40">
              {query() ? "Sin resultados." : "Escribe algo arriba para buscar."}
            </p>
          }
        >
          <Show when={resultsLabel()}>
            <div class="mb-3 flex items-center justify-between gap-3">
              <h2 class="min-w-0 truncate text-base font-bold text-white">{resultsLabel()}</h2>
              <button class="rounded-full bg-white/10 hover:bg-white/15 px-4 py-1.5 text-xs font-semibold text-white border border-white/10" onClick={() => playFromResults(0)}>
                Reproducir todo
              </button>
            </div>
          </Show>
          <div class="fade-in flex flex-col gap-1">
            <For each={results()}>
              {(r, i) => {
                const active = () => playback.track?.videoId === r.videoId;
                return (
                  <button
                    class="group flex items-center gap-3.5 rounded-xl px-3.5 py-2.5 text-left transition-all border border-transparent"
                    classList={{
                      "bg-white/[0.14] border-white/10 shadow-sm": active(),
                      "hover:bg-white/[0.06] hover:border-white/5": !active(),
                    }}
                    onDblClick={() => playFromResults(i())}
                    onClick={() => playFromResults(i())}
                  >
                    <div class="relative shrink-0 size-11 rounded-lg overflow-hidden ring-1 ring-white/10 shadow-sm">
                      <Show
                        when={r.thumbnail}
                        fallback={<div class="size-full bg-white/8" />}
                      >
                        <img
                          src={thumbAt(r.thumbnail, 96)!}
                          alt=""
                          class="size-full object-cover"
                        />
                      </Show>
                      <div class="absolute inset-0 grid place-items-center bg-black/45 opacity-0 transition-opacity group-hover:opacity-100">
                        <I.Play size={16} class="text-white" />
                      </div>
                    </div>
                    <div class="min-w-0 flex-1">
                      <div
                        class="truncate text-sm font-semibold text-white"
                        classList={{ "text-[var(--accent)]": active() }}
                      >
                        {r.title}
                      </div>
                      <div class="truncate text-xs text-white/60 font-medium mt-0.5">{r.subtitle}</div>
                    </div>
                    <span class="shrink-0 text-xs tabular-nums text-white/50 font-medium">
                      {r.duration ?? ""}
                    </span>
                  </button>
                );
              }}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}

function SkeletonList() {
  return (
    <div class="flex flex-col gap-0.5">
      <For each={Array(8).fill(0)}>
        {(_, i) => (
          <div
            class="flex items-center gap-3 px-3 py-2"
            style={{ opacity: String(1 - i() * 0.1) }}
          >
            <div class="size-11 shrink-0 animate-pulse rounded-lg bg-white/8" />
            <div class="flex-1">
              <div class="h-3 w-2/5 animate-pulse rounded bg-white/8" />
              <div class="mt-2 h-2.5 w-1/4 animate-pulse rounded bg-white/6" />
            </div>
          </div>
        )}
      </For>
    </div>
  );
}

/* ---------------------------------------------------------------- Library */

/** Favoritos e historial, ambos guardados en SQLite local. */
export function LibraryView() {
  const [tab, setTab] = createSignal<"favorites" | "history">("favorites");
  const [rows, setRows] = createSignal<SavedTrack[]>([]);

  const load = async () => {
    try {
      setRows(tab() === "favorites" ? await api.favorites() : await api.history());
    } catch {
      setRows([]);
    }
  };

  createEffect(on(tab, load));

  return (
    <div class="flex h-full flex-col">
      <div class="flex shrink-0 gap-1 px-5 pt-4">
        <For each={[["favorites", "Favoritos"], ["history", "Historial"]] as const}>
          {([id, label]) => (
            <button
              class="rounded-lg px-3.5 py-1.5 text-[12px] font-medium transition-colors"
              classList={{
                "bg-[var(--panel-strong)]": tab() === id,
                "opacity-50 hover:opacity-80": tab() !== id,
              }}
              onClick={() => setTab(id)}
            >
              {label}
            </button>
          )}
        </For>
      </div>

      <div class="scroll-area flex-1 px-4 py-3">
        <Show
          when={rows().length > 0}
          fallback={
            <p class="mt-14 text-center text-[13px] opacity-40">
              {tab() === "favorites"
                ? "Aún no has guardado nada. Usa el corazón del reproductor."
                : "Todavía no has escuchado nada."}
            </p>
          }
        >
          <div class="fade-in flex flex-col gap-0.5">
            <For each={rows()}>
              {(t, i) => (
                <button
                  class="group flex items-center gap-3 rounded-xl px-3 py-2 text-left transition-colors hover:bg-[var(--panel)]"
                  classList={{ "bg-[var(--panel)]": playback.track?.videoId === t.videoId }}
                  onClick={() => playSaved(rows(), i())}
                >
                  <Show when={t.thumbnail} fallback={<div class="size-11 rounded-lg bg-white/8" />}>
                    <img src={thumbAt(t.thumbnail, 96)!} alt="" class="size-11 rounded-lg object-cover" />
                  </Show>
                  <div class="min-w-0 flex-1">
                    <div class="truncate text-[13px]">{t.title}</div>
                    <div class="truncate text-[11.5px] opacity-50">{t.author}</div>
                  </div>
                </button>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------- Diagnostics */

/**
 * Equivalente en la app a `ytm-spike probe`.
 *
 * Es la funcionalidad que separa este proyecto de las alternativas: cuando algo
 * deja de sonar, el usuario ve al instante si YouTube cerro un cliente, en vez
 * de quedarse con una app rota y sin explicacion.
 */
export function Diagnostics() {
  const [rows, setRows] = createSignal<ClientHealth[]>([]);
  const [extractor, setExtractor] = createSignal<ExtractorStatus | null>(null);
  const [running, setRunning] = createSignal(false);

  const run = async () => {
    setRunning(true);
    try {
      api.extractorStatus().then(setExtractor).catch(() => setExtractor(null));
      setRows(await api.diagnose());
    } finally {
      setRunning(false);
    }
  };

  onMount(run);

  return (
    <div class="scroll-area h-full px-6 py-4">
      <div class="mb-1 flex items-center justify-between">
        <h2 class="text-[15px] font-semibold">Estado de los clientes</h2>
        <button class="chip px-3 py-1.5 text-[12px]" onClick={run} disabled={running()}>
          {running() ? "Comprobando…" : "Volver a comprobar"}
        </button>
      </div>
      <p class="mb-4 max-w-lg text-[12px] leading-relaxed opacity-45">
        YouTube cierra clientes cada pocos meses. Si la música deja de sonar, aquí se ve cuál
        sigue en pie.
      </p>

      {/* El extractor real es yt-dlp; los clientes de abajo son el respaldo. */}
      <div class="panel mb-4 flex items-center gap-3 px-4 py-3">
        <span
          class="size-2.5 shrink-0 rounded-full"
          style={{ background: extractor()?.available ? "#4ade80" : "#f87171" }}
        />
        <div class="min-w-0 flex-1">
          <div class="text-[13px] font-semibold">
            Extractor: yt-dlp{" "}
            <Show when={extractor()?.version}>
              <span class="font-normal opacity-60">{extractor()!.version}</span>
            </Show>
          </div>
          <div class="truncate text-[11px] opacity-45">
            <Show
              when={extractor()?.available}
              fallback="No encontrado: la reproducción queda capada a ~48 s por pista."
            >
              {extractor()!.program}
            </Show>
          </div>
        </div>
      </div>

      <Show when={rows().length > 0} fallback={<p class="text-[13px] opacity-40">Comprobando…</p>}>
        <div class="fade-in flex flex-col gap-1">
          <For each={rows()}>
            {(r) => {
              const ok = () => r.status === "OK" && r.directAudio > 0;
              return (
                <div class="panel flex items-center gap-3 px-4 py-2.5">
                  <span
                    class="size-2 shrink-0 rounded-full"
                    style={{ background: ok() ? "#4ade80" : "#f87171" }}
                  />
                  <span class="w-28 shrink-0 font-mono text-[12px]">{r.id}</span>
                  <span class="w-32 shrink-0 text-[12px] opacity-60">{r.status}</span>
                  <span class="flex-1 truncate text-[12px] opacity-45">
                    {r.best ?? (ok() ? "" : "sin audio directo")}
                  </span>
                </div>
              );
            }}
          </For>
        </div>
      </Show>
    </div>
  );
}
