import { For, Show, createEffect, createSignal, on, onCleanup, onMount } from "solid-js";
import {
  api,
  thumbAt,
  type BrowsePage,
  type ClientHealth,
  type ExtractorStatus,
  type SavedTrack,
  type Shelf as ShelfData,
  type ShelfItem,
} from "../lib/api";
import {
  playback,
  results,
  searching,
  view,
  setView,
  query,
  setQuery,
  runSearch,
  resultsLabel,
  playFromResults,
  playWithRadio,
  playSaved,
  playerViewOpen,
  setPlayerViewOpen,
  sidebarOpen,
  toggleSidebar,
  palette,
} from "../lib/store";
import * as I from "./Icons";

/* ---------------------------------------------------------------- Titlebar */

export function TitleBar() {
  return (
    <header data-tauri-drag-region class="drag-region flex h-14 shrink-0 items-center justify-between px-4 z-20">
      {/* Izquierda: Menú hamburguesa + Logo YouTube Music */}
      <div class="no-drag flex items-center gap-3">
        <button
          class="icon-btn size-9 text-white/80 hover:text-white hover:bg-white/10"
          onClick={toggleSidebar}
          title={sidebarOpen() ? "Contraer menú" : "Expandir menú"}
        >
          <I.Menu size={20} />
        </button>
        <div
          class="flex items-center gap-2 cursor-pointer select-none"
          onClick={() => {
            setView("home");
            setPlayerViewOpen(false);
          }}
          title="Posible Music"
        >
          <div class="size-7 rounded-full bg-[#ff0000] flex items-center justify-center shadow-[0_0_15px_rgba(255,0,0,0.5)]">
            <svg viewBox="0 0 24 24" class="size-4 fill-white translate-x-[1px]">
              <polygon points="6 4 18 12 6 20 6 4" />
            </svg>
          </div>
          <span class="text-[19px] font-bold tracking-tight text-white font-sans">Music</span>
        </div>
      </div>

      {/* Centro: Buscador estilo píldora ancha */}
      <SearchBox />

      {/* Derecha: Flechas navegación + Avatar J + Controles de ventana de Windows */}
      <div class="no-drag flex items-center gap-3.5">
        <div class="flex items-center gap-1 text-white/60">
          <button
            class="icon-btn size-7 rounded-full hover:bg-white/10 hover:text-white"
            onClick={() => {
              setView("home");
              setPlayerViewOpen(false);
            }}
            title="Atrás"
          >
            <I.ChevronLeft size={16} />
          </button>
          <button
            class="icon-btn size-7 rounded-full hover:bg-white/10 hover:text-white"
            onClick={() => {
              if (playback.track) setPlayerViewOpen(true);
            }}
            title="Adelante"
          >
            <I.ChevronRight size={16} />
          </button>
        </div>

        <div class="size-8 rounded-full bg-purple-600 ring-2 ring-white/20 flex items-center justify-center text-xs font-bold text-white shadow-md cursor-pointer hover:scale-105 transition-transform">
          J
        </div>

        <div class="flex items-center gap-0.5">
          <button class="icon-btn size-8 text-white/70 hover:text-white" onClick={() => api.minimize()} title="Minimizar">
            <I.Minimize size={14} />
          </button>
          <button class="icon-btn size-8 text-white/70 hover:text-white" onClick={() => api.toggleMaximize()} title="Maximizar">
            <I.Maximize size={12} />
          </button>
          <button class="icon-btn size-8 text-white/70 hover:!bg-red-600 hover:text-white" onClick={() => api.close()} title="Cerrar">
            <I.Close size={15} />
          </button>
        </div>
      </div>
    </header>
  );
}

