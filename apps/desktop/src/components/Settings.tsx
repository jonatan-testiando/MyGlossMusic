import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { api, type Storage } from "../lib/api";
import {
  ambientEnabled,
  setAmbientEnabled,
  setSettingsOpen,
  extractor,
  refreshExtractor,
} from "../lib/store";
import * as I from "./Icons";

/**
 * Ajustes.
 *
 * Solo lleva lo que la aplicación puede cambiar de verdad. La referencia tiene
 * ocho secciones — calidad de audio, descargas, privacidad, recomendaciones —
 * y aquí la mayoría no aplica: se reproduce siempre el mismo formato, no hay
 * descargas, no hay cuenta de la que sacar recomendaciones y no se envía nada a
 * ningún sitio. Un interruptor que no hace nada es peor que no tenerlo.
 *
 * El volumen, la repetición y el aleatorio no están aquí porque ya se guardan
 * solos al usarlos, que es donde tienen sentido.
 */

type Seccion = "general" | "almacenamiento" | "acerca";

function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const u = ["KB", "MB", "GB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${u[i]}`;
}

function Interruptor(p: { on: boolean; onToggle: () => void; label: string }) {
  return (
    <button
      class="relative h-6 w-11 shrink-0 rounded-full transition-colors"
      classList={{ "bg-[var(--accent)]": p.on, "bg-white/15": !p.on }}
      onClick={p.onToggle}
      role="switch"
      aria-checked={p.on}
      aria-label={p.label}
    >
      <span
        class="absolute top-0.5 size-5 rounded-full bg-white shadow transition-all"
        classList={{ "left-[22px]": p.on, "left-0.5": !p.on }}
      />
    </button>
  );
}

/**
 * Topes que se ofrecen para la caché.
 *
 * Una pista pesa ~3,5 MB, así que 3 GB son unas 850 canciones — más de lo que
 * nadie reescucha en una temporada. Por debajo de 1 GB la caché deja de servir
 * de nada, y por eso no hay opciones más pequeñas.
 */
const TOPES: [number, string][] = [
  [1024 ** 3, "1 GB"],
  [2 * 1024 ** 3, "2 GB"],
  [3 * 1024 ** 3, "3 GB"],
  [5 * 1024 ** 3, "5 GB"],
  [10 * 1024 ** 3, "10 GB"],
  [0, "Sin límite"],
];

function Fila(p: { titulo: string; nota?: string; children?: any }) {
  return (
    <div class="flex items-center justify-between gap-6 border-b border-white/[0.06] py-3.5 last:border-0">
      <div class="min-w-0">
        <div class="text-[13.5px] font-semibold text-white">{p.titulo}</div>
        <Show when={p.nota}>
          <div class="mt-0.5 text-[12px] leading-relaxed text-white/45">{p.nota}</div>
        </Show>
      </div>
      <div class="shrink-0">{p.children}</div>
    </div>
  );
}

