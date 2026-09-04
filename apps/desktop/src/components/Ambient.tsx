import { Index, Show, createEffect, createSignal, on, onCleanup, onMount } from "solid-js";
import { Kawarp } from "@kawarp/core";
import { ambientEnabled, palette, trackVisible } from "../lib/store";
import { thumbAt } from "../lib/api";

/**
 * Fondo ambiental: la ventana entera iluminada con la portada de la cancion.
 *
 * # De donde sale esto
 *
 * Es el mismo efecto de la referencia y con la misma pieza. Rastreado hasta el
 * final: el tema pinta la portada como imagen de fondo (escalada, saturada,
 * oscurecida) y encima corre `@kawarp/core`, un renderizador WebGL con
 * desenfoque de Kawase y *domain warping*. Por eso su fondo sigue fluyendo con
 * la reproduccion en pausa: el warp corre en su propio reloj, no en el del
 * reproductor.
 *
 * Y por eso una animacion CSS nunca iba a bastar. Nosotros moviamos la imagen
 * entera; el warp deforma el espacio donde se muestrea, asi que los colores se
 * meten unos dentro de otros en vez de deslizarse.
 *
 * # Que queda debajo
 *
 * Los focos de la paleta siguen aqui como respaldo, para cuando no hay WebGL o
 * la portada no carga. No es un caso raro que se pueda ignorar: sin ellos, esos
 * usuarios se quedan con una ventana negra.
 */

/**
 * Posiciones fijas de cada foco del respaldo, en porcentaje de la ventana.
 *
 * Son fijas a proposito: si cada cancion recolocara los focos, el cambio de
 * pista se veria como un salto en vez de como un cambio de color.
 */
const SPOTS = [
  { x: 22, y: 26, size: 88 },
  { x: 86, y: 16, size: 74 },
  { x: 16, y: 88, size: 80 },
  { x: 90, y: 86, size: 70 },
  { x: 56, y: 54, size: 96 },
];

/**
 * Resolucion del lienzo respecto a la ventana.
 *
 * Es un campo de color desenfocado: no hay detalle que perder, y el coste de un
 * shader a pantalla completa va con el numero de pixeles. A 0,6 se pinta un
 * tercio de los pixeles y no se ve la diferencia.
 */
const CANVAS_SCALE = 0.6;

/** "#rrggbb" a las tres componentes 0..1 que espera Kawarp. */
function hexToRgb01(hex: string): [number, number, number] {
  const n = Number.parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

export function Ambient() {
  let canvas!: HTMLCanvasElement;
  const [webgl, setWebgl] = createSignal(false);

  // Siempre se pintan los cinco focos del respaldo, aunque la portada de menos
  // colores: si el numero de nodos cambiara con la cancion, los que sobran
  // desapareceria de golpe en vez de apagarse.
  const slots = () =>
    SPOTS.map((spot, i) => ({
      spot,
      stop: palette().stops[i],
      next: palette().stops[(i + 2) % Math.max(palette().stops.length, 1)],
    }));

  // Se pide pequena: el shader la desenfoca de todas formas, y una portada de
  // 1280 px solo aniadiria descarga y memoria de textura.
  const cover = () => thumbAt(trackVisible()?.thumbnail, 320, 180);

  onMount(() => {
    let kawarp: Kawarp;
    try {
      kawarp = new Kawarp(canvas, {
        // `saturation` y `scale` son los del tema que inspira la referencia.
        saturation: 1.8,
        scale: 1.1,
        warpIntensity: 1.0,
        blurPasses: 10,
        animationSpeed: 2.4,
        transitionDuration: 1400,
        tintIntensity: 0.10,
      });
    } catch (e) {
      // Sin WebGL no hay excusa para dejar la ventana negra: quedan los focos.
      console.warn("sin WebGL, se usa el fondo de respaldo", e);
      return;
    }
    setWebgl(true);

    // El respaldo del respaldo: si el contexto se pierde en marcha (al
    // suspender el equipo, por ejemplo) se vuelve a los focos en vez de dejar
    // un lienzo congelado.
    const onLost = () => setWebgl(false);
    canvas.addEventListener("webglcontextlost", onLost);

    const quieto = window.matchMedia("(prefers-reduced-motion: reduce)");

    const ajustar = () => {
      const w = Math.max(1, Math.round(canvas.clientWidth * CANVAS_SCALE));
      const h = Math.max(1, Math.round(canvas.clientHeight * CANVAS_SCALE));
      if (canvas.width === w && canvas.height === h) return;
      // Kawarp lee `canvas.width`, no el tamano en CSS, asi que hay que
      // asignarlo antes de avisarle.
      canvas.width = w;
      canvas.height = h;
      kawarp.resize();
      if (quieto.matches) kawarp.renderFrame();
    };
    ajustar();

    const observer = new ResizeObserver(ajustar);
    observer.observe(canvas);

    const aplicarMovimiento = () => {
      // El ajuste del usuario manda sobre todo lo demás; después, la
      // preferencia del sistema de movimiento reducido.
      if (!ambientEnabled() || quieto.matches) {
        kawarp.stop();
        kawarp.renderFrame();
      } else {
        kawarp.start();
      }
    };
    createEffect(aplicarMovimiento);
    quieto.addEventListener("change", aplicarMovimiento);

    // La portada. Sin ella, un degradado con los colores de la paleta: asi el
    // shader tiene algo que deformar desde el primer instante.
    createEffect(
      on(cover, (url) => {
        const pintarSiQuieto = () => {
          if (quieto.matches) kawarp.renderFrame();
        };
        if (!url) {
          kawarp.loadGradient(palette().stops.map((s) => s.color));
          pintarSiQuieto();
          return;
        }
        kawarp
          .loadImage(url)
          .then(pintarSiQuieto)
          .catch(() => {
            kawarp.loadGradient(palette().stops.map((s) => s.color));
            pintarSiQuieto();
          });
      }),
    );

    // El tinte de las zonas oscuras lo pone la paleta, para que las sombras
    // tambien lleven el color de la cancion.
    createEffect(() => {
      kawarp.tintColor = hexToRgb01(palette().background);
    });

    onCleanup(() => {
      quieto.removeEventListener("change", aplicarMovimiento);
      canvas.removeEventListener("webglcontextlost", onLost);
      observer.disconnect();
      kawarp.dispose();
    });
  });

  return (
    <div class="ambient" aria-hidden="true">
      <canvas ref={canvas} class="ambient-canvas" classList={{ "opacity-0": !webgl() }} />

      {/* Respaldo en CSS: focos de color y la portada desenfocada. */}
      <Show when={!webgl()}>
        <Index each={slots()}>
          {(slot) => (
            <div
              class="ambient-spot"
              style={{
                "--c": slot().stop?.color ?? palette().background,
                "--c2": slot().next?.color ?? slot().stop?.color ?? palette().background,
                "--w": String(slot().stop?.weight ?? 0),
                "--x": `${slot().spot.x}%`,
                "--y": `${slot().spot.y}%`,
                "--size": `${slot().spot.size}%`,
              }}
            >
              <div class="ambient-spot-alt" />
            </div>
          )}
        </Index>

        <Show when={cover()}>
          <div class="ambient-cover" style={{ "background-image": `url(${cover()})` }} />
        </Show>
      </Show>
    </div>
  );
}
