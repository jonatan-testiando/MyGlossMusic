import { Show, createEffect, onMount } from "solid-js";
import { TitleBar, Sidebar, HomeFeed, SearchView, LibraryView, Diagnostics } from "./components/Views";
import { PlayerBar } from "./components/PlayerBar";
import { SidePanel } from "./components/SidePanel";
import { FullScreenLyrics } from "./components/FullScreenLyrics";
import { initStore, palette, playback, view, fullLyricsOpen, coverUrl } from "./lib/store";
import { thumbAt } from "./lib/api";
import "./styles.css";

export default function App() {
  onMount(initStore);

  // La paleta que calcula Rust se vuelca a variables CSS
  createEffect(() => {
    const p = palette();
    const root = document.documentElement.style;
    root.setProperty("--bg", p.background);
    root.setProperty("--bg-alt", p.backgroundAlt);
    root.setProperty("--accent", p.accent);
    root.setProperty("--fg", p.foreground);
  });

  const bg = () => thumbAt(playback.track?.thumbnail, 120);

  return (
    <div class="app-shell flex flex-col h-screen overflow-hidden select-none">
      {/* Capas de fondo difuso y viñeta ambiental cinematográfica */}
      <Show when={bg()}>
        <div class="bg-layer visible" style={{ "background-image": `url(${bg()})` }} />
      </Show>
      <div class="bg-scrim" />

      {/* Cabecera superior YouTube Music */}
      <TitleBar />

      {/* Contenedor central de navegación y área principal */}
      <div class="flex flex-1 min-h-0 overflow-hidden">
        {/* Barra lateral de navegación sin marco */}
        <Sidebar />

        {/* Área principal fluida y abierta */}
        <main class="flex-1 min-w-0 h-full overflow-hidden flex">
          <Show when={view() === "home"}>
            <Show
              when={playback.track}
              fallback={<HomeFeed />}
            >
              {/* Vista 2 Columnas idéntica a 7.webp / 9.mp4 */}
              <div class="flex-1 flex flex-col items-center justify-center p-8 min-w-0">
                {/* Selector Song / Video */}
                <div class="flex items-center gap-1 rounded-full bg-black/40 backdrop-blur-md p-1 border border-white/10 shadow-lg mb-6">
                  <button class="rounded-full px-5 py-1 text-xs font-bold bg-white/20 text-white shadow-sm">
                    Song
                  </button>
                  <button class="rounded-full px-5 py-1 text-xs font-semibold text-white/50 hover:text-white/80 transition-colors">
                    Video
                  </button>
                </div>

                {/* Carátula grande (max 480px) con esquinas redondeadas 22px y sombra profunda */}
                <div class="relative max-w-[480px] w-full aspect-square">
                  <img
                    src={coverUrl()!}
                    alt=""
                    class="w-full h-full rounded-[22px] object-cover shadow-[0_30px_70px_-15px_rgba(0,0,0,0.85)] ring-1 ring-white/15 transition-transform duration-500"
                    classList={{ "scale-[0.98] opacity-90": !playback.playing }}
                  />
                </div>
              </div>

              {/* Columna Derecha: Tarjeta de Cristal UP NEXT / LYRICS */}
              <div class="h-full py-4 pr-6">
                <SidePanel />
              </div>
            </Show>
          </Show>

          <Show when={view() === "search"}>
            <div class="flex-1 h-full overflow-hidden">
              <SearchView />
            </div>
          </Show>

          <Show when={view() === "library"}>
            <div class="flex-1 h-full overflow-hidden p-6">
              <LibraryView />
            </div>
          </Show>

          <Show when={view() === "diagnostics"}>
            <div class="flex-1 h-full overflow-hidden p-6">
              <Diagnostics />
            </div>
          </Show>
        </main>
      </div>

      {/* Barra de reproducción de extremo a extremo en la parte inferior */}
      <PlayerBar />

      {/* Modal de letras a pantalla completa */}
      <Show when={fullLyricsOpen()}>
        <FullScreenLyrics />
      </Show>
    </div>
  );
}
