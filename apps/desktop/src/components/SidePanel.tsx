import { For, Show, createEffect, createMemo, createSignal, on } from "solid-js";
import { api, type Lyrics } from "../lib/api";
import { playback, position } from "../lib/store";

type Tab = "queue" | "lyrics";

/** Panel derecho: cola y letra sincronizada. */
export function SidePanel() {
  const [tab, setTab] = createSignal<Tab>("lyrics");
  const [lyrics, setLyrics] = createSignal<Lyrics | null>(null);
  const [loading, setLoading] = createSignal(false);

  // La letra se pide una vez por pista, no en cada cambio de estado.
  createEffect(
    on(
      () => playback.track?.videoId,
      async () => {
        const t = playback.track;
        if (!t) {
          setLyrics(null);
          return;
        }
        setLoading(true);
        try {
          setLyrics(await api.getLyrics(t.title, t.author, playback.durationMs));
        } catch {
          setLyrics(null);
        } finally {
          setLoading(false);
        }
      },
    ),
  );

  return (
    <aside class="panel flex w-[360px] shrink-0 flex-col overflow-hidden">
      <div class="flex shrink-0 gap-1 p-2">
        <TabButton active={tab() === "queue"} onClick={() => setTab("queue")}>
          En cola
        </TabButton>
        <TabButton active={tab() === "lyrics"} onClick={() => setTab("lyrics")}>
          Letra
        </TabButton>
      </div>

      <Show when={tab() === "queue"}>
        <QueueList />
      </Show>
      <Show when={tab() === "lyrics"}>
        <LyricsView lyrics={lyrics()} loading={loading()} />
      </Show>
    </aside>
  );
}

function TabButton(p: { active: boolean; onClick: () => void; children: string }) {
  return (
    <button
      class="flex-1 rounded-lg px-3 py-2 text-[11px] font-semibold uppercase tracking-wider transition-colors"
      classList={{
        "bg-[var(--panel-strong)]": p.active,
        "opacity-45 hover:opacity-75": !p.active,
      }}
      onClick={p.onClick}
    >
      {p.children}
    </button>
  );
}

function QueueList() {
  return (
    <div class="scroll-area flex-1 px-2 pb-2">
      <Show
        when={playback.queue.length > 0}
        fallback={<Empty>La cola está vacía.</Empty>}
      >
        <For each={playback.queue}>
          {(t, i) => (
            <button
              class="flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left transition-colors hover:bg-[var(--panel)]"
              classList={{ "bg-[var(--panel)]": i() === playback.queueIndex }}
              onClick={() => api.jumpTo(i())}
            >
              <span
                class="w-5 shrink-0 text-center text-[10px] tabular-nums"
                classList={{
                  "opacity-30": i() !== playback.queueIndex,
                  "text-[var(--accent)]": i() === playback.queueIndex,
                }}
              >
                {i() === playback.queueIndex ? "▶" : i() + 1}
              </span>
              <span class="min-w-0 flex-1">
                <span class="block truncate text-[12.5px]">{t.title}</span>
                <span class="block truncate text-[11px] opacity-45">{t.author}</span>
              </span>
            </button>
          )}
        </For>
      </Show>
    </div>
  );
}

function LyricsView(p: { lyrics: Lyrics | null; loading: boolean }) {
  let container!: HTMLDivElement;

  /** Indice de la linea que suena ahora. */
  const activeIndex = createMemo(() => {
    const lines = p.lyrics?.lines;
    if (!lines?.length) return -1;
    const t = position();
    // Busqueda binaria: la letra puede tener cientos de lineas y esto corre en
    // cada fotograma.
    let lo = 0;
    let hi = lines.length - 1;
    let found = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (lines[mid].startMs <= t) {
        found = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    return found;
  });

  /**
   * Progreso dentro de la linea actual, de 0 a 1.
   *
   * LRCLIB da sincronia POR LINEA, no por palabra. Interpolando la duracion de
   * la linea se consigue el mismo barrido de texto que se ve en las apps
   * bonitas, sin depender de datos por palabra que casi no existen.
   */
  const lineProgress = createMemo(() => {
    const lines = p.lyrics?.lines;
    const i = activeIndex();
    if (!lines || i < 0) return 0;
    const { startMs, endMs } = lines[i];
    const span = Math.max(1, endMs - startMs);
    return Math.min(1, Math.max(0, (position() - startMs) / span));
  });

  // Desplaza para mantener centrada la linea activa.
  createEffect(
    on(activeIndex, (i) => {
      if (i < 0 || !container) return;
      const el = container.querySelector<HTMLElement>(`[data-line="${i}"]`);
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
    }),
  );

  return (
    <Show
      when={!p.loading}
      fallback={<Empty>Buscando letra…</Empty>}
    >
      <Show
        when={p.lyrics}
        fallback={
          <Empty>
            No hay letra para esta canción.
            <br />
            <span class="opacity-60">La cobertura de LRCLIB es irregular.</span>
          </Empty>
        }
      >
        <Show
          when={p.lyrics!.synced}
          fallback={
            <div class="scroll-area flex-1 whitespace-pre-wrap px-5 pb-6 text-[14px] leading-relaxed opacity-70">
              {p.lyrics!.plain}
            </div>
          }
        >
          <div ref={container} class="scroll-area flex-1 px-6 py-[42%]">
            <For each={p.lyrics!.lines}>
              {(line, i) => {
                const active = () => i() === activeIndex();
                const past = () => i() < activeIndex();
                return (
                  <p
                    data-line={i()}
                    class="cursor-pointer py-2.5 pr-2 text-[18px] font-bold leading-snug tracking-tight transition-all duration-500"
                    classList={{
                      "opacity-100 scale-100": active(),
                      "opacity-25 scale-[0.97]": !active(),
                      "opacity-15": past(),
                    }}
                    style={
                      active()
                        ? {
                            // El degradado con `background-clip: text` produce el
                            // barrido. Solo cambia un porcentaje, asi que el
                            // navegador no rehace el layout.
                            background: `linear-gradient(90deg, var(--fg) ${
                              lineProgress() * 100
                            }%, rgb(255 255 255 / 0.3) ${lineProgress() * 100}%)`,
                            "-webkit-background-clip": "text",
                            "background-clip": "text",
                            color: "transparent",
                          }
                        : undefined
                    }
                    onClick={() => api.seek(line.startMs)}
                  >
                    {line.text || " "}
                  </p>
                );
              }}
            </For>
          </div>
        </Show>
      </Show>
    </Show>
  );
}

function Empty(p: { children: any }) {
  return (
    <div class="flex flex-1 items-center justify-center px-6 text-center text-[12.5px] leading-relaxed opacity-35">
      <span>{p.children}</span>
    </div>
  );
}
