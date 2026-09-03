import { For, Show, createEffect, createSignal, on } from "solid-js";
import { api, thumbAt } from "../lib/api";
import { playback, lyrics, lyricsLoading, position, setFullLyricsOpen } from "../lib/store";
import * as I from "./Icons";

/** Panel lateral derecho: Pestañas A Continuación (Cola) y Letra. */
export function SidePanel() {
  const [tab, setTab] = createSignal<"queue" | "lyrics">("queue");

  return (
    <aside class="panel flex w-[410px] shrink-0 flex-col overflow-hidden shadow-2xl">
      {/* Cabecera con pestañas de cristal escarchado */}
      <div class="flex shrink-0 items-center justify-between border-b border-white/10 p-3">
        <div class="flex items-center gap-1.5 rounded-xl bg-white/[0.04] p-1 border border-white/5">
          <button
            class="rounded-lg px-3 py-1 text-[11.5px] font-semibold tracking-wider uppercase transition-all duration-200"
            classList={{
              "bg-white/15 text-white shadow-sm": tab() === "queue",
              "text-white/50 hover:text-white/80": tab() !== "queue",
            }}
            onClick={() => setTab("queue")}
          >
            A continuación
          </button>
          <button
            class="rounded-lg px-3 py-1 text-[11.5px] font-semibold tracking-wider uppercase transition-all duration-200"
            classList={{
              "bg-white/15 text-white shadow-sm": tab() === "lyrics",
              "text-white/50 hover:text-white/80": tab() !== "lyrics",
            }}
            onClick={() => setTab("lyrics")}
          >
            Letra
          </button>
        </div>

        <Show when={tab() === "lyrics"}>
          <button
            class="icon-btn size-7 opacity-60 hover:opacity-100 hover:bg-white/10"
            onClick={() => setFullLyricsOpen(true)}
            title="Pantalla completa de letra"
          >
            <I.Maximize size={13} />
          </button>
        </Show>
      </div>

      {/* Contenido según pestaña */}
      <Show when={tab() === "queue"}>
        <QueueView />
      </Show>
      <Show when={tab() === "lyrics"}>
        <LyricsView />
      </Show>
    </aside>
  );
}

function QueueView() {
  const [filter, setFilter] = createSignal("all");
  const filters = [
    { id: "all", label: "Todos" },
    { id: "chill", label: "Chill" },
    { id: "popular", label: "Popular" },
    { id: "fresh", label: "Descubrir" },
  ];

  return (
    <div class="flex flex-1 flex-col overflow-hidden">
      {/* Subcabecera y filtros */}
      <div class="px-4 pt-3 pb-2">
        <div class="text-[10px] font-semibold uppercase tracking-widest text-white/40">
          Sonando desde: <span class="text-white/80">Tu cola</span>
        </div>
        <div class="mt-2.5 flex items-center gap-1.5 overflow-x-auto pb-1 scrollbar-none">
          <For each={filters}>
            {(f) => (
              <button
                class="shrink-0 rounded-full px-2.5 py-0.5 text-[11px] font-medium transition-colors border"
                classList={{
                  "bg-white/20 border-white/20 text-white": filter() === f.id,
                  "bg-white/[0.03] border-white/5 text-white/50 hover:text-white/80 hover:bg-white/10":
                    filter() !== f.id,
                }}
                onClick={() => setFilter(f.id)}
              >
                {f.label}
              </button>
            )}
          </For>
        </div>
      </div>

      {/* Lista de pistas */}
      <div class="scroll-area flex-1 px-2.5 py-1">
        <Show
          when={playback.queue.length > 0}
          fallback={<Empty>La cola está vacía.</Empty>}
        >
          <For each={playback.queue}>
            {(t, i) => {
              const isCurrent = () => i() === playback.queueIndex;
              return (
                <button
                  class="group flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left transition-all duration-150 border border-transparent"
                  classList={{
                    "bg-white/10 border-white/10 shadow-sm": isCurrent(),
                    "hover:bg-white/[0.06] hover:border-white/5": !isCurrent(),
                  }}
                  onClick={() => api.jumpTo(i())}
                >
                  {/* Número o ecualizador animado */}
                  <div class="w-5 shrink-0 flex items-center justify-center">
                    <Show
                      when={isCurrent()}
                      fallback={
                        <span class="text-[11px] tabular-nums opacity-35 group-hover:hidden">
                          {i() + 1}
                        </span>
                      }
                    >
                      <EqualizerWave playing={playback.playing} />
                    </Show>
                    <span class="hidden text-[11px] text-[var(--accent)] group-hover:block">
                      ▶
                    </span>
                  </div>

                  {/* Portada miniatura */}
                  <Show when={t.thumbnail}>
                    <img
                      src={thumbAt(t.thumbnail, 80)!}
                      alt=""
                      class="size-9 shrink-0 rounded-lg object-cover ring-1 ring-white/10"
                    />
                  </Show>

                  {/* Título y Autor */}
                  <div class="min-w-0 flex-1">
                    <div
                      class="truncate text-[12.5px] font-medium leading-snug"
                      classList={{ "text-[var(--accent)] font-semibold": isCurrent() }}
                    >
                      {t.title}
                    </div>
                    <div class="truncate text-[11px] opacity-45">{t.author}</div>
                  </div>
                </button>
              );
            }}
          </For>
        </Show>
      </div>
    </div>
  );
}

