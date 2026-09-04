/**
 * La portada volando entre la vista de reproducción y el miniplayer.
 *
 * # Por qué hace falta
 *
 * Abrir y cerrar la canción cambiaba de pantalla de golpe. Con la portada
 * ocupando media ventana en un lado y una tarjeta en la esquina en el otro, el
 * salto se lee como "he cambiado de vista", no como "esto se ha hecho grande".
 *
 * # Cómo
 *
 * Es un FLIP: se mide de dónde sale, se hace el cambio, se mide dónde ha caído
 * y se anima un clon entre las dos posiciones mientras el de verdad está
 * escondido. Las dos portadas nunca existen a la vez —cada una vive dentro de
 * su `Show`— así que animar los elementos reales no es una opción.
 *
 * Se animan `left/top/width/height` y no `transform`. Con transform el clon
 * llevaría escalas distintas en cada eje (la tarjeta es 16:9 y la portada tiene
 * la proporción que tenga) y la imagen se vería estirada durante el vuelo; con
 * la caja, `object-fit: cover` recorta igual que en los dos extremos y el
 * resultado es exacto. Es un solo elemento fijo con su propia capa, así que el
 * coste de no ir por el compositor no se nota.
 */

/**
 * Marca de las dos portadas en el DOM.
 *
 * Se buscan por atributo en vez de pasarse por `ref`: viven en componentes
 * distintos y entran y salen con un `Show`, así que dos referencias habría que
 * mantenerlas sincronizadas con `onCleanup` para acabar consultando lo mismo.
 */
const GRANDE = '[data-portada="grande"]';
const MINI = '[data-portada="mini"]';

const DURACION = 420;

/** La curva de iOS: sale rápido y frena largo. Es lo que hace que "pese". */
const CURVA = "cubic-bezier(0.32, 0.72, 0, 1)";

function quieto(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Un rectángulo utilizable: los de tamaño cero son de una imagen sin cargar. */
function medir(sel: string): { el: HTMLImageElement; caja: DOMRect } | null {
  const el = document.querySelector<HTMLImageElement>(sel);
  if (!el) return null;
  const caja = el.getBoundingClientRect();
  return caja.width > 4 && caja.height > 4 ? { el, caja } : null;
}

/**
 * Hace el cambio de vista con la portada volando de un sitio al otro.
 *
 * El cambio se pasa como función porque hay que medir a los dos lados de él:
 * el origen antes, cuando todavía existe, y el destino después, cuando ya está
 * en el árbol. Las dos medidas son síncronas — `getBoundingClientRect` fuerza
 * el cálculo del diseño, así que no hay que esperar a ningún cuadro y la
 * animación arranca dentro del mismo gesto. Con `requestAnimationFrame` no solo
 * se perdía un cuadro: una ventana que no está repartiendo cuadros no lo
 * ejecuta nunca, y el cambio se quedaba sin animar sin dar ninguna señal.
 *
 * Si falta cualquiera de los dos extremos el cambio se hace igual, solo que
 * seco. Preferible a quedarse a medias o a dejar un clon colgado.
 */
export function cambiarConPortada(abriendo: boolean, cambiar: () => void) {
  if (quieto()) return cambiar();

  const origen = medir(abriendo ? MINI : GRANDE)?.caja;
  cambiar();
  if (!origen) return;

  const destino = medir(abriendo ? GRANDE : MINI);
  if (!destino) return;

  const url = destino.el.currentSrc || destino.el.src;
  if (!url) return;

  const clon = document.createElement("img");
  clon.src = url;
  clon.alt = "";
  clon.style.cssText = [
    "position:fixed",
    "z-index:40",
    "margin:0",
    "object-fit:cover",
    "border-radius:16px",
    "pointer-events:none",
    "box-shadow:0 25px 60px -15px rgba(0,0,0,0.85)",
    "will-change:left,top,width,height",
  ].join(";");
  document.body.appendChild(clon);

  // El de verdad se esconde hasta que el clon aterriza encima. La transición se
  // corta a la vez: el miniplayer lleva `transition-all`, y sin esto no se
  // escondería, se desvanecería a lo largo de todo el vuelo.
  const transicionPrevia = destino.el.style.transition;
  destino.el.style.transition = "none";
  destino.el.style.opacity = "0";

  const caja = (r: DOMRect) => ({
    left: `${r.left}px`,
    top: `${r.top}px`,
    width: `${r.width}px`,
    height: `${r.height}px`,
  });

  const animacion = clon.animate([caja(origen), caja(destino.caja)], {
    duration: DURACION,
    easing: CURVA,
    fill: "both",
  });

  let hecho = false;
  const terminar = () => {
    if (hecho) return;
    hecho = true;
    destino.el.style.opacity = "";
    destino.el.style.transition = transicionPrevia;
    clon.remove();
  };
  animacion.addEventListener("finish", terminar);

  // El temporizador no es un adorno, es la garantía. Los eventos de animación
  // se despachan en el bucle de dibujado, así que una ventana que no está
  // pintando —minimizada, tapada, en otro escritorio— no los recibe nunca, y
  // sin esto el clon se quedaría tapando media pantalla y la portada de destino
  // invisible. `setTimeout` corre igual. Los 60 ms de margen son para que en
  // condiciones normales gane el evento, que es exacto.
  window.setTimeout(terminar, DURACION + 60);
}
