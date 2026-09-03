import { Show, createEffect, onMount } from "solid-js";
import { TitleBar, Sidebar, HomeFeed, SearchView, LibraryView, Diagnostics } from "./components/Views";
import { PlayerBar } from "./components/PlayerBar";
import { SidePanel } from "./components/SidePanel";
import { FullScreenLyrics } from "./components/FullScreenLyrics";
import { Ambient } from "./components/Ambient";
import {
  initStore,
  palette,
  playback,
  view,
  fullLyricsOpen,
  coverUrl,
  coverFallbackUrl,
  playerViewOpen,
  setPlayerViewOpen,
} from "./lib/store";
import * as I from "./components/Icons";
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

  return (
    <div class="app-shell flex flex-col h-screen overflow-hidden select-none">
      {/* Iluminación ambiental con los colores de la portada */}
      <Ambient />
      <div class="bg-scrim" />

      {/* Cabecera superior YouTube Music */}
      <TitleBar />

      {/* Contenedor central de navegación y área principal */}
      <div class="relative flex flex-1 min-h-0 overflow-hidden">
        {/* Barra lateral de navegación sin marco */}
        <Sidebar />

        {/* Área principal fluida */}
        <main class="flex-1 min-w-0 h-full overflow-hidden flex relative">
          {/*
            1. Vista de reproducción.

            Carátula y panel son UN bloque centrado, no dos columnas pegadas a
            los bordes. Con la ventana maximizada, lo segundo dejaba ~900 px
            muertos entre una y otro. Las proporciones salen de la referencia:
            el panel es más ancho que la carátula, no al revés.
          */}
          <Show when={playerViewOpen() && playback.track}>
            <div class="flex flex-1 min-w-0 items-center justify-center gap-12 px-8 py-16">
              {/*
                No hay marco: el marco ES la imagen. Con `max-w`/`max-h` y sin
                ancho fijo, la portada se pinta con SU proporción — cuadrada la
                de álbum, 16:9 la de vídeo — y encaja dentro del hueco. Un marco
                de proporción fija obligaba a elegir entre recortar o dejar
                barras, y ninguna de las dos es lo que se ve en la referencia.
              */}
              <div class="flex h-full w-[min(44vw,780px)] shrink-0 items-center justify-center">
                <img
                  src={coverUrl()!}
                  alt=""
                  onError={(e) => {
                    // `maxresdefault` no existe para todos los vídeos.
                    const alt = coverFallbackUrl();
                    if (alt && e.currentTarget.src !== alt) e.currentTarget.src = alt;
                  }}
                  class="max-h-full max-w-full rounded-2xl shadow-[0_25px_60px_-15px_rgba(0,0,0,0.85)] transition-transform duration-500"
                  classList={{ "scale-[0.98] opacity-90": !playback.playing }}
                />
              </div>

              <SidePanel />
            </div>
          </Show>

          {/* 2. Vista de Exploración / Navegación (Image 3) */}
          <Show when={!playerViewOpen() || !playback.track}>
            <Show when={view() === "home"}>
              <HomeFeed />
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
          </Show>

          {/* Miniplayer flotante en la esquina inferior derecha (Image 3) */}
          <Show when={!playerViewOpen() && playback.track}>
            <div
              class="absolute bottom-6 right-8 z-30 w-64 aspect-video rounded-2xl overflow-hidden ring-1 ring-white/20 shadow-[0_20px_45px_rgba(0,0,0,0.8)] cursor-pointer group bg-black/60 backdrop-blur-md transition-all duration-300 hover:scale-[1.03] hover:ring-white/40"
              onClick={() => setPlayerViewOpen(true)}
              title="Volver a la canción"
            >
              <img
                src={coverUrl()!}
                alt=""
                class="size-full object-cover"
              />
              <div class="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-2 text-white font-medium text-xs">
                <I.Maximize size={18} />
                <span>Ampliar</span>
              </div>
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
