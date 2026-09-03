import { Show, createEffect, onMount } from "solid-js";
import { TitleBar, Sidebar, NowPlaying, SearchView, LibraryView, Diagnostics } from "./components/Views";
import { PlayerBar } from "./components/PlayerBar";
import { SidePanel } from "./components/SidePanel";
import { initStore, palette, playback, view } from "./lib/store";
import { thumbAt } from "./lib/api";
import "./styles.css";

export default function App() {
  onMount(initStore);

  // La paleta que calcula Rust se vuelca a variables CSS. Toda la interfaz se
  // tinta sola porque cada color de `styles.css` sale de estas variables.
  createEffect(() => {
    const p = palette();
    const root = document.documentElement.style;
    root.setProperty("--bg", p.background);
    root.setProperty("--bg-alt", p.backgroundAlt);
    root.setProperty("--accent", p.accent);
    root.setProperty("--fg", p.foreground);
  });

  // Portada minuscula estirada a toda la ventana: el reescalado del navegador
  // hace de desenfoque y no cuesta GPU.
  const bg = () => thumbAt(playback.track?.thumbnail, 48);

  return (
    <div class="app-shell flex flex-col">
      <Show when={bg()}>
        <div class="bg-layer visible" style={{ "background-image": `url(${bg()})` }} />
      </Show>
      <div class="bg-scrim" />

      <TitleBar />

      <main class="flex min-h-0 flex-1 gap-2 pl-1 pr-4">
        <Sidebar />
        <section class="panel min-w-0 flex-1 overflow-hidden">
          <Show when={view() === "home"}>
            <NowPlaying />
          </Show>
          <Show when={view() === "search"}>
            <SearchView />
          </Show>
          <Show when={view() === "library"}>
            <LibraryView />
          </Show>
          <Show when={view() === "diagnostics"}>
            <Diagnostics />
          </Show>
        </section>
        <SidePanel />
      </main>

      <PlayerBar />
    </div>
  );
}
