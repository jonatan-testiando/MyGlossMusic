import { For, Show, createEffect, createSignal, on } from "solid-js";
import { api, fmtTime, thumbAt, type BrowsePage, type Track } from "../lib/api";
import {
  playback,
  lyrics,
  lyricsLoading,
  position,
  setFullLyricsOpen,
  relatedArtistId,
  openBrowse,
} from "../lib/store";
import * as I from "./Icons";

/** Panel derecho flotante de cristal escarchado idéntico a Image 2. */
type TabId = "queue" | "lyrics" | "comments" | "similar";

export function SidePanel() {
  const [tab, setTab] = createSignal<TabId>("queue");

  return (
    <aside class="glass-card flex h-full min-w-[340px] max-w-[920px] flex-1 flex-col overflow-hidden rounded-3xl border border-white/10 shadow-[0_25px_60px_rgba(0,0,0,0.5)] select-none">
      {/* Cabecera con pestañas estilo Image 2 */}
      <div class="glass-block m-3 flex shrink-0 items-center justify-between gap-2 px-3 py-2">
        <div class="flex shrink-0 items-center gap-1">
          <Tab id="queue" active={tab()} onPick={setTab}>A continuación</Tab>
          <Tab id="lyrics" active={tab()} onPick={setTab}>Letra</Tab>
          <Tab id="comments" active={tab()} onPick={setTab}>Comentarios</Tab>
          <Tab id="similar" active={tab()} onPick={setTab}>Similares</Tab>
        </div>

        <Show when={tab() === "lyrics"}>
          <div class="flex min-w-0 items-center gap-1.5">
            <Show when={lyrics()?.source}>
              {/* De donde sale la letra, y si viene sincronizada. Antes era un
                  texto fijo que mentia cuando cambiara el proveedor. */}
              <div
                class="flex min-w-0 items-center gap-1.5 rounded-full border border-white/10 bg-white/[0.06] px-2 py-1 text-[10.5px] font-medium text-white/60"
                title={lyrics()!.synced ? "Letra sincronizada" : "Letra sin sincronizar"}
              >
                <span
                  class="size-1.5 shrink-0 rounded-full"
                  classList={{
                    "bg-[var(--accent)]": lyrics()!.synced,
                    "bg-white/40": !lyrics()!.synced,
                  }}
                />
                <span class="truncate">{lyrics()!.source}</span>
              </div>
            </Show>
            <button
              class="icon-btn size-7 shrink-0 opacity-70 hover:opacity-100 hover:bg-white/10"
              onClick={() => setFullLyricsOpen(true)}
              title="Pantalla completa"
            >
              <I.Maximize size={14} />
            </button>
          </div>
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
        <SimilarView />
      </Show>
    </aside>
  );
}

/**
 * Duracion que se pinta en una fila de la cola.
 *
 * La pista trae la suya desde la busqueda; la actual, ademas, tiene la que midio
 * el motor al resolverla, que es la buena. Cuando no hay ninguna se deja un
 * guion: inventarse un numero es peor que admitir que aun no se sabe.
 */
function trackDuration(t: Track, isCurrent: boolean): string {
  if (isCurrent && playback.durationMs) return fmtTime(playback.durationMs);
  if (t.durationMs) return fmtTime(t.durationMs);
  return "--:--";
}

function Tab(p: {
  id: TabId;
  active: TabId;
  onPick: (id: TabId) => void;
  children: string;
}) {
  return (
    <button
      class="shrink-0 rounded-lg px-2.5 py-1 text-[11px] font-bold uppercase transition-all duration-150"
      classList={{
        "bg-white/20 text-white shadow-sm": p.active === p.id,
        "text-white/50 hover:text-white/80": p.active !== p.id,
      }}
      onClick={() => p.onPick(p.id)}
    >
      {p.children}
    </button>
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
      <div class="px-3 pb-2">
        <div class="glass-block flex items-center justify-between px-4 py-3">
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
                      {trackDuration(t, isCurrent())}
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

/**
 * Pestaña SIMILARES: la página del artista de lo que suena.
 *
 * No es la pestaña "Relacionado" de YouTube Music. Esa cuelga de un `browseId`
 * con prefijo `MPTR` que solo responde dentro del contexto de sesión de `next`
 * — pedido suelto devuelve una respuesta vacía de 2 KB, comprobado. Ofrecer la
 * página del artista es real y útil; dejar la pestaña con un texto que promete
 * algo que no llega, no.
 */
function SimilarView() {
  const [pagina, setPagina] = createSignal<BrowsePage | null>(null);
  const [cargando, setCargando] = createSignal(false);

  createEffect(
    on(relatedArtistId, async (id) => {
      setPagina(null);
      if (!id) return;
      setCargando(true);
      try {
        setPagina(await api.browse(id));
      } catch {
        setPagina(null);
      } finally {
        setCargando(false);
      }
    }),
  );

  return (
    <div class="scroll-area flex-1 overflow-y-auto px-4 py-3">
      <Show when={cargando()}>
        <p class="py-10 text-center text-sm text-white/40">Cargando…</p>
      </Show>

      <Show
        when={!cargando() && pagina()}
        fallback={
          <Show when={!cargando()}>
            <Empty>No hay nada relacionado para esta pista.</Empty>
          </Show>
        }
      >
        <div class="mb-3 px-1">
          <div class="text-[10px] font-bold uppercase tracking-widest text-white/45">
            Más de
          </div>
          <button
            class="truncate text-left text-[17px] font-bold leading-tight text-white hover:underline"
            onClick={() => relatedArtistId() && openBrowse(relatedArtistId()!)}
          >
            {pagina()!.title ?? "este artista"}
          </button>
        </div>

        <For each={pagina()!.shelves}>
          {(estante) => (
            <section class="mb-4">
              <Show when={estante.title}>
                <h3 class="mb-1.5 px-1 text-[11px] font-bold uppercase tracking-wider text-white/45">
                  {estante.title}
                </h3>
              </Show>
              <For each={estante.items.slice(0, 6)}>
                {(item) => (
                  <button
                    class="group flex w-full items-center gap-3 rounded-xl px-2 py-2 text-left transition-colors hover:bg-white/[0.07]"
                    onClick={() =>
                      item.kind === "track" ? api.playNow(item.id) : openBrowse(item.id)
                    }
                  >
                    <Show
                      when={item.thumbnail}
                      fallback={<div class="size-10 shrink-0 rounded-lg bg-white/10" />}
                    >
                      <img
                        src={thumbAt(item.thumbnail, 80)!}
                        alt=""
                        class="size-10 shrink-0 object-cover ring-1 ring-white/10"
                        classList={{
                          "rounded-full": item.kind === "artist",
                          "rounded-lg": item.kind !== "artist",
                        }}
                      />
                    </Show>
                    <div class="min-w-0 flex-1">
                      <div class="truncate text-[13px] font-semibold text-white">{item.title}</div>
                      <div class="truncate text-[11.5px] text-white/55">{item.subtitle}</div>
                    </div>
                    <span class="shrink-0 text-[11.5px] tabular-nums text-white/45">
                      {item.duration ?? ""}
                    </span>
                  </button>
                )}
              </For>
            </section>
          )}
        </For>
      </Show>
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
