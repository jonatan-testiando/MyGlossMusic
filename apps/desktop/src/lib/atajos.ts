import { api } from "./api";
import { playback, position } from "./store";
import { avisar } from "./toast";

/**
 * Atajos de teclado.
 *
 * # Cuándo NO actúan
 *
 * Cuando estás escribiendo. Suena obvio, pero es el fallo clásico de esto:
 * teclear "space" en el buscador pausaría la música. Se comprueba el elemento
 * con el foco, no solo el buscador, porque también hay campos en los diálogos
 * de playlist. Tampoco actúan con Ctrl/Alt/Meta pulsados: eso pertenece al
 * sistema o al navegador, no a nosotros.
 *
 * # Por qué estos y no otros
 *
 * Los de YouTube, porque son los que la gente ya tiene en los dedos: `k` para
 * pausar, `j`/`l` para diez segundos, `m` para silenciar, `/` para buscar. Las
 * flechas van a cinco segundos y al 5 % de volumen, que es la resolución que
 * se espera de una flecha.
 *
 * `Escape` no está aquí a propósito. Cada cosa que se puede cerrar —un menú, un
 * diálogo, los ajustes— se cierra sola con Escape desde su propio componente,
 * que es quien sabe si está encima. Y no navega a Inicio: un Escape de más te
 * echaría de donde estabas sin manera de volver.
 */

const SALTO_CORTO = 5_000;
const SALTO_LARGO = 10_000;
const PASO_VOLUMEN = 0.05;

/** Marca del buscador, para `/`. Buscar por el texto del placeholder se rompe
 * en cuanto se traduce la interfaz. */
export const BUSCADOR = "data-buscador";

/** `true` si el foco está en algo donde se escribe. */
function escribiendo(): boolean {
  const el = document.activeElement as HTMLElement | null;
  if (!el) return false;
  const etiqueta = el.tagName;
  return (
    etiqueta === "INPUT" ||
    etiqueta === "TEXTAREA" ||
    etiqueta === "SELECT" ||
    el.isContentEditable
  );
}

/**
 * Lo último que hemos pedido, mientras el motor no lo confirme.
 *
 * Sin esto, pulsar la flecha cinco veces seguidas sube el volumen UNA vez. El
 * motor publica su estado cada 100 ms, así que las cinco pulsaciones leen el
 * mismo valor viejo y las cinco piden lo mismo. Lo mismo con `l` y los saltos.
 *
 * Se guarda con la hora: pasada la ventana manda otra vez el motor, que es lo
 * correcto si mientras tanto lo has movido con el ratón o ha cambiado la pista.
 */
function reciente(inicial: number) {
  const VENTANA = 600;
  let valor = inicial;
  let cuando = 0;
  return {
    /** El valor sobre el que aplicar el siguiente paso. */
    base: (delMotor: number) =>
      performance.now() - cuando < VENTANA ? valor : delMotor,
    anotar: (v: number) => {
      valor = v;
      cuando = performance.now();
    },
  };
}

const vol = reciente(1);
const pos = reciente(0);

/** Volumen antes de silenciar, para poder devolverlo donde estaba. */
let volumenPrevio = 1;

function saltar(ms: number) {
  const total = playback.durationMs;
  if (!total) return;
  const destino = Math.max(0, Math.min(total, pos.base(position()) + ms));
  pos.anotar(destino);
  api.seek(destino);
}

function ponerVolumen(v: number) {
  const limpio = Math.max(0, Math.min(1, v));
  vol.anotar(limpio);
  api.setVolume(limpio);
  return limpio;
}

function ajustarVolumen(delta: number) {
  const v = ponerVolumen(vol.base(playback.volume) + delta);
  if (v > 0) volumenPrevio = v;
  // La rueda de volumen está escondida detrás de un botón, así que sin esto
  // el cambio no se ve por ningún sitio.
  avisar(`Volumen ${Math.round(v * 100)} %`, "volumen");
}

function silenciar() {
  if (vol.base(playback.volume) > 0) {
    volumenPrevio = vol.base(playback.volume);
    ponerVolumen(0);
    avisar("Silenciado", "volumen");
  } else {
    const v = ponerVolumen(volumenPrevio || 1);
    avisar(`Volumen ${Math.round(v * 100)} %`, "volumen");
  }
}

function enfocarBuscador() {
  const campo = document.querySelector<HTMLInputElement>(`[${BUSCADOR}]`);
  if (!campo) return;
  campo.focus();
  campo.select();
}

/** Engancha los atajos a la ventana. Se llama una vez al arrancar. */
export function instalarAtajos() {
  window.addEventListener("keydown", (e) => {
    if (e.ctrlKey || e.altKey || e.metaKey) return;

    // `/` enfoca el buscador desde cualquier pantalla, pero solo si no estabas
    // ya escribiendo — dentro de un campo, una barra es una barra.
    if (e.key === "/" && !escribiendo()) {
      e.preventDefault();
      enfocarBuscador();
      return;
    }
    if (escribiendo()) return;

    switch (e.key) {
      case " ":
      case "k":
      case "K":
        // La barra espaciadora desplaza la página si no se para aquí.
        e.preventDefault();
        api.togglePlay();
        break;
      case "j":
      case "J":
        saltar(-SALTO_LARGO);
        break;
      case "l":
      case "L":
        saltar(SALTO_LARGO);
        break;
      case "ArrowLeft":
        e.preventDefault();
        saltar(-SALTO_CORTO);
        break;
      case "ArrowRight":
        e.preventDefault();
        saltar(SALTO_CORTO);
        break;
      case "ArrowUp":
        e.preventDefault();
        ajustarVolumen(PASO_VOLUMEN);
        break;
      case "ArrowDown":
        e.preventDefault();
        ajustarVolumen(-PASO_VOLUMEN);
        break;
      case "m":
      case "M":
        silenciar();
        break;
    }
  });
}
