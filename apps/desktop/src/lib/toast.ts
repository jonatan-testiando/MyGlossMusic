import { createSignal } from "solid-js";

/**
 * Avisos flotantes.
 *
 * # Por qué
 *
 * Hasta ahora, cuando algo iba mal —se cayó la red, YouTube devolvió un 429, no
 * se pudo guardar una playlist— el único rastro era un `console.error`. Es
 * decir: para el usuario, no pasaba nada, simplemente la aplicación se quedaba
 * quieta. Y al revés, las cosas que sí funcionaban tampoco se confirmaban:
 * añadir una canción a una playlist desde el menú no daba ninguna señal.
 *
 * # Reglas
 *
 * Un aviso es una frase corta sobre algo que ACABA de pasar. No sustituye a un
 * estado permanente: si algo está mal de forma continuada, eso va en la
 * pantalla, no en un aviso que se va solo a los tres segundos.
 */

export type Tipo = "info" | "error";

export type Aviso = {
  id: number;
  texto: string;
  tipo: Tipo;
  /** Etiqueta opcional: un aviso nuevo con la misma sustituye al anterior. */
  clave?: string;
};

const VIDA = { info: 3_200, error: 5_500 };

/** Más de tres a la vez tapan media pantalla y ya no se leen. */
const MAXIMO = 3;

const [avisos, setAvisos] = createSignal<Aviso[]>([]);
export { avisos };

const relojes = new Map<number, number>();
let siguienteId = 1;

export function descartar(id: number) {
  const t = relojes.get(id);
  if (t !== undefined) {
    clearTimeout(t);
    relojes.delete(id);
  }
  setAvisos((lista) => lista.filter((a) => a.id !== id));
}

function mostrar(texto: string, tipo: Tipo, clave?: string) {
  const id = siguienteId++;

  setAvisos((lista) => {
    // La clave existe para el volumen y compañía: pulsar la flecha diez veces
    // seguidas tiene que dejar UN aviso que se actualiza, no diez apilados.
    const limpia = clave ? lista.filter((a) => a.clave !== clave) : lista;
    if (clave) {
      for (const viejo of lista) {
        if (viejo.clave === clave) descartar(viejo.id);
      }
    }
    return [...limpia, { id, texto, tipo, clave }].slice(-MAXIMO);
  });

  relojes.set(id, window.setTimeout(() => descartar(id), VIDA[tipo]));
  return id;
}

/** Algo salió bien y no se ve por ningún otro sitio. */
export function avisar(texto: string, clave?: string) {
  return mostrar(texto, "info", clave);
}

/** Algo falló. Dura más, porque hay que llegar a leerlo. */
export function avisarError(texto: string) {
  return mostrar(texto, "error");
}

/**
 * Traduce un fallo a algo que se pueda leer.
 *
 * Los errores llegan como cadenas desde Rust, y la mayoría son el `to_string()`
 * de un `anyhow` con toda la cadena de contexto. Eso no se le enseña a nadie;
 * lo que importa es de qué tipo de problema se trata.
 */
export function motivo(e: unknown, porDefecto: string): string {
  const texto = String((e as { message?: string })?.message ?? e ?? "");

  if (!navigator.onLine) return "Sin conexión a internet.";
  if (/\b429\b|too many requests/i.test(texto)) {
    return "YouTube está limitando las peticiones. Prueba en un minuto.";
  }
  if (/\b40[13]\b|forbidden|unauthorized/i.test(texto)) {
    return "YouTube ha rechazado la petición.";
  }
  if (/timed? ?out|timeout/i.test(texto)) return "La petición ha tardado demasiado.";
  if (/dns|connect|network|error sending request/i.test(texto)) {
    return "No se pudo conectar con YouTube.";
  }
  return porDefecto;
}

/**
 * Avisa al perder y recuperar la conexión.
 *
 * Se llama una vez al arrancar. Lo de recuperarla también se dice: si no, te
 * quedas sin saber si ya puedes volver a intentarlo.
 */
export function vigilarConexion() {
  window.addEventListener("offline", () =>
    avisarError("Sin conexión a internet."),
  );
  window.addEventListener("online", () => avisar("Conexión recuperada.", "red"));
}
