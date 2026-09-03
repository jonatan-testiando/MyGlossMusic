import { Show, createSignal, onCleanup, onMount } from "solid-js";
import type { Track } from "../lib/api";
import { setAddingTo } from "../lib/store";
import * as I from "./Icons";

/**
 * Menú de una canción.
 *
 * Solo lleva acciones que la aplicación sabe hacer de verdad. La referencia
 * muestra veinte entradas, pero la mitad —descargar, denunciar, ver créditos—
 * exigen sesión o funciones que aquí no existen; ponerlas en gris o, peor, que
 * no hagan nada, es lo que convierte un menú en decoración.
 */
export type AccionMenu = {
  etiqueta: string;
  icono: (p: { size?: number; class?: string }) => any;
  hacer: () => void;
};

export function TrackMenu(p: {
  track: Partial<Track>;
  /** Acciones propias del sitio desde el que se abre (quitar de la cola, etc.). */
  extra?: AccionMenu[];
  class?: string;
}) {
  const [abierto, setAbierto] = createSignal(false);
  let raiz!: HTMLDivElement;

  onMount(() => {
    // Se cierra al pulsar fuera o con Escape. `mousedown` y no `click` para que
    // el menú desaparezca al empezar el gesto, no al soltarlo.
    const fuera = (e: MouseEvent) => {
      if (!raiz.contains(e.target as Node)) setAbierto(false);
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape") setAbierto(false);
    };
    window.addEventListener("mousedown", fuera);
    window.addEventListener("keydown", escape);
    onCleanup(() => {
      window.removeEventListener("mousedown", fuera);
      window.removeEventListener("keydown", escape);
    });
  });

  const acciones = (): AccionMenu[] => [
    {
      etiqueta: "Guardar en playlist",
      icono: I.Plus,
      hacer: () => setAddingTo(p.track),
    },
    ...(p.extra ?? []),
  ];

  return (
    <div ref={raiz} class={`relative ${p.class ?? ""}`}>
      <button
        class="icon-btn size-7 text-white/40 hover:!bg-white/10 hover:text-white/90"
        onClick={(e) => {
          e.stopPropagation();
          setAbierto((v) => !v);
        }}
        title="Más opciones"
      >
        <I.More size={15} />
      </button>

      <Show when={abierto()}>
        <div
          class="glass-card absolute right-0 z-50 mt-1 w-56 overflow-hidden rounded-xl py-1 shadow-2xl"
          onClick={(e) => e.stopPropagation()}
        >
          {acciones().map((a) => (
            <button
              class="flex w-full items-center gap-3 px-3.5 py-2.5 text-left text-[13px] font-medium text-white/85 transition-colors hover:bg-white/[0.09]"
              onClick={() => {
                setAbierto(false);
                a.hacer();
              }}
            >
              <a.icono size={15} class="shrink-0 text-white/50" />
              {a.etiqueta}
            </button>
          ))}
        </div>
      </Show>
    </div>
  );
}
