import { For, Show } from "solid-js";
import { api } from "../lib/api";
import { playback } from "../lib/store";

/** Panel derecho: solo cola de reproducción (letras removidas para diagnóstico). */
export function SidePanel() {
  return (
    <aside class="panel flex w-[400px] shrink-0 flex-col overflow-hidden">
      <div class="flex shrink-0 items-center justify-between p-3 border-b border-[var(--line)]">
        <span class="text-[12px] font-semibold tracking-wide uppercase opacity-70">
          En cola ({playback.queue.length})
        </span>
      </div>
      <QueueList />
    </aside>
  );
}

function QueueList() {
  return (
    <div class="scroll-area flex-1 px-2 py-2">
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

function Empty(p: { children: any }) {
  return (
    <div class="flex flex-1 items-center justify-center px-6 text-center text-[12.5px] leading-relaxed opacity-35">
      <span>{p.children}</span>
    </div>
  );
}
