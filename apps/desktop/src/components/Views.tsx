import { For, Show, createEffect, createSignal, on, onMount } from "solid-js";
import { api, thumbAt, type ClientHealth, type SavedTrack } from "../lib/api";
import {
  playback,
  results,
  searching,
  view,
  setView,
  query,
  setQuery,
  runSearch,
  playFromResults,
  coverUrl,
  playSaved,
} from "../lib/store";
import * as I from "./Icons";

/* ---------------------------------------------------------------- Titlebar */

export function TitleBar() {
  return (
    <header class="drag-region flex h-11 shrink-0 items-center gap-3 px-3">
      <div class="no-drag flex items-center gap-2 pl-1">
        <div
          class="grid size-6 place-items-center rounded-md"
          style={{ background: "var(--accent)" }}
        >
          <I.Music size={14} class="text-black/70" />
        </div>
        <span class="text-[13px] font-semibold tracking-tight">Posible</span>
      </div>

      <SearchBox />

      <div class="no-drag ml-auto flex items-center gap-0.5">
        <button class="icon-btn size-8" onClick={() => api.minimize()} title="Minimizar">
          <I.Minimize size={16} />
        </button>
        <button class="icon-btn size-8" onClick={() => api.toggleMaximize()} title="Maximizar">
          <I.Maximize size={14} />
        </button>
        <button
          class="icon-btn size-8 hover:!bg-red-500/80"
          onClick={() => api.close()}
          title="Cerrar"
        >
          <I.Close size={16} />
        </button>
      </div>
    </header>
  );
}

function SearchBox() {
  let input!: HTMLInputElement;
  return (
    <form
      class="no-drag mx-auto w-full max-w-md"
      onSubmit={(e) => {
        e.preventDefault();
        runSearch(query());
        input.blur();
      }}
    >
      <div class="chip flex items-center gap-2.5 px-3.5 py-2 focus-within:border-white/20 focus-within:bg-white/[0.09]">
        <I.Search size={15} class="shrink-0 opacity-60" />
        <input
          ref={input}
          value={query()}
          onInput={(e) => setQuery(e.currentTarget.value)}
          placeholder="Buscar canciones, álbumes, artistas"
          class="w-full bg-transparent text-[13px] outline-none placeholder:opacity-45"
        />
      </div>
    </form>
  );
}

/* ----------------------------------------------------------------- Sidebar */

export function Sidebar() {
  const items = [
    { id: "home" as const, label: "Inicio", icon: I.Home },
    { id: "search" as const, label: "Buscar", icon: I.Search },
    { id: "library" as const, label: "Biblioteca", icon: I.Library },
    { id: "diagnostics" as const, label: "Diagnóstico", icon: I.Stethoscope },
  ];

  return (
    <nav class="flex w-[186px] shrink-0 flex-col gap-1 py-2 pl-3 pr-1">
      <For each={items}>
        {(item) => (
          <button
            class="flex items-center gap-3 rounded-xl px-3 py-2.5 text-left text-[13px] transition-colors"
            classList={{
              "bg-[var(--panel-strong)] font-medium": view() === item.id,
              "opacity-65 hover:opacity-100 hover:bg-[var(--panel)]": view() !== item.id,
            }}
            onClick={() => setView(item.id)}
          >
            <item.icon size={18} />
            {item.label}
          </button>
        )}
      </For>

      <div class="mt-auto px-3 pb-2 text-[10.5px] leading-relaxed opacity-25">
        Modo anónimo. Sin sesión iniciada.
      </div>
    </nav>
  );
}

/* -------------------------------------------------------------- Now playing */

export function NowPlaying() {
  return (
    <div class="fade-in flex h-full flex-col items-center justify-center gap-6 px-8">
      <Show
        when={playback.track}
        fallback={
          <div class="text-center opacity-35">
            <I.Music size={44} class="mx-auto mb-4 opacity-60" />
            <p class="text-[13px]">Nada sonando todavía</p>
          </div>
        }
      >
        <div class="w-full max-w-[420px]">
          <Show
            when={coverUrl()}
            fallback={<div class="cover bg-white/8" classList={{ paused: !playback.playing }} />}
          >
            <img
              src={coverUrl()!}
              alt={`Portada de ${playback.track!.title}`}
              class="cover"
              classList={{ paused: !playback.playing }}
            />
          </Show>
        </div>
        <div class="max-w-[440px] text-center">
          <h1 class="text-balance text-[22px] font-semibold leading-tight tracking-tight">
            {playback.track!.title}
          </h1>
          <p class="mt-1.5 text-[13px] opacity-55">{playback.track!.author}</p>
        </div>
      </Show>
    </div>
  );
}

/* ------------------------------------------------------------ Search results */

export function SearchView() {
  return (
    <div class="scroll-area h-full px-6 py-4">
      <Show when={!searching()} fallback={<SkeletonList />}>
        <Show
          when={results().length > 0}
          fallback={
            <p class="mt-16 text-center text-[13px] opacity-40">
              {query() ? "Sin resultados." : "Escribe algo arriba para buscar."}
            </p>
          }
        >
          <div class="fade-in flex flex-col gap-0.5">
            <For each={results()}>
              {(r, i) => {
                const active = () => playback.track?.videoId === r.videoId;
                return (
                  <button
                    class="group flex items-center gap-3 rounded-xl px-3 py-2 text-left transition-colors hover:bg-[var(--panel)]"
                    classList={{ "bg-[var(--panel)]": active() }}
                    onDblClick={() => playFromResults(i())}
                    onClick={() => playFromResults(i())}
                  >
                    <div class="relative shrink-0">
                      <Show
                        when={r.thumbnail}
                        fallback={<div class="size-11 rounded-lg bg-white/8" />}
                      >
                        <img
                          src={thumbAt(r.thumbnail, 96)!}
                          alt=""
                          class="size-11 rounded-lg object-cover"
                        />
                      </Show>
                      <div class="absolute inset-0 grid place-items-center rounded-lg bg-black/45 opacity-0 transition-opacity group-hover:opacity-100">
                        <I.Play size={16} />
                      </div>
                    </div>
                    <div class="min-w-0 flex-1">
                      <div
                        class="truncate text-[13px]"
                        classList={{ "text-[var(--accent)] font-medium": active() }}
                      >
                        {r.title}
                      </div>
                      <div class="truncate text-[11.5px] opacity-50">{r.subtitle}</div>
                    </div>
                    <span class="shrink-0 text-[11px] tabular-nums opacity-40">
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
  const [running, setRunning] = createSignal(false);

  const run = async () => {
    setRunning(true);
    try {
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