export function SettingsDialog() {
  const [seccion, setSeccion] = createSignal<Seccion>("general");
  const [almacen, setAlmacen] = createSignal<Storage | null>(null);
  const [version, setVersion] = createSignal("");
  const [normaliza, setNormaliza] = createSignal(true);
  const [limpiando, setLimpiando] = createSignal(false);

  const cargarAlmacen = async () => {
    try {
      setAlmacen(await api.storageInfo());
    } catch {
      setAlmacen(null);
    }
  };

  onMount(() => {
    cargarAlmacen();
    api.appVersion().then(setVersion).catch(() => setVersion("?"));
    api.settings().then((a) => setNormaliza(a.normalize)).catch(() => {});
    refreshExtractor();

    const escape = (e: KeyboardEvent) => {
      if (e.key === "Escape") setSettingsOpen(false);
    };
    window.addEventListener("keydown", escape);
    onCleanup(() => window.removeEventListener("keydown", escape));
  });

  const secciones = [
    ["general", "General"],
    ["almacenamiento", "Almacenamiento"],
    ["acerca", "Acerca de"],
  ] as const;

  return (
    <div
      class="fixed inset-0 z-50 grid place-items-center bg-black/50 p-6 backdrop-blur-sm"
      onClick={(e) => {
        if (e.target === e.currentTarget) setSettingsOpen(false);
      }}
    >
      <div class="glass-card flex h-[min(560px,85vh)] w-full max-w-3xl overflow-hidden rounded-2xl">
        {/* Navegación lateral, como en la referencia. */}
        <nav class="w-48 shrink-0 space-y-1 border-r border-white/[0.08] bg-black/15 p-3">
          <div class="px-2 pb-2 text-base font-bold text-white">Ajustes</div>
          <For each={secciones}>
            {([id, etiqueta]) => (
              <button
                class="w-full rounded-lg px-3 py-2 text-left text-[13px] font-medium transition-colors"
                classList={{
                  "bg-white/[0.14] text-white": seccion() === id,
                  "text-white/60 hover:bg-white/[0.07] hover:text-white": seccion() !== id,
                }}
                onClick={() => setSeccion(id)}
              >
                {etiqueta}
              </button>
            )}
          </For>
        </nav>

        <div class="flex min-w-0 flex-1 flex-col">
          <div class="flex items-center justify-end border-b border-white/[0.08] px-4 py-2.5">
            <button
              class="icon-btn size-7 rounded-full hover:bg-white/10"
              onClick={() => setSettingsOpen(false)}
              title="Cerrar"
            >
              <I.Close size={14} />
            </button>
          </div>

          <div class="scroll-area flex-1 overflow-y-auto px-6 py-2">
            <Show when={seccion() === "general"}>
              <Fila
                titulo="Fondo animado"
                nota="La portada, desenfocada y en movimiento. Apagarlo deja un fondo fijo con los colores de la canción y ahorra GPU."
              >
                <Interruptor
                  on={ambientEnabled()}
                  onToggle={() => setAmbientEnabled(!ambientEnabled())}
                  label="Fondo animado"
                />
              </Fila>
              <Fila
                titulo="Igualar el volumen entre canciones"
                nota="Usa la medición que trae YouTube con cada pista. Sin esto hay casi 6 dB de diferencia entre unas y otras, y hay que tocar la rueda en cada cambio."
              >
                <Interruptor
                  on={normaliza()}
                  onToggle={async () => {
                    const v = !normaliza();
                    setNormaliza(v);
                    await api.setNormalize(v);
                  }}
                  label="Igualar el volumen entre canciones"
                />
              </Fila>

              <Fila
                titulo="Atajos de teclado"
                nota="Espacio o K reproduce y pausa · J y L saltan 10 s · ← y → saltan 5 s · ↑ y ↓ cambian el volumen · M silencia · / busca. No actúan mientras escribes."
              >
                <span class="rounded-full bg-white/10 px-3 py-1 text-[11px] font-semibold text-white/70">
                  Siempre
                </span>
              </Fila>

              <Fila
                titulo="Modo anónimo"
                nota="No hay sesión iniciada y las peticiones de audio nunca llevan cookies. No es configurable: es cómo está construida la aplicación."
              >
                <span class="rounded-full bg-white/10 px-3 py-1 text-[11px] font-semibold text-white/70">
                  Siempre
                </span>
              </Fila>
            </Show>

            <Show when={seccion() === "almacenamiento"}>
              <Fila
                titulo="Caché de audio"
                nota={
                  almacen()
                    ? `${almacen()!.cacheFiles} archivos · ${almacen()!.cacheDir}`
                    : "Calculando…"
                }
              >
                <div class="flex items-center gap-3">
                  <span class="text-sm font-bold tabular-nums text-white">
                    {almacen() ? bytes(almacen()!.cacheBytes) : "—"}
                  </span>
                  <button
                    class="rounded-full border border-white/10 bg-white/10 px-3.5 py-1.5 text-xs font-semibold text-white hover:bg-white/15 disabled:opacity-40"
                    disabled={limpiando() || !almacen()?.cacheFiles}
                    onClick={async () => {
                      setLimpiando(true);
                      try {
                        await api.clearCache();
                        await cargarAlmacen();
                      } finally {
                        setLimpiando(false);
                      }
                    }}
                  >
                    {limpiando() ? "Vaciando…" : "Vaciar"}
                  </button>
                </div>
              </Fila>

              <Fila
                titulo="Tope de la caché"
                nota="Al pasarse, se borran las pistas que llevan más tiempo sin sonar. Nunca la que está reproduciéndose."
              >
                <select
                  class="rounded-full border border-white/10 bg-white/10 px-3.5 py-1.5 text-xs font-semibold text-white outline-none"
                  value={String(almacen()?.cacheLimit ?? 0)}
                  onChange={async (e) => {
                    await api.setCacheLimit(Number(e.currentTarget.value));
                    await cargarAlmacen();
                  }}
                >
                  <For each={TOPES}>
                    {([bytes, etiqueta]) => (
                      <option value={String(bytes)} class="bg-neutral-900">
                        {etiqueta}
                      </option>
                    )}
                  </For>
                </select>
              </Fila>

              <Fila
                titulo="Base de datos"
                nota={
                  almacen()
                    ? `Favoritos, historial, playlists y ajustes · ${almacen()!.dataDir}`
                    : "Calculando…"
                }
              >
                <span class="text-sm font-bold tabular-nums text-white">
                  {almacen() ? bytes(almacen()!.dbBytes) : "—"}
                </span>
              </Fila>

              <p class="pt-4 text-[12px] leading-relaxed text-white/40">
                Vaciar la caché no borra nada tuyo: las canciones se vuelven a
                descargar al reproducirlas. La base de datos no se toca desde aquí
                a propósito.
              </p>
            </Show>

            <Show when={seccion() === "acerca"}>
              <Fila titulo="MyGlossMusic" nota="Reproductor de YouTube Music nativo. Rust + Tauri + SolidJS.">
                <span class="font-mono text-[12px] text-white/60">{version()}</span>
              </Fila>
              <Fila
                titulo="Extractor"
                nota={
                  extractor()?.available
                    ? (extractor()!.program ?? "")
                    : "No encontrado: la reproducción queda capada a ~48 s por pista."
                }
              >
                <span class="flex items-center gap-2 text-[12px] text-white/70">
                  <span
                    class="size-2 rounded-full"
                    style={{ background: extractor()?.available ? "#4ade80" : "#f87171" }}
                  />
                  yt-dlp {extractor()?.version ?? ""}
                </span>
              </Fila>
              <Fila titulo="Licencia" nota="GPLv3. El código es tuyo para leerlo y cambiarlo.">
                <span class="text-[12px] text-white/60">GPLv3</span>
              </Fila>
              <p class="pt-4 text-[12px] leading-relaxed text-white/40">
                Esto viola los Términos de Servicio de YouTube. Funciona en modo
                anónimo: sin sesión iniciada no hay ninguna cuenta que Google pueda
                sancionar.
              </p>
            </Show>
          </div>
        </div>
      </div>
    </div>
  );
}
