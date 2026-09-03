import { Show, createSignal } from "solid-js";
import { api, fmtTime, thumbAt } from "../lib/api";
import { playback, position, isFavorite, toggleFavorite, playerViewOpen, setPlayerViewOpen } from "../lib/store";
import * as I from "./Icons";

/** Barra de reproducción inferior de extremo a extremo estilo glassy-music (7.webp / 9.mp4). */
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
    <footer class="player-bar-container relative h-[72px] px-5 flex items-center justify-between z-30 select-none">
      {/* Barra de progreso interactiva continua a todo lo ancho en el borde superior */}
      <div
        class="absolute -top-[3px] inset-x-0 h-3 flex items-center cursor-pointer group z-40"
        ref={scrubBarRef}
        onMouseDown={handleMouseDown}
        onClick={(e) => scrubTo(e.clientX, true)}
      >
        <div class="scrub w-full relative h-[3px] group-hover:h-[5px] transition-all bg-white/15">
          <div class="scrub-buffered" style={{ width: `${playback.buffered * 100}%` }} />
          <div class="scrub-played" style={{ width: `${pct()}%` }} />
          {/* El sitio y el tamaño los pone `.scrub-knob`. Repetirlos aquí con
              utilidades hacía que el desplazamiento se aplicara DOS veces —
              `translate` de Tailwind y `transform` del CSS se suman — y la
              bolita salía una altura por encima de la barra. */}
          <div class="scrub-knob" style={{ left: `${pct()}%` }} />
        </div>
      </div>

      {/* 1. Izquierda: Controles de transporte + Tiempo exacto 0:05 / 3:21 */}
      <div class="flex items-center gap-2 min-w-[240px]">
        <button
          class="icon-btn size-9 text-white/80 hover:text-white"
          onClick={() => api.prev()}
          title="Previous track"
        >
          <I.Prev size={19} />
        </button>
        <button
          class="icon-btn size-11 !opacity-100 rounded-full text-white hover:bg-white/10 active:scale-95 transition-all"
          onClick={() => api.togglePlay()}
          title={playback.playing ? "Pausar" : "Reproducir"}
        >
          <Show when={!playback.loading} fallback={<Spinner />}>
            <Show when={playback.playing} fallback={<I.Play size={26} class="translate-x-[1px]" />}>
              <I.Pause size={26} />
            </Show>
          </Show>
        </button>
        <button
          class="icon-btn size-9 text-white/80 hover:text-white"
          onClick={() => api.next()}
          title="Next track"
        >
          <I.Next size={19} />
        </button>

        {/* Contador de tiempo 0:05 / 3:21 */}
        <span class="ml-3 text-xs font-semibold tabular-nums text-white/75">
          {fmtTime(shown())} <span class="text-white/35 font-normal">/</span> {fmtTime(playback.durationMs)}
        </span>
      </div>

      {/* 2. Centro: Carátula + Título + Artista + Thumbs & Más opciones */}
      <div class="flex items-center gap-3.5 max-w-[500px]">
        <Show
          when={playback.track?.thumbnail}
          fallback={<div class="size-11 shrink-0 rounded-lg bg-white/10 cursor-pointer" onClick={() => setPlayerViewOpen(!playerViewOpen())} />}
        >
          <img
            src={thumbAt(playback.track!.thumbnail, 96)!}
            alt=""
            class="size-11 shrink-0 rounded-lg object-cover ring-1 ring-white/15 shadow-sm cursor-pointer hover:opacity-85 transition-opacity"
            onClick={() => setPlayerViewOpen(!playerViewOpen())}
          />
        </Show>
        <div class="min-w-0 max-w-[280px]">
          <div
            class="truncate text-[13.5px] font-bold text-white leading-snug cursor-pointer hover:underline"
            onClick={() => setPlayerViewOpen(!playerViewOpen())}
          >
            {playback.track?.title ?? "Nada reproduciéndose"}
          </div>
          <div class="truncate text-[11.5px] text-white/55 font-medium mt-0.5">
            {playback.track?.author ?? ""}
          </div>
        </div>

        <Show when={playback.track}>
          <div class="flex items-center gap-0.5 ml-1">
            <button
              class="icon-btn size-8 text-white/60 hover:text-white"
              title="No me gusta"
            >
              <I.ThumbsDown size={16} />
            </button>
            <button
              class="icon-btn size-8"
              classList={{
                "text-[var(--accent)] !opacity-100": isFavorite(),
                "text-white/60 hover:text-white": !isFavorite(),
              }}
              onClick={() => toggleFavorite()}
              title={isFavorite() ? "Quitar de me gusta" : "Me gusta"}
            >
              <I.ThumbsUp size={16} />
            </button>
            <button
              class="icon-btn size-8 text-white/60 hover:text-white"
              title="Más acciones"
            >
              <I.More size={16} />
            </button>
          </div>
        </Show>
      </div>

      {/* 3. Derecha: Volumen + Repetir + Aleatorio + Expandir/Cerrar Reproductor */}
      <div class="flex items-center justify-end gap-1.5 min-w-[240px]">
        {/* Volumen con slider emergente */}
        <div
          class="flex items-center gap-2 mr-2"
          onMouseEnter={() => setVolumeOpen(true)}
          onMouseLeave={() => setVolumeOpen(false)}
        >
          <button
            class="icon-btn size-8 text-white/70 hover:text-white"
            onClick={() => api.setVolume(playback.volume > 0 ? 0 : 1)}
            title="Volumen"
          >
            <Show when={playback.volume > 0} fallback={<I.VolumeMute size={18} />}>
              <I.Volume size={18} />
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
              class="w-full accent-white h-1 cursor-pointer"
              aria-label="Volumen"
            />
          </div>
        </div>

        {/* Repetir */}
        <button
          class="icon-btn size-8"
          classList={{
            "text-[var(--accent)] !opacity-100": playback.repeat !== "off",
            "text-white/70 hover:text-white": playback.repeat === "off",
          }}
          onClick={cycleRepeat}
          title={`Repetir: ${playback.repeat}`}
        >
          <Show when={playback.repeat === "one"} fallback={<I.Repeat size={17} />}>
            <I.RepeatOne size={17} />
          </Show>
        </button>

        {/* Aleatorio */}
        <button
          class="icon-btn size-8"
          classList={{
            "text-[var(--accent)] !opacity-100": playback.shuffle,
            "text-white/70 hover:text-white": !playback.shuffle,
          }}
          onClick={() => api.setShuffle(!playback.shuffle)}
          title="Aleatorio"
        >
          <I.Shuffle size={17} />
        </button>

        {/* Chevron para alternar vista del reproductor (Image 2 vs 3) */}
        <button
          class="icon-btn size-8 ml-1 text-white/70 hover:text-white"
          onClick={() => setPlayerViewOpen(!playerViewOpen())}
          title={playerViewOpen() ? "Cerrar reproductor" : "Abrir reproductor"}
        >
          <I.ChevronUp
            size={18}
            class={`transition-transform duration-300 ${playerViewOpen() ? "rotate-180" : ""}`}
          />
        </button>
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
