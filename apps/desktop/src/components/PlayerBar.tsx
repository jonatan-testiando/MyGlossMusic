import { Show, createSignal } from "solid-js";
import { api, fmtTime, thumbAt } from "../lib/api";
import { playback, position, isFavorite, toggleFavorite, fullLyricsOpen, setFullLyricsOpen } from "../lib/store";
import * as I from "./Icons";

/** Barra de reproduccion flotante estilo DemoApp. */
export function PlayerBar() {
  let scrubBarRef!: HTMLDivElement;
  const [dragging, setDragging] = createSignal<number | null>(null);
  const [volumeOpen, setVolumeOpen] = createSignal(false);

  const duration = () => playback.durationMs || 1;
  const shown = () => dragging() ?? position();
  const pct = () => Math.min(100, (shown() / duration()) * 100);

  const scrubTo = (clientX: number, commit: boolean) => {
    if (!scrubBarRef) return;
    const rect = scrubBarRef.getBoundingClientRect();
    if (rect.width <= 0) return;
    const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    const ms = Math.floor(ratio * duration());
    if (commit) {
      api.seek(ms);
      setDragging(null);
    } else {
      setDragging(ms);
    }
  };

  const handleMouseDown = (e: MouseEvent) => {
    scrubTo(e.clientX, false);
    const onMove = (ev: MouseEvent) => {
      scrubTo(ev.clientX, false);
    };
    const onUp = (ev: MouseEvent) => {
      scrubTo(ev.clientX, true);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  const cycleRepeat = () => {
    const next = playback.repeat === "off" ? "all" : playback.repeat === "all" ? "one" : "off";
    api.setRepeat(next);
  };

  return (
    <footer class="no-drag shrink-0 px-4 pb-4">
      <div class="panel relative px-4 pt-3 pb-3">
        {/* Barra de progreso superior interactiva con hover suave */}
        <div class="absolute -top-1.5 inset-x-4 h-3 flex items-center cursor-pointer group"
          ref={scrubBarRef}
          onMouseDown={handleMouseDown}
          onClick={(e) => scrubTo(e.clientX, true)}
        >
          <div class="scrub w-full relative h-[3px] group-hover:h-[5px] transition-all duration-150 rounded-full bg-white/15">
            <div class="scrub-buffered" style={{ width: `${playback.buffered * 100}%` }} />
            <div class="scrub-played" style={{ width: `${pct()}%` }} />
            <div
              class="scrub-knob absolute top-1/2 -translate-y-1/2 size-3 rounded-full bg-white shadow-md transition-transform scale-0 group-hover:scale-100 pointer-events-none"
              style={{ left: `${pct()}%` }}
            />
          </div>
        </div>

        {/* 3 Columnas: Transporte + Info Pista + Opciones & Volumen */}
        <div class="grid grid-cols-[1.1fr_1.3fr_1.1fr] items-center gap-4 pt-1">
          {/* Columna Izquierda: Botones de transporte + Tiempo */}
          <div class="flex items-center gap-1.5">
            <button
              class="icon-btn size-8"
              classList={{ "text-[var(--accent)]": playback.shuffle }}
              onClick={() => api.setShuffle(!playback.shuffle)}
              title="Aleatorio"
            >
              <I.Shuffle size={16} />
            </button>
            <button class="icon-btn size-8" onClick={() => api.prev()} title="Anterior">
              <I.Prev size={18} />
            </button>
            <button
              class="icon-btn size-10 rounded-full !opacity-100 bg-white/10 hover:bg-white/20 active:scale-95 transition-all shadow-md"
              onClick={() => api.togglePlay()}
              title={playback.playing ? "Pausa" : "Reproducir"}
            >
              <Show when={!playback.loading} fallback={<Spinner />}>
                <Show when={playback.playing} fallback={<I.Play size={18} />}>
                  <I.Pause size={18} />
                </Show>
              </Show>
            </button>
            <button class="icon-btn size-8" onClick={() => api.next()} title="Siguiente">
              <I.Next size={18} />
            </button>
            <button
              class="icon-btn size-8"
              classList={{ "text-[var(--accent)]": playback.repeat !== "off" }}
              onClick={cycleRepeat}
              title={`Repetir: ${playback.repeat}`}
            >
              <Show when={playback.repeat === "one"} fallback={<I.Repeat size={16} />}>
                <I.RepeatOne size={16} />
              </Show>
            </button>

            {/* Contador de tiempo 0:19 / 3:10 */}
            <span class="ml-2 text-[11.5px] tabular-nums font-medium opacity-65">
              {fmtTime(shown())} <span class="opacity-35">/</span> {fmtTime(playback.durationMs)}
            </span>
          </div>

          {/* Columna Central: Información de la canción actual */}
          <div class="flex min-w-0 items-center justify-center gap-3">
            <Show
              when={playback.track?.thumbnail}
              fallback={<div class="size-10 shrink-0 rounded-lg bg-white/5" />}
            >
              <img
                src={thumbAt(playback.track!.thumbnail, 96)!}
                alt=""
                class="size-10 shrink-0 rounded-lg object-cover ring-1 ring-white/10 shadow-sm"
              />
            </Show>
            <div class="min-w-0 max-w-[240px] text-center">
              <div class="truncate text-[13px] font-semibold leading-tight">
                {playback.track?.title ?? "Nada sonando"}
              </div>
              <div class="truncate text-[11px] opacity-50 mt-0.5">{playback.track?.author ?? ""}</div>
            </div>
            <Show when={playback.track}>
              <button
                class="icon-btn size-8 shrink-0"
                classList={{ "!text-[var(--accent)] !opacity-100": isFavorite() }}
                onClick={() => toggleFavorite()}
                title={isFavorite() ? "Quitar de favoritos" : "Añadir a favoritos"}
              >
                <Show when={isFavorite()} fallback={<I.Heart size={16} />}>
                  <I.HeartFilled size={16} />
                </Show>
              </button>
            </Show>
          </div>

          {/* Columna Derecha: Volumen + Letras pantalla completa */}
          <div class="flex items-center justify-end gap-2">
            <div
              class="flex items-center gap-2"
              onMouseEnter={() => setVolumeOpen(true)}
              onMouseLeave={() => setVolumeOpen(false)}
            >
              <button
                class="icon-btn size-8"
                onClick={() => api.setVolume(playback.volume > 0 ? 0 : 1)}
                title="Volumen"
              >
                <Show when={playback.volume > 0} fallback={<I.VolumeMute size={17} />}>
                  <I.Volume size={17} />
                </Show>
              </button>
              <div
                class="overflow-hidden transition-all duration-200"
                style={{ width: volumeOpen() ? "84px" : "0px" }}
              >
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={playback.volume}
                  onInput={(e) => api.setVolume(Number(e.currentTarget.value))}
                  class="w-full accent-[var(--accent)] h-1 cursor-pointer"
                  aria-label="Volumen"
                />
              </div>
            </div>

            {/* Botón de letras en vivo estilo Apple Music / Fullscreen */}
            <button
              class="icon-btn size-8 transition-colors"
              classList={{
                "bg-white/20 text-white": fullLyricsOpen(),
                "opacity-60 hover:opacity-100 hover:bg-white/10": !fullLyricsOpen(),
              }}
              onClick={() => setFullLyricsOpen(!fullLyricsOpen())}
              title="Letras en pantalla completa"
            >
              <I.Lyrics size={16} />
            </button>
          </div>
        </div>
      </div>
    </footer>
  );
}

function Spinner() {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" class="animate-spin" aria-hidden="true">
      <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-dasharray="32" stroke-linecap="round" opacity="0.4" />
      <path d="M12 2a10 10 0 0 1 10 10" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
    </svg>
  );
}
