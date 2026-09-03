import { Index, Show } from "solid-js";
import { palette, playback } from "../lib/store";
import { thumbAt } from "../lib/api";

/**
 * Fondo ambiental: la ventana entera iluminada con los colores de la portada.
 *
 * # Por que focos y no la portada desenfocada
 *
 * Estirar la portada a 48 px y dejar que el navegador la reescale sale gratis,
 * pero pinta *la foto*: quedan las formas del original, y hay que taparlas con
 * una vinieta oscura para que el texto se lea. El resultado es un fondo apagado
 * y sucio.
 *
 * Aqui se pintan degradados radiales con los colores que `palette.rs` saca de la
 * portada. No hay imagen, asi que no hay formas que tapar y no hace falta
 * vinieta: la ventana puede ser luminosa. Y es mas barato que antes — no hay
 * `filter: blur()`, que repinta cada fotograma, solo transformaciones que
 * resuelve la GPU.
 */

/**
 * Posiciones fijas de cada foco, en porcentaje de la ventana.
 *
 * Son fijas a proposito: si cada cancion recolocara los focos, el cambio de
 * pista se veria como un salto en vez de como un cambio de color. Lo unico que
 * varia entre canciones es el color y el tamanio.
 */
const SPOTS = [
  { x: 22, y: 26, size: 88 },
  { x: 86, y: 16, size: 74 },
  { x: 16, y: 88, size: 80 },
  { x: 90, y: 86, size: 70 },
  { x: 56, y: 54, size: 96 },
];

export function Ambient() {
  // Siempre se pintan los cinco focos, aunque la portada de menos colores: si
  // el numero de nodos cambiara con la cancion, los que sobran desaparecerian de
  // golpe en vez de apagarse.
  const slots = () =>
    SPOTS.map((spot, i) => ({
      spot,
      stop: palette().stops[i],
      // El color con el que se cruza este foco. Se salta uno a proposito: las
      // paradas vienen ordenadas por peso, asi que las contiguas suelen ser
      // parecidas, y cruzar dos turquesas casi identicos no mueve nada. Con dos
      // de distancia se cruza lo frio con lo calido de la portada, que es el
      // vaiven que se ve en la referencia.
      next: palette().stops[(i + 2) % Math.max(palette().stops.length, 1)],
    }));

  // Se pide diminuta a proposito. Estirarla a varias veces la ventana ES el
  // desenfoque, y sale gratis: el reescalado lo hace la GPU. Un `filter:blur()`
  // sobre una imagen grande repinta cada fotograma.
  const cover = () => thumbAt(playback.track?.thumbnail, 320, 180);

  return (
    <div class="ambient" aria-hidden="true">
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

      {/*
        La portada, borrosa y enorme, por encima de los focos.

        Es lo que hace la referencia: el tono de su fondo coincide con la
        carátula desenfocada con 16-25 grados de diferencia, mientras que
        comparada con la carátula nítida se va a 78-94. Y sigue moviéndose con
        la reproducción en pausa, así que no muestrea el vídeo: es una imagen
        fija en movimiento lento.

        Los focos de debajo no sobran: rellenan de color donde la portada es
        oscura, y son lo único que queda cuando aún no hay carátula.
      */}
      <Show when={cover()}>
        <div class="ambient-cover" style={{ "background-image": `url(${cover()})` }} />
      </Show>
    </div>
  );
}
