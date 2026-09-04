import type { Track } from "../lib/api";
import { setAddingTo } from "../lib/store";
import { MenuKebab, type Accion } from "./MenuKebab";
import * as I from "./Icons";

/**
 * Menú de una canción.
 *
 * Solo lleva acciones que la aplicación sabe hacer de verdad. La referencia
 * muestra veinte entradas, pero la mitad —descargar, denunciar, ver créditos—
 * exigen sesión o funciones que aquí no existen; ponerlas en gris o, peor, que
 * no hagan nada, es lo que convierte un menú en decoración.
 *
 * La mecánica del desplegable vive en [`MenuKebab`]: estaba duplicada con la
 * del menú de playlists y las dos copias se salían de la ventana por su cuenta.
 */
export type AccionMenu = Accion;

export function TrackMenu(p: {
  track: Partial<Track>;
  /** Acciones propias del sitio desde el que se abre (quitar de la cola, etc.). */
  extra?: AccionMenu[];
  class?: string;
}) {
  return (
    <MenuKebab
      titulo="Más opciones"
      class={p.class}
      botonClass="icon-btn size-7 text-white/40 hover:!bg-white/10 hover:text-white/90"
      acciones={() => [
        {
          etiqueta: "Guardar en playlist",
          icono: I.Plus,
          hacer: () => setAddingTo(p.track),
        },
        ...(p.extra ?? []),
      ]}
    />
  );
}
