import { For } from "solid-js";
import { avisos, descartar } from "../lib/toast";

/**
 * Los avisos, apilados sobre la barra de reproducción.
 *
 * Abajo a la izquierda y no en el centro: el centro de la barra es donde está
 * la canción, y el centro de la ventana es donde está la carátula. La esquina
 * es el único sitio que no tapa nada de lo que se está mirando.
 */
export function Toasts() {
  return (
    <div class="pointer-events-none fixed bottom-[88px] left-5 z-[60] flex flex-col gap-2">
      <For each={avisos()}>
        {(a) => (
          <div
            class="aviso-entra glass-card pointer-events-auto flex max-w-sm items-start gap-3 rounded-xl px-4 py-3 shadow-2xl"
            classList={{ "ring-1 ring-red-400/40": a.tipo === "error" }}
            role="status"
            onClick={() => descartar(a.id)}
            title="Descartar"
          >
            <span
              class="mt-1.5 size-1.5 shrink-0 rounded-full"
              style={{ background: a.tipo === "error" ? "#f87171" : "var(--accent)" }}
            />
            <span class="text-[13px] font-medium leading-snug text-white/90">{a.texto}</span>
          </div>
        )}
      </For>
    </div>
  );
}
