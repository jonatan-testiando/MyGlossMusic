import { Show } from "solid-js";
import { api } from "../lib/api";
import {
  instalandoUpdate,
  setInstalandoUpdate,
  updateProgress,
  updateReady,
} from "../lib/store";
import * as I from "./Icons";

/**
 * Aviso de actualización, y el velo que tapa el reinicio.
 *
 * La descarga la arranca Rust solo, veinte segundos después de abrir (ver
 * `src-tauri/src/actualizacion.rs`). Aquí no se descarga nada: solo se pinta
 * en qué punto está y, cuando ya está lista, un botón.
 *
 * No hay ninguna ventana del sistema en todo el recorrido. El instalador de
 * NSIS corre en silencio (`installMode: "quiet"` en `tauri.conf.json`), así que
 * lo único que se ve es esta píldora y luego el velo.
 */

/** Segundos de velo antes de soltar el instalador. Ver [`VeloDeInstalacion`]. */
const ESCENA = 4000;

async function instalar() {
  setInstalandoUpdate(true);
  // El velo necesita que le dé tiempo a entrar. Sin esta pausa el proceso
  // muere tan rápido que no se llega a pintar, y la ventana desapareciendo de
  // golpe para volver medio minuto después se lee como un cuelgue, no como una
  // actualización.
  await new Promise((r) => setTimeout(r, ESCENA));
  try {
    // En Windows esto no vuelve: el instalador toma el relevo y mata el
    // proceso. La `/R` de NSIS relanza la aplicación al terminar.
    await api.updateInstall();
  } catch (e) {
    console.error(e);
    setInstalandoUpdate(false);
  }
}

/**
 * La píldora de la barra lateral.
 *
 * Tres estados y ninguno interrumpe: nada mientras no hay novedad, el
 * porcentaje mientras baja, y el botón cuando ya se puede instalar.
 */
export function PildoraActualizacion(p: { compacta?: boolean }) {
  return (
    <Show when={updateReady() ?? updateProgress()} keyed={false}>
      <Show
        when={!p.compacta}
        fallback={
          // Con el raíl cerrado no cabe texto. Un punto basta para que se sepa
          // que hay algo, y el resto está en Ajustes.
          <button
            class="mx-auto flex size-8 items-center justify-center rounded-full transition-colors"
            classList={{
              "bg-[var(--accent)]/20 text-[var(--accent)] hover:bg-[var(--accent)]/30":
                !!updateReady(),
              "text-white/40": !updateReady(),
            }}
            onClick={() => updateReady() && instalar()}
            title={
              updateReady()
                ? `Actualización ${updateReady()!.version} lista para instalar`
                : `Descargando ${updateProgress()!.version}… ${updateProgress()!.percent}%`
            }
          >
            <I.Update size={16} />
          </button>
        }
      >
        <Show
          when={updateReady()}
          fallback={
            <div
              class="rounded-xl px-2.5 py-2 text-white/55"
              title="Se está descargando en segundo plano. Puedes seguir usando la aplicación."
            >
              <div class="flex items-center gap-1.5 text-[11px] font-medium">
                <I.Update size={13} class="shrink-0 text-white/35" />
                <span class="tabular-nums">
                  v{updateProgress()!.version} · {updateProgress()!.percent}%
                </span>
              </div>
              <div class="mt-1.5 h-[3px] overflow-hidden rounded-full bg-white/10">
                <div
                  class="h-full rounded-full bg-white/40 transition-[width] duration-200"
                  style={{ width: `${updateProgress()!.percent}%` }}
                />
              </div>
            </div>
          }
        >
          <button
            class="flex w-full items-center gap-1.5 rounded-xl border border-[var(--accent)]/30 bg-[var(--accent)]/15 px-2.5 py-2 text-left text-xs font-semibold text-white transition-colors hover:bg-[var(--accent)]/25"
            onClick={instalar}
            title="Ya está descargada. Un clic: se instala y la aplicación vuelve sola."
          >
            <I.Update size={13} class="shrink-0 text-[var(--accent)]" />
            <span>Instalar v{updateReady()!.version}</span>
          </button>
        </Show>
      </Show>
    </Show>
  );
}

/**
 * Lo que se ve mientras la aplicación se cierra para instalar.
 *
 * Existe porque el tránsito es invisible si no se cuenta: el proceso muere en
 * cuanto arranca el instalador y la ventana se esfuma. Sin nada que lo explique,
 * volver medio minuto después parece un fallo.
 */
export function VeloDeInstalacion() {
  return (
    <Show when={instalandoUpdate()}>
      <div class="fixed inset-0 z-[100] flex flex-col items-center justify-center gap-5 bg-black/85 backdrop-blur-md">
        <div class="size-10 animate-spin rounded-full border-2 border-white/15 border-t-[var(--accent)]" />
        <div class="text-center">
          <p class="text-sm font-semibold text-white">Instalando la actualización</p>
          <p class="mt-1.5 text-[12px] text-white/50">
            La aplicación se cerrará y volverá sola. No hace falta que toques nada.
          </p>
        </div>
      </div>
    </Show>
  );
}
