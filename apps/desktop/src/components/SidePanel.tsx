import { For, Show, createEffect, createSignal, on } from "solid-js";
import { api, fmtTime, thumbAt } from "../lib/api";
import { playback, lyrics, lyricsLoading, position, setFullLyricsOpen } from "../lib/store";
import * as I from "./Icons";

/** Panel derecho flotante de cristal escarchado idéntico a Image 2. */
export function SidePanel() {
  const [tab, setTab] = createSignal<"queue" | "lyrics" | "comments" | "similar">("queue");

  return (
    <aside class="glass-card flex w-[500px] max-w-[540px] h-full flex-col overflow-hidden shadow-[0_25px_60px_rgba(0,0,0,0.65)] rounded-3xl border border-white/10 select-none">
      {/* Cabecera con pestañas estilo Image 2 */}
      <div class="flex shrink-0 items-center justify-between border-b border-white/10 px-5 py-3.5 bg-black/25">
        <div class="flex items-center gap-2">
          <button
            class="px-3 py-1 text-[12px] font-bold tracking-wider uppercase transition-all duration-150 rounded-lg"
            classList={{
              "bg-white/20 text-white shadow-sm font-extrabold": tab() === "queue",
              "text-white/50 hover:text-white/80": tab() !== "queue",
            }}
            onClick={() => setTab("queue")}
          >
            A continuación
          </button>
          <button
            class="px-3 py-1 text-[12px] font-bold tracking-wider uppercase transition-all duration-150 rounded-lg"
            classList={{
              "bg-white/20 text-white shadow-sm font-extrabold": tab() === "lyrics",
              "text-white/50 hover:text-white/80": tab() !== "lyrics",
            }}
            onClick={() => setTab("lyrics")}
          >
            Letra
          </button>
          <button
            class="px-3 py-1 text-[12px] font-bold tracking-wider uppercase transition-all duration-150 rounded-lg"
            classList={{
              "bg-white/20 text-white shadow-sm font-extrabold": tab() === "comments",
              "text-white/50 hover:text-white/80": tab() !== "comments",
            }}
            onClick={() => setTab("comments")}
          >
            Comentarios
          </button>
          <button
            class="px-3 py-1 text-[12px] font-bold tracking-wider uppercase transition-all duration-150 rounded-lg"
            classList={{
              "bg-white/20 text-white shadow-sm font-extrabold": tab() === "similar",
              "text-white/50 hover:text-white/80": tab() !== "similar",
            }}
            onClick={() => setTab("similar")}
          >
            Similares
          </button>
        </div>

        <Show when={tab() === "lyrics"}>
          <button
            class="icon-btn size-7 opacity-70 hover:opacity-100 hover:bg-white/10"
            onClick={() => setFullLyricsOpen(true)}
            title="Pantalla completa"
          >
            <I.Maximize size={14} />
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
      <Show when={tab() === "comments"}>
        <div class="flex flex-1 items-center justify-center p-8 text-center text-sm text-white/40">
          Los comentarios no están disponibles en modo anónimo.
        </div>
      </Show>
      <Show when={tab() === "similar"}>
        <div class="flex flex-1 items-center justify-center p-8 text-center text-sm text-white/40">
          Canciones y recomendaciones similares a esta pista.
        </div>
      </Show>
    </aside>
  );
}

