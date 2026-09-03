import { Show, createSignal } from "solid-js";
import { api, fmtTime, thumbAt } from "../lib/api";
import { playback, position, isFavorite, toggleFavorite } from "../lib/store";
import * as I from "./Icons";

/** Barra de reproduccion flotante. */
export function PlayerBar() {
  const [dragging, setDragging] = createSignal<number | null>(null);
  const [volumeOpen, setVolumeOpen] = createSignal(false);

  const duration = () => playback.durationMs || 1;
  const shown = () => dragging() ?? position();
  const pct = () => Math.min(100, (shown() / duration()) * 100);

  const scrubTo = (e: MouseEvent, commit: boolean) => {
    const bar = e.currentTarget as HTMLElement;
    const rect = bar.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
    const ms = ratio * duration();
    if (commit) {
      api.seek(Math.floor(ms));
      setDragging(null);
    } else {
      setDragging(ms);
    }
  };

  const cycleRepeat = () => {
    const next = playback.repeat === "off" ? "all" : playback.repeat === "all" ? "one" : "off";
    api.setRepeat(next);
  };

  return (
    <footer class="no-drag shrink-0 px-4 pb-4">
      <div class="panel px-4 py-3">
        {/* Progreso */}
        <div class="flex items-center gap-3">
          <span class="w-11 text-right text-[11px] tabular-nums opacity-60">
            {fmtTime(shown())}
          </span>
          <div
            class="scrub flex-1"
            onMouseDown={(e) => scrubTo(e, false)}
            onMouseMove={(e) => dragging() !== null && scrubTo(e, false)}
            onMouseUp={(e) => scrubTo(e, true)}
            onMouseLeave={() => setDragging(null)}
            role="slider"
            aria-label="Posición"
            aria-valuemin={0}
            aria-valuemax={duration()}
            aria-valuenow={shown()}
            tabindex={0}
          >
            <div class="scrub-buffered" style={{ width: `${playback.buffered * 100}%` }} />
            <div class="scrub-played" style={{ width: `${pct()}%` }} />
            <div class="scrub-knob" style={{ left: `${pct()}%` }} />
          </div>
          <span class="w-11 text-[11px] tabular-nums opacity-60">
            {fmtTime(playback.durationMs)}
          </span>
        </div>

        {/* Controles */}
        <div class="mt-2.5 grid grid-cols-[1fr_auto_1fr] items-center gap-4">
          {/* Pista actual */}
          <div class="flex min-w-0 items-center gap-3">
            <Show
              when={playback.track?.thumbnail}
              fallback={<div class="size-11 shrink-0 rounded-lg bg-white/8" />}
            >
              <img
                src={thumbAt(playback.track!.thumbnail, 96)!}
                alt=""
                class="size-11 shrink-0 rounded-lg object-cover"
              />
            </Show>
            <div class="min-w-0">
              <div class="truncate text-[13px] font-medium">
                {playback.track?.title ?? "Nada sonando"}
              </div>
              <div class="truncate text-[11px] opacity-55">{playback.track?.author ?? ""}</div>
            </div>
            <Show when={playback.track}>
              <button
                class="icon-btn size-8 shrink-0"
                classList={{ active: isFavorite() }}
                onClick={() => toggleFavorite()}
                title={isFavorite() ? "Quitar de favoritos" : "Añadir a favoritos"}
              >
                <Show when={isFavorite()} fallback={<I.Heart size={16} />}>
                  <I.HeartFilled size={16} />
                </Show>
              </button>
            </Show>
          </div>

          {/* Transporte */}
          <div class="flex items-center gap-1">
            <button
              class="icon-btn size-9"
              classList={{ active: playback.shuffle }}
              onClick={() => api.setShuffle(!playback.shuffle)}
              title="Aleatorio"
            >
              <I.Shuffle size={17} />
            </button>
            <button class="icon-btn size-9" onClick={() => api.prev()} title="Anterior">
              <I.Prev size={19} />
            </button>
            <button
              class="icon-btn size-11 !opacity-100"
              style={{ background: "var(--panel-strong)" }}
              onClick={() => api.togglePlay()}
              title={playback.playing ? "Pausa" : "Reproducir"}
            >
              <Show when={!playback.loading} fallback={<Spinner />}>
                <Show when={playback.playing} fallback={<I.Play size={20} />}>
                  <I.Pause size={20} />
                </Show>
              </Show>
            </button>
            <button class="icon-btn size-9" onClick={() => api.next()} title="Siguiente">
              <I.Next size={19} />
            </button>
            <button
              class="icon-btn size-9"
              classList={{ active: playback.repeat !== "off" }}
              onClick={cycleRepeat}
              title={`Repetir: ${playback.repeat}`}
            >
              <Show when={playback.repeat === "one"} fallback={<I.Repeat size={17} />}>
                <I.RepeatOne size={17} />
              </Show>
            </button>
          </div>

          {/* Volumen */}
          <div
            class="flex items-center justify-end gap-2"
            onMouseEnter={() => setVolumeOpen(true)}
            onMouseLeave={() => setVolumeOpen(false)}
          >
            <div
              class="overflow-hidden transition-all duration-300"
              style={{ width: volumeOpen() ? "92px" : "0px" }}
            >
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                value={playback.volume}
                onInput={(e) => api.setVolume(Number(e.currentTarget.value))}
                class="w-full accent-[var(--accent)]"
                aria-label="Volumen"
              />
            </div>
            <button
              class="icon-btn size-9"
              onClick={() => api.setVolume(playback.volume > 0 ? 0 : 1)}
              title="Volumen"
            >
              <Show when={playback.volume > 0} fallback={<I.VolumeMute size={18} />}>
                <I.Volume size={18} />
              </Show>
            </button>
          </div>
        </div>
      </div>
    </footer>
  );
}

function Spinner() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="2" opacity="0.25" />
      <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" stroke-width="2" stroke-linecap="round">
        <animateTransform
          attributeName="transform"
          type="rotate"
          from="0 12 12"
          to="360 12 12"
          dur="0.8s"
          repeatCount="indefinite"
        />
      </path>
    </svg>
  );
}