function SearchBox() {
  let input!: HTMLInputElement;
  const [sugerencias, setSugerencias] = createSignal<string[]>([]);
  const [abierto, setAbierto] = createSignal(false);
  // -1 = nada resaltado; entonces Intro busca lo que hay escrito.
  const [resaltada, setResaltada] = createSignal(-1);

  /**
   * Freno y guardia de carrera.
   *
   * El freno evita una petición por tecla. La guardia hace falta igualmente:
   * dos peticiones en vuelo pueden volver en cualquier orden, y sin ella la
   * respuesta de "kast" podría pisar a la de "kastra" y el desplegable se
   * quedaría mostrando sugerencias de algo que ya no está escrito.
   */
  let temporizador: number | undefined;
  let ultimaPedida = "";

  const pedirSugerencias = (texto: string) => {
    clearTimeout(temporizador);
    const limpio = texto.trim();
    if (limpio.length < 2) {
      setSugerencias([]);
      setAbierto(false);
      return;
    }
    temporizador = window.setTimeout(async () => {
      ultimaPedida = limpio;
      try {
        const res = await api.searchSuggestions(limpio);
        if (ultimaPedida !== limpio) return;
        setSugerencias(res);
        setResaltada(-1);
        setAbierto(res.length > 0);
      } catch {
        if (ultimaPedida === limpio) setAbierto(false);
      }
    }, 180);
  };

  const buscar = (texto: string) => {
    clearTimeout(temporizador);
    setAbierto(false);
    setQuery(texto);
    runSearch(texto);
    setPlayerViewOpen(false);
    input.blur();
  };

  const enTecla = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      setAbierto(false);
      return;
    }
    if (!abierto() || sugerencias().length === 0) return;

    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const paso = e.key === "ArrowDown" ? 1 : -1;
      const total = sugerencias().length;
      // Se recorre incluyendo el -1, para poder volver a lo escrito a mano.
      setResaltada((i) => {
        const siguiente = i + paso;
        if (siguiente < -1) return total - 1;
        if (siguiente >= total) return -1;
        return siguiente;
      });
    } else if (e.key === "Enter" && resaltada() >= 0) {
      e.preventDefault();
      buscar(sugerencias()[resaltada()]);
    }
  };

  return (
    <div class="no-drag relative mx-auto w-full max-w-[540px] px-4">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          buscar(query());
        }}
      >
        <div
          class="flex items-center gap-3 border border-white/5 bg-white/[0.08] px-4 py-2 transition-all hover:bg-white/[0.12] focus-within:bg-white/[0.16] focus-within:ring-1 focus-within:ring-white/25"
          classList={{ "rounded-full": !abierto(), "rounded-t-2xl": abierto() }}
        >
          <I.Search size={16} class="shrink-0 text-white/50" />
          <input
            ref={input}
            value={query()}
            onInput={(e) => {
              setQuery(e.currentTarget.value);
              pedirSugerencias(e.currentTarget.value);
            }}
            onFocus={() => setAbierto(sugerencias().length > 0)}
            // Con un cierre inmediato, el clic en una sugerencia nunca llega:
            // el desplegable desaparece antes de que se registre.
            onBlur={() => setTimeout(() => setAbierto(false), 120)}
            onKeyDown={enTecla}
            placeholder="Buscar canciones, álbumes, artistas o podcasts"
            class="w-full bg-transparent text-sm text-white outline-none placeholder:text-white/40"
            autocomplete="off"
            spellcheck={false}
          />
          <Show when={query()}>
            <button
              type="button"
              class="icon-btn size-6 shrink-0 text-white/50 hover:text-white"
              onClick={() => {
                setQuery("");
                setSugerencias([]);
                setAbierto(false);
                input.focus();
              }}
              title="Borrar"
            >
              <I.Close size={13} />
            </button>
          </Show>
        </div>
      </form>

      <Show when={abierto()}>
        <ul class="glass-card absolute inset-x-4 top-full z-50 max-h-[60vh] overflow-y-auto rounded-b-2xl rounded-t-none border-t-0 py-1 shadow-2xl">
          <For each={sugerencias()}>
            {(s, i) => (
              <li>
                <button
                  type="button"
                  class="flex w-full items-center gap-3 px-4 py-2 text-left text-sm text-white/85 transition-colors"
                  classList={{ "bg-white/15 text-white": resaltada() === i() }}
                  // `mousedown` y no `click`: en `click` el `blur` del campo ya
                  // ha cerrado el desplegable y el botón ya no existe.
                  onMouseDown={(e) => {
                    e.preventDefault();
                    buscar(s);
                  }}
                  onMouseEnter={() => setResaltada(i())}
                >
                  <I.Search size={14} class="shrink-0 text-white/35" />
                  <span class="truncate">{s}</span>
                </button>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
}

/* ----------------------------------------------------------------- Sidebar */

export function Sidebar() {
  const items = [
    { id: "home" as const, label: "Principal", icon: I.Home },
    { id: "explore" as const, label: "Explorar", icon: I.Compass },
    { id: "library" as const, label: "Biblioteca", icon: I.Library },
  ];

  // El raíl es una preferencia del usuario, no una consecuencia de la vista:
  // la barra tiene que servir para navegar esté donde esté.
  const collapsed = () => !sidebarOpen();

  const go = (id: (typeof items)[number]["id"]) => {
    setView(id === "explore" ? "search" : id);
    setPlayerViewOpen(false);
  };

  // Mientras se mira la canción no hay ninguna pestaña activa: se está en el
  // reproductor, no navegando.
  const isActive = (id: (typeof items)[number]["id"]) =>
    !playerViewOpen() && view() === (id === "explore" ? "search" : id);

  return (
    <nav
      class="flex shrink-0 flex-col gap-1 py-3 z-10 select-none transition-[width] duration-300 ease-out"
      classList={{ "w-[72px] px-2 items-center": collapsed(), "w-[215px] pl-3 pr-2": !collapsed() }}
    >
      <div class="w-full space-y-1">
        <For each={items}>
          {(item) => (
            <button
              class="flex w-full rounded-xl transition-all"
              classList={{
                "flex-col items-center gap-1 px-1 py-2.5": collapsed(),
                "items-center gap-4 px-3.5 py-2.5 text-left text-[13.5px] font-medium": !collapsed(),
                "bg-white/15 text-white shadow-sm font-semibold": isActive(item.id),
                "text-white/70 hover:text-white hover:bg-white/10": !isActive(item.id),
              }}
              onClick={() => go(item.id)}
              title={item.label}
            >
              <item.icon size={19} class={isActive(item.id) ? "text-white" : "text-white/70"} />
              <span classList={{ "text-[10px] font-medium leading-none": collapsed() }}>
                {item.label}
              </span>
            </button>
          )}
        </For>
      </div>

      {/* Todo lo que sigue solo cabe con la barra abierta. */}
      <Show when={!collapsed()}>
        <div class="pt-4 pb-2 px-1">
          <button
            class="flex w-full items-center justify-center gap-2 rounded-full bg-white/10 hover:bg-white/15 active:scale-95 text-xs font-semibold py-2 px-3 text-white transition-all border border-white/10 shadow-sm"
            onClick={() => {
              setView("library");
              setPlayerViewOpen(false);
            }}
          >
            <I.Plus size={15} />
            Nueva playlist
          </button>
        </div>

        <div class="my-2 h-[1px] bg-white/10 mx-2" />

        <div class="scroll-area flex-1 px-1 space-y-1 overflow-y-auto">
          {/* La única playlist que existe de verdad hoy. Las demás llegan
              cuando haya playlists locales; poner nombres de ejemplo aquí
              solo hace que la app mienta sobre lo que tiene. */}
          <button
            class="flex w-full flex-col rounded-xl px-2.5 py-2 text-left transition-colors hover:bg-white/10"
            onClick={() => {
              setView("library");
              setPlayerViewOpen(false);
            }}
          >
            <div class="flex items-center gap-1.5 text-xs font-semibold text-white">
              <I.Pin size={12} class="text-[var(--accent)] shrink-0" />
              <span>Música que te gustó</span>
            </div>
            <span class="text-[10.5px] text-white/45 pl-4">Playlist autogenerada</span>
          </button>
        </div>

        <div class="mt-auto space-y-1 px-1 pb-1">
          <button
            class="flex w-full items-center gap-1.5 rounded-xl px-2.5 py-2 text-left text-xs font-medium text-white/60 transition-colors hover:bg-white/10 hover:text-white/90"
            onClick={() => {
              setView("diagnostics");
              setPlayerViewOpen(false);
            }}
          >
            <I.Stethoscope size={13} class="shrink-0 text-white/40" />
            <span>Diagnóstico</span>
          </button>
          <div class="px-2.5 text-[11px] leading-relaxed text-white/30">Modo anónimo</div>
        </div>
      </Show>
    </nav>
  );
}

/* -------------------------------------------------------------- Home Feed */

/**
 * Una fila del inicio: título y tarjetas en horizontal.
 *
 * Es la forma que tiene YouTube Music de presentarlo todo — carruseles de
 * tarjetas — y la que usa la referencia.
 */
function Shelf(p: {
  title: string;
  subtitle?: string;
  items: ShelfItem[];
  onPick: (item: ShelfItem, index: number) => void;
}) {
  let carril!: HTMLDivElement;
  // Botones muertos, no. Si las tarjetas caben en pantalla no hay nada que
  // desplazar, y una flecha que no hace nada es peor que no tenerla.
  const [desbordado, setDesbordado] = createSignal(false);

  const revisar = () => setDesbordado(carril.scrollWidth > carril.clientWidth + 4);
  onMount(() => {
    revisar();
    const obs = new ResizeObserver(revisar);
    obs.observe(carril);
    onCleanup(() => obs.disconnect());
  });

  const desplazar = (dir: number) =>
    carril.scrollBy({ left: dir * carril.clientWidth * 0.8, behavior: "smooth" });

  return (
    <section>
      <div class="mb-3 flex items-end justify-between gap-3">
        <div class="min-w-0">
          <Show when={p.subtitle}>
            <div class="text-[10.5px] font-bold uppercase tracking-widest text-white/40">
              {p.subtitle}
            </div>
          </Show>
          <h2 class="truncate text-2xl font-bold tracking-tight text-white">{p.title}</h2>
        </div>
        <Show when={desbordado()}>
          <div class="flex shrink-0 items-center gap-1">
            <button
              class="icon-btn size-8 rounded-full hover:bg-white/10"
              onClick={() => desplazar(-1)}
              title="Anterior"
            >
              <I.ChevronLeft size={17} />
            </button>
            <button
              class="icon-btn size-8 rounded-full hover:bg-white/10"
              onClick={() => desplazar(1)}
              title="Siguiente"
            >
              <I.ChevronRight size={17} />
            </button>
          </div>
        </Show>
      </div>

      <div ref={carril} class="scrollbar-none flex gap-4 overflow-x-auto pb-1">
        <For each={p.items}>
          {(item, i) => (
            <button
              class="group flex w-[168px] shrink-0 flex-col text-left"
              onClick={() => p.onPick(item, i())}
              title={item.title}
            >
              <div class="relative aspect-square w-full overflow-hidden rounded-xl shadow-lg ring-1 ring-white/10 transition-all group-hover:shadow-2xl">
                <Show when={item.thumbnail} fallback={<div class="size-full bg-white/8" />}>
                  <img
                    src={thumbAt(item.thumbnail, 320)!}
                    alt=""
                    class="size-full object-cover transition-transform duration-300 group-hover:scale-105"
                  />
                </Show>
                {/* Como en la referencia: el botón aparece abajo a la derecha,
                    no tapando la portada entera. */}
                <div class="absolute inset-0 bg-black/25 opacity-0 transition-opacity group-hover:opacity-100" />
                <div class="absolute bottom-2 right-2 grid size-9 place-items-center rounded-full bg-black/70 text-white opacity-0 shadow-xl backdrop-blur-sm transition-opacity group-hover:opacity-100">
                  <I.Play size={16} class="translate-x-[1px]" />
                </div>
              </div>
              <div class="mt-2.5 line-clamp-2 text-[13px] font-bold leading-snug text-white">
                {item.title}
              </div>
              <div class="mt-0.5 truncate text-[11.5px] font-medium text-white/50">
                {item.subtitle}
              </div>
            </button>
          )}
        </For>
      </div>
    </section>
  );
}

export function HomeFeed() {
  const [historial, setHistorial] = createSignal<SavedTrack[]>([]);
  const [estantes, setEstantes] = createSignal<ShelfData[]>([]);
  const [mix, setMix] = createSignal<{ semilla: SavedTrack; items: ShelfItem[] } | null>(null);
  const [cargando, setCargando] = createSignal(true);

  onMount(async () => {
    // Las tres fuentes van en paralelo: el historial es instantáneo (SQLite),
    // el feed y la radio tardan. Encadenarlas dejaría la pantalla en blanco
    // hasta la más lenta.
    const hist = api.history().catch(() => [] as SavedTrack[]);
    const feed = api
      .home()
      .catch(
        (): BrowsePage => ({ title: null, subtitle: null, thumbnail: null, shelves: [] }),
      );

    const h = await hist;
    setHistorial(h);

    // La radio de lo último escuchado es lo que sustituye a las
    // recomendaciones personalizadas de Google, que exigen cuenta.
    const semilla = h[0];
    if (semilla) {
      api
        .radio(semilla.videoId)
        .then((r) => {
          const items = r.tracks
            .filter((t) => t.videoId !== semilla.videoId)
            .map((t) => ({
              kind: "track" as const,
              id: t.videoId,
              title: t.title,
              subtitle: t.subtitle,
              thumbnail: t.thumbnail,
              duration: t.duration,
            }));
          if (items.length) setMix({ semilla, items });
        })
        .catch(() => {});
    }

    setEstantes((await feed).shelves);
    setCargando(false);
  });

  /** El historial, sin repetir, como tarjetas. */
  const volverAEscuchar = (): ShelfItem[] => {
    const vistos = new Set<string>();
    return historial()
      .filter((t) => vistos.has(t.videoId) ? false : vistos.add(t.videoId))
      .slice(0, 12)
      .map((t) => ({
        kind: "track" as const,
        id: t.videoId,
        title: t.title,
        subtitle: t.author,
        thumbnail: t.thumbnail,
        duration: null,
      }));
  };

  const abrir = (item: ShelfItem) => {
    if (item.kind === "track") {
      playWithRadio({
        videoId: item.id,
        title: item.title,
        subtitle: item.subtitle,
        duration: item.duration,
        thumbnail: item.thumbnail,
      });
      setPlayerViewOpen(true);
    } else if (item.kind === "playlist") {
      // Los ids de playlist de `browse` vienen con prefijo `VL`.
      runSearch(item.id.replace(/^VL/, ""));
    }
    // Artistas y álbumes: pendientes de sus pantallas.
  };

  return (
    <div class="scroll-area h-full flex-1 space-y-9 overflow-y-auto px-8 py-5">
      <Show when={volverAEscuchar().length > 0}>
        <Shelf
          subtitle="De lo tuyo"
          title="Volver a escuchar"
          items={volverAEscuchar()}
          onPick={abrir}
        />
      </Show>

      <Show when={mix()}>
        <Shelf
          subtitle="Porque escuchaste"
          title={`Mix de ${mix()!.semilla.title}`}
          items={mix()!.items}
          onPick={abrir}
        />
      </Show>

      <For each={estantes()}>
        {(e) => <Shelf title={e.title} items={e.items} onPick={abrir} />}
      </For>

      <Show when={!cargando() && estantes().length === 0 && volverAEscuchar().length === 0}>
        <p class="mt-20 text-center text-sm text-white/40">
          Busca algo arriba para empezar. Según vayas escuchando, esta pantalla se
          llena con recomendaciones.
        </p>
      </Show>

      <Show when={cargando()}>
        <div class="flex gap-4">
          <For each={Array(6).fill(0)}>
            {() => (
              <div class="w-[168px] shrink-0">
                <div class="aspect-square w-full animate-pulse rounded-xl bg-white/8" />
                <div class="mt-2.5 h-3 w-4/5 animate-pulse rounded bg-white/8" />
                <div class="mt-1.5 h-2.5 w-2/5 animate-pulse rounded bg-white/6" />
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}

/* ------------------------------------------------------------ Search results */

export function SearchView() {
  const searchChips = ["Songs", "Videos", "Featured playlists", "Albums", "Artists", "Community playlists", "Episodes", "Podcasts"];
  const [activeChip, setActiveChip] = createSignal("Songs");

  return (
    <div class="scroll-area h-full px-8 py-6 space-y-5">
      {/* Selector de chips estilo 6.webp */}
      <div class="flex items-center gap-2 overflow-x-auto pb-1 scrollbar-none">
        <For each={searchChips}>
          {(c) => (
            <button
              class="shrink-0 rounded-full px-3.5 py-1 text-xs font-semibold transition-all border"
              classList={{
                "bg-white/20 border-white/25 text-white shadow-sm": activeChip() === c,
                "bg-white/[0.04] border-white/10 text-white/60 hover:text-white hover:bg-white/10": activeChip() !== c,
              }}
              onClick={() => setActiveChip(c)}
            >
              {c}
            </button>
          )}
        </For>
      </div>

      <Show when={!searching()} fallback={<SkeletonList />}>
        <Show
          when={results().length > 0}
          fallback={
            <p class="mt-16 text-center text-sm text-white/40">
              {query() ? "Sin resultados." : "Escribe algo arriba para buscar."}
            </p>
          }
        >
          <Show when={resultsLabel()}>
            <div class="mb-3 flex items-center justify-between gap-3">
              <h2 class="min-w-0 truncate text-base font-bold text-white">{resultsLabel()}</h2>
              <button class="rounded-full bg-white/10 hover:bg-white/15 px-4 py-1.5 text-xs font-semibold text-white border border-white/10" onClick={() => playFromResults(0)}>
                Reproducir todo
              </button>
            </div>
          </Show>
          <div class="fade-in flex flex-col gap-1">
            <For each={results()}>
              {(r, i) => {
                const active = () => playback.track?.videoId === r.videoId;
                return (
                  <button
                    class="group flex items-center gap-3.5 rounded-xl px-3.5 py-2.5 text-left transition-all border border-transparent"
                    classList={{
                      "bg-white/[0.14] border-white/10 shadow-sm": active(),
                      "hover:bg-white/[0.06] hover:border-white/5": !active(),
                    }}
                    onDblClick={() => playFromResults(i())}
                    onClick={() => playFromResults(i())}
                  >
                    <div class="relative shrink-0 size-11 rounded-lg overflow-hidden ring-1 ring-white/10 shadow-sm">
                      <Show
                        when={r.thumbnail}
                        fallback={<div class="size-full bg-white/8" />}
                      >
                        <img
                          src={thumbAt(r.thumbnail, 96)!}
                          alt=""
                          class="size-full object-cover"
                        />
                      </Show>
                      <div class="absolute inset-0 grid place-items-center bg-black/45 opacity-0 transition-opacity group-hover:opacity-100">
                        <I.Play size={16} class="text-white" />
                      </div>
                    </div>
                    <div class="min-w-0 flex-1">
                      <div
                        class="truncate text-sm font-semibold text-white"
                        classList={{ "text-[var(--accent)]": active() }}
                      >
                        {r.title}
                      </div>
                      <div class="truncate text-xs text-white/60 font-medium mt-0.5">{r.subtitle}</div>
                    </div>
                    <span class="shrink-0 text-xs tabular-nums text-white/50 font-medium">
                      {r.duration ?? ""}
                    </span>
                  </button>
                );
              }}
            </For>
          </div>
        </Show>
      </Show>
    </div>
  );
}

function SkeletonList() {
  return (
    <div class="flex flex-col gap-0.5">
      <For each={Array(8).fill(0)}>
        {(_, i) => (
          <div
            class="flex items-center gap-3 px-3 py-2"
            style={{ opacity: String(1 - i() * 0.1) }}
          >
            <div class="size-11 shrink-0 animate-pulse rounded-lg bg-white/8" />
            <div class="flex-1">
              <div class="h-3 w-2/5 animate-pulse rounded bg-white/8" />
              <div class="mt-2 h-2.5 w-1/4 animate-pulse rounded bg-white/6" />
            </div>
          </div>
        )}
      </For>
    </div>
  );
}

/* ---------------------------------------------------------------- Library */

/** Favoritos e historial, ambos guardados en SQLite local. */
export function LibraryView() {
  const [tab, setTab] = createSignal<"favorites" | "history">("favorites");
  const [rows, setRows] = createSignal<SavedTrack[]>([]);

  const load = async () => {
    try {
      setRows(tab() === "favorites" ? await api.favorites() : await api.history());
    } catch {
      setRows([]);
    }
  };

  createEffect(on(tab, load));

  return (
    <div class="flex h-full flex-col">
      <div class="flex shrink-0 gap-1 px-5 pt-4">
        <For each={[["favorites", "Favoritos"], ["history", "Historial"]] as const}>
          {([id, label]) => (
            <button
              class="rounded-lg px-3.5 py-1.5 text-[12px] font-medium transition-colors"
              classList={{
                "bg-[var(--panel-strong)]": tab() === id,
                "opacity-50 hover:opacity-80": tab() !== id,
              }}
              onClick={() => setTab(id)}
            >
              {label}
            </button>
          )}
        </For>
      </div>

      <div class="scroll-area flex-1 px-4 py-3">
        <Show
          when={rows().length > 0}
          fallback={
            <p class="mt-14 text-center text-[13px] opacity-40">
              {tab() === "favorites"
                ? "Aún no has guardado nada. Usa el corazón del reproductor."
                : "Todavía no has escuchado nada."}
            </p>
          }
        >
          <div class="fade-in flex flex-col gap-0.5">
            <For each={rows()}>
              {(t, i) => (
                <button
                  class="group flex items-center gap-3 rounded-xl px-3 py-2 text-left transition-colors hover:bg-[var(--panel)]"
                  classList={{ "bg-[var(--panel)]": playback.track?.videoId === t.videoId }}
                  onClick={() => playSaved(rows(), i())}
                >
                  <Show when={t.thumbnail} fallback={<div class="size-11 rounded-lg bg-white/8" />}>
                    <img src={thumbAt(t.thumbnail, 96)!} alt="" class="size-11 rounded-lg object-cover" />
                  </Show>
                  <div class="min-w-0 flex-1">
                    <div class="truncate text-[13px]">{t.title}</div>
                    <div class="truncate text-[11.5px] opacity-50">{t.author}</div>
                  </div>
                </button>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  );
}

/**
 * La paleta que se está usando ahora mismo.
 *
 * Es el equivalente de `probe` para la estética: cuando el fondo salga gris o
 * de un color raro, aquí se ve si el problema es la portada, el reparto de
 * colores o la interfaz, sin tener que adivinarlo desde una captura.
 */
function PaletteReport() {
  const p = () => palette();
  const roles = () =>
    [
      ["fondo", p().background],
      ["fondo alt", p().backgroundAlt],
      ["acento", p().accent],
      ["texto", p().foreground],
    ] as const;

  return (
    <div class="panel mb-5 px-4 py-3">
      <div class="mb-2 flex items-baseline justify-between gap-3">
        <h2 class="text-[15px] font-semibold">Paleta de la portada</h2>
        <span class="truncate text-[11px] opacity-45">
          {playback.track?.title ?? "sin pista"}
        </span>
      </div>

      <div class="mb-3 flex flex-wrap gap-2">
        <For each={p().stops}>
          {(stop) => (
            <div class="flex items-center gap-2 rounded-lg bg-white/[0.04] px-2 py-1.5">
              <span
                class="size-6 shrink-0 rounded ring-1 ring-white/15"
                style={{ background: stop.color }}
              />
              <div class="leading-tight">
                <div class="font-mono text-[11px]">{stop.color}</div>
                <div class="text-[10px] opacity-45">{Math.round(stop.weight * 100)}%</div>
              </div>
            </div>
          )}
        </For>
      </div>

      <div class="flex flex-wrap gap-3">
        <For each={roles()}>
          {([nombre, color]) => (
            <div class="flex items-center gap-1.5">
              <span
                class="size-3.5 shrink-0 rounded ring-1 ring-white/15"
                style={{ background: color }}
              />
              <span class="text-[11px] opacity-55">{nombre}</span>
              <span class="font-mono text-[11px] opacity-35">{color}</span>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------- Diagnostics */

/**
 * Equivalente en la app a `ytm-spike probe`.
 *
 * Es la funcionalidad que separa este proyecto de las alternativas: cuando algo
 * deja de sonar, el usuario ve al instante si YouTube cerro un cliente, en vez
 * de quedarse con una app rota y sin explicacion.
 */
export function Diagnostics() {
  const [rows, setRows] = createSignal<ClientHealth[]>([]);
  const [extractor, setExtractor] = createSignal<ExtractorStatus | null>(null);
  const [running, setRunning] = createSignal(false);

  const run = async () => {
    setRunning(true);
    try {
      api.extractorStatus().then(setExtractor).catch(() => setExtractor(null));
      setRows(await api.diagnose());
    } finally {
      setRunning(false);
    }
  };

  onMount(run);

  return (
    <div class="scroll-area h-full px-6 py-4">
      <PaletteReport />

      <div class="mb-1 flex items-center justify-between">
        <h2 class="text-[15px] font-semibold">Estado de los clientes</h2>
        <button class="chip px-3 py-1.5 text-[12px]" onClick={run} disabled={running()}>
          {running() ? "Comprobando…" : "Volver a comprobar"}
        </button>
      </div>
      <p class="mb-4 max-w-lg text-[12px] leading-relaxed opacity-45">
        YouTube cierra clientes cada pocos meses. Si la música deja de sonar, aquí se ve cuál
        sigue en pie.
      </p>

      {/* El extractor real es yt-dlp; los clientes de abajo son el respaldo. */}
      <div class="panel mb-4 flex items-center gap-3 px-4 py-3">
        <span
          class="size-2.5 shrink-0 rounded-full"
          style={{ background: extractor()?.available ? "#4ade80" : "#f87171" }}
        />
        <div class="min-w-0 flex-1">
          <div class="text-[13px] font-semibold">
            Extractor: yt-dlp{" "}
            <Show when={extractor()?.version}>
              <span class="font-normal opacity-60">{extractor()!.version}</span>
            </Show>
          </div>
          <div class="truncate text-[11px] opacity-45">
            <Show
              when={extractor()?.available}
              fallback="No encontrado: la reproducción queda capada a ~48 s por pista."
            >
              {extractor()!.program}
            </Show>
          </div>
        </div>
      </div>

      <Show when={rows().length > 0} fallback={<p class="text-[13px] opacity-40">Comprobando…</p>}>
        <div class="fade-in flex flex-col gap-1">
          <For each={rows()}>
            {(r) => {
              const ok = () => r.status === "OK" && r.directAudio > 0;
              return (
                <div class="panel flex items-center gap-3 px-4 py-2.5">
                  <span
                    class="size-2 shrink-0 rounded-full"
                    style={{ background: ok() ? "#4ade80" : "#f87171" }}
                  />
                  <span class="w-28 shrink-0 font-mono text-[12px]">{r.id}</span>
                  <span class="w-32 shrink-0 text-[12px] opacity-60">{r.status}</span>
                  <span class="flex-1 truncate text-[12px] opacity-45">
                    {r.best ?? (ok() ? "" : "sin audio directo")}
                  </span>
                </div>
              );
            }}
          </For>
        </div>
      </Show>
    </div>
  );
}