function QueueView() {
  const [filter, setFilter] = createSignal("all");
  const filters = [
    { id: "all", label: "Todo" },
    { id: "discover", label: "Canciones por descubrir" },
    { id: "energize", label: "Energizante" },
    { id: "workout", label: "Entrenamiento" },
    { id: "electronic", label: "Electrónica" },
    { id: "jpop", label: "J-pop" },
  ];

  return (
    <div class="flex flex-1 flex-col overflow-hidden">
      {/* Subcabecera: REPRODUCIENDO DESDE + Mix de... + Guardar */}
      <div class="px-5 pt-4 pb-2">
        <div class="flex items-center justify-between">
          <div class="min-w-0 pr-2">
            <span class="text-[10px] font-bold uppercase tracking-widest text-white/45">
              REPRODUCIENDO DESDE
            </span>
            <div class="text-[16px] font-bold text-white tracking-tight leading-tight truncate">
              {playback.track ? `Mix de ${playback.track.title}` : "Tu cola de reproducción"}
            </div>
          </div>
          <button class="flex shrink-0 items-center gap-1.5 rounded-full bg-white/10 hover:bg-white/15 px-4 py-1.5 text-xs font-semibold text-white border border-white/10 transition-all shadow-sm">
            <I.Plus size={13} />
            Guardar
          </button>
        </div>

        {/* Píldoras de filtro horizontales (Image 2) */}
        <div class="mt-3 flex items-center gap-2 overflow-x-auto pb-1 scrollbar-none">
          <For each={filters}>
            {(f) => (
              <button
                class="shrink-0 rounded-lg px-3.5 py-1 text-xs font-semibold transition-all border"
                classList={{
                  "bg-white/20 border-white/30 text-white shadow-sm": filter() === f.id,
                  "bg-white/[0.04] border-white/5 text-white/55 hover:text-white hover:bg-white/10":
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

      {/* Lista de pistas de la cola */}
      <div class="scroll-area flex-1 px-3 py-2 space-y-1 overflow-y-auto">
        <Show
          when={playback.queue.length > 0}
          fallback={<Empty>La cola de reproducción está vacía.</Empty>}
        >
          <For each={playback.queue}>
            {(t, i) => {
              const isCurrent = () => i() === playback.queueIndex;
              return (
                <div
                  class="group flex w-full items-center gap-3.5 rounded-xl px-3 py-2.5 text-left transition-all duration-150 border cursor-pointer select-none"
                  classList={{
                    "bg-white/[0.12] border-white/15 shadow-sm": isCurrent(),
                    "border-transparent hover:bg-white/[0.06] hover:border-white/5": !isCurrent(),
                  }}
                  onClick={() => api.jumpTo(i())}
                >
                  {/* Portada miniatura */}
                  <div class="relative size-11 shrink-0 rounded-lg overflow-hidden ring-1 ring-white/15 shadow-sm">
                    <Show
                      when={t.thumbnail}
                      fallback={<div class="size-full bg-white/10" />}
                    >
                      <img
                        src={thumbAt(t.thumbnail, 96)!}
                        alt=""
                        class="size-full object-cover"
                      />
                    </Show>

                    {/* Icono de sonido animado si es la actual */}
                    <Show when={isCurrent()}>
                      <div class="absolute inset-0 bg-black/40 flex items-center justify-center">
                        <EqualizerWave playing={playback.playing} />
                      </div>
                    </Show>
                  </div>

                  {/* Título y Autor */}
                  <div class="min-w-0 flex-1">
                    <div
                      class="truncate text-[13.5px] font-bold leading-tight"
                      classList={{
                        "text-white": isCurrent(),
                        "text-white/90 group-hover:text-white": !isCurrent(),
                      }}
                    >
                      {t.title}
                    </div>
                    <div class="truncate text-[12px] text-white/55 font-medium mt-0.5">
                      {t.author}
                    </div>
                  </div>

                  {/* Duración */}
                  <div class="flex items-center gap-2">
                    <span class="text-xs tabular-nums font-medium text-white/60">
                      {isCurrent() && playback.durationMs ? fmtTime(playback.durationMs) : "3:21"}
                    </span>
                    <button
                      class="icon-btn size-7 text-white/40 group-hover:text-white/80 hover:!bg-white/10"
                      onClick={(e) => {
                        e.stopPropagation();
                      }}
                      title="Más opciones"
                    >
                      <I.More size={15} />
                    </button>
                  </div>
                </div>
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
    <div class="scroll-area flex-1 px-6 py-8 overflow-y-auto">
      <Show
        when={!lyricsLoading()}
        fallback={
          <div class="flex h-full items-center justify-center text-sm text-white/50 animate-pulse">
            Cargando letra...
          </div>
        }
      >
        <Show
          when={lyrics()?.lines?.length}
          fallback={
            <Empty>No hay letra disponible para esta canción.</Empty>
          }
        >
          <div class="space-y-6 py-12">
            <For each={lyrics()!.lines}>
              {(line, i) => {
                const isActive = () => i() === activeIndex();
                return (
                  <p
                    ref={(el) => lineRefs.set(i(), el)}
                    class="cursor-pointer transition-all duration-300 select-none text-left"
                    classList={{
                      "text-white font-extrabold text-[20px] scale-[1.02] origin-left [text-shadow:0_0_20px_rgba(255,255,255,0.45)]":
                        isActive(),
                      "text-white/35 font-medium text-[16px] hover:text-white/70 hover:opacity-90":
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
    <div class="flex items-end gap-[2px] h-3.5 w-3.5 text-white">
      <span
        class="w-[2px] bg-current rounded-full transition-all duration-300"
        classList={{
          "animate-[pulse_0.8s_ease-in-out_infinite] h-full": p.playing,
          "h-1.5": !p.playing,
        }}
      />
      <span
        class="w-[2px] bg-current rounded-full transition-all duration-300"
        classList={{
          "animate-[pulse_1.1s_ease-in-out_infinite_150ms] h-2/3": p.playing,
          "h-2.5": !p.playing,
        }}
      />
      <span
        class="w-[2px] bg-current rounded-full transition-all duration-300"
        classList={{
          "animate-[pulse_0.9s_ease-in-out_infinite_300ms] h-4/5": p.playing,
          "h-1": !p.playing,
        }}
      />
    </div>
  );
}

function Empty(p: { children: any }) {
  return (
    <div class="flex flex-1 items-center justify-center p-8 text-center text-sm font-medium text-white/40">
      <span>{p.children}</span>
    </div>
  );
}