function LyricsView() {
  const lineRefs = new Map<number, HTMLElement>();

  const activeIndex = () => {
    const lyr = lyrics();
    if (!lyr?.lines?.length) return -1;
    const ms = position();
    let idx = 0;
    for (let i = 0; i < lyr.lines.length; i++) {
      if (ms >= lyr.lines[i].startMs) {
        idx = i;
      } else {
        break;
      }
    }
    return idx;
  };

  createEffect(
    on(activeIndex, (idx) => {
      if (idx < 0) return;
      const el = lineRefs.get(idx);
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    })
  );

  return (
    <div class="scroll-area flex-1 px-5 py-8 overflow-y-auto">
      <Show
        when={!lyricsLoading()}
        fallback={
          <div class="flex h-full items-center justify-center text-xs opacity-50 animate-pulse">
            Cargando letra...
          </div>
        }
      >
        <Show
          when={lyrics()?.lines?.length}
          fallback={
            <Empty>No encontramos letra para esta pista.</Empty>
          }
        >
          <div class="space-y-6 py-20">
            <For each={lyrics()!.lines}>
              {(line, i) => {
                const isActive = () => i() === activeIndex();
                return (
                  <p
                    ref={(el) => lineRefs.set(i(), el)}
                    class="cursor-pointer transition-all duration-300 select-none text-left"
                    classList={{
                      "text-white font-bold text-[19px] scale-[1.02] origin-left [text-shadow:0_0_16px_rgba(255,255,255,0.4)]":
                        isActive(),
                      "text-white/35 font-medium text-[15px] blur-[0.3px] hover:text-white/70 hover:opacity-90":
                        !isActive(),
                    }}
                    onClick={() => api.seek(line.startMs)}
                  >
                    {line.text}
                  </p>
                );
              }}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}

function EqualizerWave(p: { playing: boolean }) {
  return (
    <div class="flex items-end gap-[2px] h-3.5 w-3.5 text-[var(--accent)]">
      <span
        class="w-[2.5px] bg-current rounded-full transition-all duration-300"
        classList={{
          "animate-[pulse_0.8s_ease-in-out_infinite] h-full": p.playing,
          "h-2": !p.playing,
        }}
      />
      <span
        class="w-[2.5px] bg-current rounded-full transition-all duration-300"
        classList={{
          "animate-[pulse_1.1s_ease-in-out_infinite_150ms] h-2/3": p.playing,
          "h-3": !p.playing,
        }}
      />
      <span
        class="w-[2.5px] bg-current rounded-full transition-all duration-300"
        classList={{
          "animate-[pulse_0.9s_ease-in-out_infinite_300ms] h-4/5": p.playing,
          "h-1.5": !p.playing,
        }}
      />
    </div>
  );
}

function Empty(p: { children: any }) {
  return (
    <div class="flex flex-1 items-center justify-center p-8 text-center text-[12.5px] leading-relaxed opacity-40">
      <span>{p.children}</span>
    </div>
  );
}
