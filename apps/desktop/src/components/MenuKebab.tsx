import { Show, createSignal, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import * as I from "./Icons";

/**
 * El menú de los tres puntos, uno solo para toda la aplicación.
 *
 * Lo usan el de una canción y el de una playlist. Estaban duplicados y cada
 * copia tuvo el mismo fallo: el menú se abría hacia abajo y se salía de la
 * ventana. En la barra lateral porque la lista desplaza y recorta; en la barra
 * de reproducción porque debajo no hay nada — la barra ES el borde inferior.
 *
 * De ahí las dos decisiones que no son evidentes:
 *
 *   - Se pinta en un `Portal` con posición fija. Ni `absolute` ni `fixed` a
 *     secas bastan: cualquier ancestro con `backdrop-filter` —y aquí hay
 *     varios, es una interfaz de cristal— se convierte en su bloque contenedor
 *     y vuelve a recortarlo.
 *   - Se coloca debajo del botón si cabe y encima si no, y siempre se encierra
 *     dentro de la ventana. La altura se estima para el primer pintado y se
 *     corrige en cuanto el menú existe y se puede medir de verdad.
 */

export type Accion = {
  etiqueta: string;
  icono: (p: { size?: number; class?: string }) => any;
  hacer: () => void;
};

const ANCHO = 224; // w-56

/** Alto de una fila, para estimar antes de poder medir. `py-2.5` más el texto. */
const FILA = 42;

const MARGEN = 8;

export function MenuKebab(p: {
  acciones: () => Accion[];
  /** Texto del botón. "Más opciones" para una canción, y demás. */
  titulo: string;
  class?: string;
  /** Clases del botón, para los sitios que lo quieren más apagado. */
  botonClass?: string;
}) {
  const [abierto, setAbierto] = createSignal(false);
  const [sitio, setSitio] = createSignal({ top: 0, left: 0 });
  let raiz!: HTMLDivElement;
  let boton!: HTMLButtonElement;
  // El menú vive en un portal, fuera de `raiz`: sin comprobarlo aparte, pulsar
  // dentro del propio menú contaría como pulsar fuera y lo cerraría.
  let flotante: HTMLDivElement | undefined;

  onMount(() => {
    const fuera = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!raiz.contains(t) && !flotante?.contains(t)) setAbierto(false);
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape") setAbierto(false);
    };
    // Al desplazar o redimensionar, el menú se quedaría en un sitio que ya no
    // corresponde al botón.
    const cerrar = () => setAbierto(false);
    window.addEventListener("mousedown", fuera);
    window.addEventListener("keydown", escape);
    window.addEventListener("scroll", cerrar, true);
    window.addEventListener("resize", cerrar);
    onCleanup(() => {
      window.removeEventListener("mousedown", fuera);
      window.removeEventListener("keydown", escape);
      window.removeEventListener("scroll", cerrar, true);
      window.removeEventListener("resize", cerrar);
    });
  });

  /** Debajo del botón si cabe, encima si no, y nunca fuera de la ventana. */
  const colocar = (alto: number) => {
    const r = boton.getBoundingClientRect();
    const left = Math.max(
      MARGEN,
      Math.min(r.right - ANCHO, window.innerWidth - ANCHO - MARGEN),
    );
    const debajo = r.bottom + 4;
    const encima = r.top - alto - 4;
    const top = debajo + alto > window.innerHeight - MARGEN ? encima : debajo;
    setSitio({
      left,
      top: Math.max(MARGEN, Math.min(top, window.innerHeight - alto - MARGEN)),
    });
  };

  const alternar = () => {
    if (abierto()) return setAbierto(false);
    colocar(p.acciones().length * FILA + 8);
    setAbierto(true);
  };

  return (
    <div ref={raiz} class={p.class}>
      <button
        ref={boton}
        class={p.botonClass ?? "icon-btn size-7 text-white/45 hover:!bg-white/10 hover:text-white"}
        onClick={(e) => {
          e.stopPropagation();
          alternar();
        }}
        title={p.titulo}
      >
        <I.More size={15} />
      </button>

      <Show when={abierto()}>
        <Portal>
          <div
            ref={(el) => {
              flotante = el;
              // Ya existe: se sustituye la estimación por la altura real antes
              // de que se pinte. Con dos acciones sobra, pero el menú de una
              // canción crece con las opciones del sitio desde el que se abre.
              queueMicrotask(() => el.isConnected && colocar(el.offsetHeight));
            }}
            class="glass-card fixed z-50 w-56 overflow-hidden rounded-xl py-1 shadow-2xl"
            style={{ top: `${sitio().top}px`, left: `${sitio().left}px` }}
            onClick={(e) => e.stopPropagation()}
          >
            {p.acciones().map((a) => (
              <button
                class="flex w-full items-center gap-3 px-3.5 py-2.5 text-left text-[13px] font-medium text-white/85 transition-colors hover:bg-white/[0.09]"
                onClick={(e) => {
                  e.stopPropagation();
                  setAbierto(false);
                  a.hacer();
                }}
              >
                <a.icono size={15} class="shrink-0 text-white/50" />
                {a.etiqueta}
              </button>
            ))}
          </div>
        </Portal>
      </Show>
    </div>
  );
}
