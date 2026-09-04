import { api, type Track } from "../lib/api";
import { comenzarMix, openBrowse, setAddingTo } from "../lib/store";
import { avisar, avisarError } from "../lib/toast";
import { MenuKebab, type Accion } from "./MenuKebab";
import * as I from "./Icons";

/**
 * Menú de una canción.
 *
 * Solo lleva acciones que la aplicación sabe hacer de verdad. YouTube Music
 * muestra nueve; de esas, tres necesitan cuenta —fijar en "Volver a escuchar",
 * "No me interesa", quitar de las que te gustaron sin ser las nuestras— y
 * "Descargar" es de Premium. Ponerlas en gris o, peor, que no hagan nada, es lo
 * que convierte un menú en decoración.
 *
 * "Ir al artista" y "Ir al álbum" aparecen solo cuando la fila de la que sale
 * la canción traía esos enlaces. No siempre los trae, y no se inventan.
 *
 * La mecánica del desplegable vive en [`MenuKebab`].
 */
export type AccionMenu = Accion;

export function TrackMenu(p: {
  track: Partial<Track>;
  /** Canal del artista, si se conoce. Sale de la fila, no de la pista. */
  artistId?: string | null;
  albumId?: string | null;
  /** Acciones propias del sitio desde el que se abre (quitar de la cola, etc.). */
  extra?: AccionMenu[];
  class?: string;
}) {
  const copiarEnlace = async () => {
    const url = `https://music.youtube.com/watch?v=${p.track.videoId}`;
    try {
      await navigator.clipboard.writeText(url);
      avisar("Enlace copiado al portapapeles");
    } catch {
      // El portapapeles puede estar bloqueado; sin aviso parecería que no pasó
      // nada y el usuario lo intentaría otra vez.
      avisarError("No se pudo copiar el enlace.");
    }
  };

  const acciones = (): AccionMenu[] => {
    const t = p.track;
    const lista: AccionMenu[] = [
      {
        etiqueta: "Comenzar mix",
        icono: I.Radio,
        hacer: () => comenzarMix(t),
      },
      {
        etiqueta: "Reproducir a continuación",
        icono: I.Next,
        hacer: () => {
          api.playNext(t);
          avisar("Sonará a continuación");
        },
      },
      {
        etiqueta: "Añadir a la cola",
        icono: I.QueueAdd,
        hacer: () => {
          api.enqueue(t);
          avisar("Añadida al final de la cola");
        },
      },
      { etiqueta: "Guardar en playlist", icono: I.Plus, hacer: () => setAddingTo(t) },
    ];

    if (p.artistId) {
      lista.push({
        etiqueta: "Ir al artista",
        icono: I.Music,
        hacer: () => openBrowse(p.artistId!, t.author ?? undefined),
      });
    }
    if (p.albumId) {
      lista.push({
        etiqueta: "Ir al álbum",
        icono: I.Library,
        hacer: () => openBrowse(p.albumId!, t.title ?? undefined),
      });
    }

    lista.push({ etiqueta: "Copiar enlace", icono: I.Share, hacer: copiarEnlace });
    return [...lista, ...(p.extra ?? [])];
  };

  return (
    <MenuKebab
      titulo="Más opciones"
      class={p.class}
      botonClass="icon-btn size-7 text-white/40 hover:!bg-white/10 hover:text-white/90"
      acciones={acciones}
    />
  );
}
