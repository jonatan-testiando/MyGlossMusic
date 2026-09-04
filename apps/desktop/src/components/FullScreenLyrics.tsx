import { For, Show, createEffect, on } from "solid-js";
import { api, thumbAt } from "../lib/api";
import { lyrics, lyricsLoading, position, setFullLyricsOpen, trackVisible } from "../lib/store";
import * as I from "./Icons";

export function FullScreenLyrics() {
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
    <div class="fixed inset-0 z-40 flex flex-col bg-[var(--bg)]/90 backdrop-blur-3xl animate-in fade-in duration-300">
      {/* Barra superior de cierre */}
      <div class="flex h-12 items-center justify-between px-6 pt-3">
        <span class="text-xs font-semibold uppercase tracking-widest opacity-50">Letras en vivo</span>
        <button
          class="icon-btn size-9 rounded-full bg-white/10 hover:bg-white/20"
          onClick={() => setFullLyricsOpen(false)}
          title="Cerrar letras"
        >
          <I.Close size={18} />
        </button>
      </div>

      {/* Contenido principal */}
      <div class="flex flex-1 min-h-0 items-center justify-center gap-12 px-12 pb-24">
        {/* Lado izquierdo: portada y metadatos */}
        <div class="flex w-[340px] shrink-0 flex-col items-center text-center">
          <Show
            when={trackVisible()?.thumbnail}
            fallback={<div class="size-72 rounded-2xl bg-white/10 shadow-2xl" />}
          >
            <img
              src={thumbAt(trackVisible()!.thumbnail, 540)!}
              alt=""
              class="size-72 rounded-2xl object-cover shadow-[0_20px_60px_-15px_rgba(0,0,0,0.7)] ring-1 ring-white/10"
            />
          </Show>
          <h2 class="mt-6 text-xl font-bold tracking-tight line-clamp-1">{trackVisible()?.title ?? "Sin título"}</h2>
          <p class="mt-1 text-sm opacity-60 line-clamp-1">{trackVisible()?.author ?? "Desconocido"}</p>
        </div>

        {/* Lado derecho: flujo de letras grandes */}
        <div class="scroll-area flex-1 h-[70vh] px-8 py-20 flex flex-col items-start overflow-y-auto">
          <Show
            when={!lyricsLoading()}
            fallback={
              <div class="m-auto text-sm opacity-50 animate-pulse">Buscando letra sincronizada...</div>
            }
          >
            <Show
              when={lyrics()?.lines?.length}
              fallback={
                <div class="m-auto text-center opacity-40">
                  <p class="text-lg">No encontramos letra para esta canción</p>
                </div>
              }
            >
              <div class="my-auto w-full space-y-7 py-32">
                <For each={lyrics()!.lines}>
                  {(line, i) => {
                    const isActive = () => i() === activeIndex();
                    return (
                      <p
                        ref={(el) => lineRefs.set(i(), el)}
                        class="cursor-pointer transition-all duration-300 select-none text-left"
                        classList={{
                          "text-white font-extrabold text-3xl md:text-4xl scale-105 origin-left [text-shadow:0_0_24px_rgba(255,255,255,0.45)]":
                            isActive(),
                          "text-white/35 font-semibold text-2xl hover:text-white/70 hover:opacity-80 blur-[0.4px]":
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
      </div>
    </div>
  );
}
