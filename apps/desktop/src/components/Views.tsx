import { For, Show, createEffect, createSignal, on, onCleanup, onMount } from "solid-js";
import { PlaylistMenu } from "./Playlists";
import { BUSCADOR } from "../lib/atajos";
import {
  api,
  EXPLORE,
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
  query,
  setQuery,
  runSearch,
  resultsLabel,
  playFromResults,
  playWithRadio,
  navegar,
  playlists,
  setSettingsOpen,
  refreshPlaylists,
  showPlaylist,
  setCreatingPlaylist,
  browsePage,
  browseCompleting,
  browseLoading,
  browseId,
  browseTitulo,
  openBrowse,
  openExplore,
  saveBrowseAsPlaylist,
  savingBrowse,
  atras,
  adelante,
  puedeAtras,
  puedeAdelante,
  searchChips,
  searchFilter,
  searchMoreToken,
  applySearchFilter,
  loadMoreResults,
  loadingMore,
  playSaved,
  playerViewOpen,
  togglePlayerView,
  sidebarOpen,
  toggleSidebar,
  palette,
} from "../lib/store";
import * as I from "./Icons";
import { TrackMenu } from "./TrackMenu";

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
            navegar({ view: "home" });
          }}
          title="MyGlossMusic"
        >
          <div class="size-7 rounded-full bg-[#ff0000] flex items-center justify-center shadow-[0_0_15px_rgba(255,0,0,0.5)]">
            <svg viewBox="0 0 24 24" class="size-4 fill-white translate-x-[1px]">
              <polygon points="6 4 18 12 6 20 6 4" />
            </svg>
          </div>
          <span class="text-[19px] font-bold tracking-tight text-white font-sans">MyGloss<span class="text-white/70">Music</span></span>
        </div>
      </div>

      {/* Centro: Buscador estilo píldora ancha */}
      <SearchBox />

      {/* Derecha: Flechas navegación + Avatar J + Controles de ventana de Windows */}
      <div class="no-drag flex items-center gap-3.5">
        <button
          class="icon-btn size-8 rounded-full hover:bg-white/10"
          onClick={() => setSettingsOpen(true)}
          title="Ajustes"
        >
          <I.Settings size={17} />
        </button>
        <div class="flex items-center gap-1 text-white/60">
          <button
            class="icon-btn size-7 rounded-full hover:bg-white/10 hover:text-white disabled:cursor-default disabled:opacity-25"
            onClick={atras}
            disabled={!puedeAtras()}
            title="Atrás"
          >
            <I.ChevronLeft size={16} />
          </button>
          <button
            class="icon-btn size-7 rounded-full hover:bg-white/10 hover:text-white disabled:cursor-default disabled:opacity-25"
            onClick={adelante}
            disabled={!puedeAdelante()}
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
    togglePlayerView(false, false);
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
            {...{ [BUSCADOR]: "" }}
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
    if (id === "explore") openExplore();
    else navegar({ view: id });
  };

  // Mientras se mira la canción no hay ninguna pestaña activa: se está en el
  // reproductor, no navegando.
  const isActive = (id: (typeof items)[number]["id"]) =>
    !playerViewOpen() &&
    (id === "explore" ? view() === "browse" && browseId() === EXPLORE : view() === id);

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
            onClick={() => setCreatingPlaylist(true)}
          >
            <I.Plus size={15} />
            Nueva playlist
          </button>
        </div>

        <div class="my-2 h-[1px] bg-white/10 mx-2" />

        <div class="scroll-area flex-1 px-1 space-y-1 overflow-y-auto">
          <button
            class="flex w-full flex-col rounded-xl px-2.5 py-2 text-left transition-colors hover:bg-white/10"
            onClick={() => {
              navegar({ view: "library" });
            }}
          >
            <div class="flex items-center gap-1.5 text-xs font-semibold text-white">
              <I.Pin size={12} class="text-[var(--accent)] shrink-0" />
              <span>Música que te gustó</span>
            </div>
            <span class="text-[10.5px] text-white/45 pl-4">Playlist autogenerada</span>
          </button>

          <For each={playlists()}>
            {(l) => (
              <div class="group flex items-center rounded-xl pr-1 transition-colors hover:bg-white/10">
                <button
                  class="flex min-w-0 flex-1 flex-col px-2.5 py-2 text-left"
                  onClick={() => showPlaylist(l)}
                  title={l.name}
                >
                  <span class="truncate text-xs font-medium text-white/85">{l.name}</span>
                  <span class="text-[10.5px] text-white/45">
                    {l.count} {l.count === 1 ? "canción" : "canciones"}
                  </span>
                </button>
                <PlaylistMenu lista={l} class="shrink-0 opacity-0 group-hover:opacity-100" />
              </div>
            )}
          </For>
        </div>

        <div class="mt-auto space-y-1 px-1 pb-1">
          <button
            class="flex w-full items-center gap-1.5 rounded-xl px-2.5 py-2 text-left text-xs font-medium text-white/60 transition-colors hover:bg-white/10 hover:text-white/90"
            onClick={() => {
              navegar({ view: "diagnostics" });
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

/** Página de `browse` vacía, para cuando el feed no llega. */
const VACIO: BrowsePage = {
  title: null,
  subtitle: null,
  secondSubtitle: null,
  description: null,
  thumbnail: null,
  shelves: [],
  buttons: [],
  continuation: null,
};

/**
 * Llama a algo que puede fallar y devuelve un valor de repuesto.
 *
 * Envuelve la llamada entera, no solo la promesa: un comando que todavía no
 * existe en el backend lanza de forma síncrona, y eso se escapa de un
 * `.catch()`.
 */
function seguro<T>(fn: () => Promise<T>, alternativa: T): Promise<T> {
  try {
    return fn().catch(() => alternativa);
  } catch {
    return Promise.resolve(alternativa);
  }
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
    //
    // `seguro` y no un `.catch()` a secas porque `.catch()` solo atrapa
    // promesas rechazadas: si la llamada falla ANTES de devolver uña — el
    // comando no existe, la interfaz va por delante del backend — el error es
    // síncrono, se escapa del `onMount` y se lleva por delante la pantalla.
    const hist = seguro(() => api.history(), [] as SavedTrack[]);
    const feed = seguro(() => api.home(), VACIO);

    const h = await hist;
    setHistorial(h);

    // La radio de lo último escuchado es lo que sustituye a las
    // recomendaciones personalizadas de Google, que exigen cuenta.
    const semilla = h[0];
    if (semilla) {
      seguro(() => api.radio(semilla.videoId), { playlistId: null, tracks: [], artistBrowseId: null })
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
        });
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
      togglePlayerView(true, false);
    } else {
      openBrowse(item.id, item.title);
    }
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

function SkeletonList() {
  return (
    <div class="flex flex-col gap-0.5">
      <For each={Array(8).fill(0)}>
        {(_, i) => (
          <div class="flex items-center gap-3 px-3 py-2" style={{ opacity: String(1 - i() * 0.1) }}>
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

/** Cuánto antes del final se empieza a pedir la página siguiente, en píxeles. */
const MARGEN_CARGA = 600;

/** Etiqueta legible del tipo de un resultado. */
const TIPO: Record<ShelfItem["kind"], string> = {
  track: "Canción",
  album: "Álbum",
  artist: "Artista",
  playlist: "Lista",
};

export function SearchView() {
  let fondo!: HTMLDivElement;

  /** ¿El final de la lista está a la vista (o casi)? */
  const finalALaVista = () =>
    fondo.getBoundingClientRect().top < window.innerHeight + MARGEN_CARGA;

  // Carga infinita. `IntersectionObserver` y no el evento `scroll` porque el
  // evento dispara decenas de veces por gesto y habría que frenarlo a mano.
  onMount(() => {
    const obs = new IntersectionObserver(
      (entradas) => {
        if (entradas.some((e) => e.isIntersecting)) loadMoreResults();
      },
      { root: fondo.closest(".scroll-area"), rootMargin: `${MARGEN_CARGA}px` },
    );
    obs.observe(fondo);
    onCleanup(() => obs.disconnect());
  });

  // El observador solo avisa de CAMBIOS de visibilidad. Cuando los resultados
  // caben en pantalla, el centinela ya está a la vista antes de que exista el
  // token, así que la única notificación llega demasiado pronto y no vuelve a
  // haber otra: la lista se queda corta para siempre. Este efecto cubre ese
  // caso — si al llegar un token nuevo el final sigue a la vista, se pide ya.
  createEffect(() => {
    if (searchMoreToken() && !loadingMore() && finalALaVista()) loadMoreResults();
  });

  const abrir = (item: ShelfItem, i: number) => {
    if (item.kind === "track") playFromResults(i);
    else openBrowse(item.id, item.title);
  };

  return (
    <div class="scroll-area h-full space-y-5 px-8 py-6">
      {/* Los filtros vienen de YouTube en la propia respuesta. */}
      <Show when={searchChips().length > 0}>
        <div class="scrollbar-none flex items-center gap-2 overflow-x-auto pb-1">
          <button
            class="shrink-0 rounded-full border px-3.5 py-1 text-xs font-semibold transition-all"
            classList={{
              "bg-white/20 border-white/25 text-white shadow-sm": searchFilter() === null,
              "bg-white/[0.04] border-white/10 text-white/60 hover:bg-white/10 hover:text-white":
                searchFilter() !== null,
            }}
            onClick={() => applySearchFilter(null)}
          >
            Todo
          </button>
          <For each={searchChips()}>
            {(c) => (
              <button
                class="shrink-0 rounded-full border px-3.5 py-1 text-xs font-semibold transition-all"
                classList={{
                  "bg-white/20 border-white/25 text-white shadow-sm": searchFilter() === c.params,
                  "bg-white/[0.04] border-white/10 text-white/60 hover:bg-white/10 hover:text-white":
                    searchFilter() !== c.params,
                }}
                onClick={() => applySearchFilter(c.params)}
              >
                {c.label}
              </button>
            )}
          </For>
        </div>
      </Show>

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
              <button
                class="rounded-full border border-white/10 bg-white/10 px-4 py-1.5 text-xs font-semibold text-white hover:bg-white/15"
                onClick={() => playFromResults(0)}
              >
                Reproducir todo
              </button>
            </div>
          </Show>

          <div class="fade-in flex flex-col gap-1">
            <For each={results()}>
              {(r, i) => {
                const activo = () => r.kind === "track" && playback.track?.videoId === r.id;
                return (
                  <div
                    class="group flex items-center gap-3.5 rounded-xl border border-transparent px-3.5 py-2.5 transition-all"
                    classList={{
                      "bg-white/[0.14] border-white/10 shadow-sm": activo(),
                      "hover:border-white/5 hover:bg-white/[0.06]": !activo(),
                    }}
                  >
                    <button
                      class="flex min-w-0 flex-1 items-center gap-3.5 text-left"
                      onClick={() => abrir(r, i())}
                    >
                    <div
                      class="relative size-11 shrink-0 overflow-hidden shadow-sm ring-1 ring-white/10"
                      classList={{
                        // Los artistas se pintan redondos, como en YouTube Music.
                        "rounded-full": r.kind === "artist",
                        "rounded-lg": r.kind !== "artist",
                      }}
                    >
                      <Show when={r.thumbnail} fallback={<div class="size-full bg-white/8" />}>
                        <img src={thumbAt(r.thumbnail, 96)!} alt="" class="size-full object-cover" />
                      </Show>
                      <Show when={r.kind === "track"}>
                        <div class="absolute inset-0 grid place-items-center bg-black/45 opacity-0 transition-opacity group-hover:opacity-100">
                          <I.Play size={16} class="text-white" />
                        </div>
                      </Show>
                    </div>

                    <div class="min-w-0 flex-1">
                      <div
                        class="truncate text-sm font-semibold text-white"
                        classList={{ "text-[var(--accent)]": activo() }}
                      >
                        {r.title}
                      </div>
                      <div class="mt-0.5 flex items-center gap-1.5 text-xs font-medium text-white/60">
                        {/* El tipo va delante, como en YouTube Music: es lo que
                            deja ver de un vistazo que la lista trae de todo. */}
                        <Show when={r.kind !== "track"}>
                          <span class="shrink-0 rounded bg-white/10 px-1.5 py-px text-[10px] uppercase tracking-wide text-white/70">
                            {TIPO[r.kind]}
                          </span>
                        </Show>
                        <span class="truncate">{r.subtitle}</span>
                      </div>
                    </div>

                      <span class="shrink-0 text-xs font-medium tabular-nums text-white/50">
                        {r.duration ?? ""}
                      </span>
                    </button>

                    <Show when={r.kind === "track"}>
                      <TrackMenu
                        artistId={r.artistId}
                        albumId={r.albumId}
                        track={{
                          videoId: r.id,
                          title: r.title,
                          author: r.subtitle,
                          thumbnail: r.thumbnail,
                        }}
                        class="opacity-0 group-hover:opacity-100"
                      />
                    </Show>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>
      </Show>

      {/* Centinela de la carga infinita. Va siempre, aunque no haya token: si
          apareciera y desapareciera, el observador se quedaría sin nada que
          mirar justo cuando llega la siguiente página. */}
      <div ref={fondo} class="h-px" />
      <Show when={loadingMore()}>
        <p class="py-4 text-center text-xs text-white/40">Cargando más…</p>
      </Show>
    </div>
  );
}


/* ------------------------------------------------------- Artista y álbum */

/**
 * Página de artista, álbum o playlist.
 *
 * Las tres son el mismo endpoint con distinto `browseId`, así que son la misma
 * pantalla: una cabecera y las estanterías que devuelva YouTube. Lo que cambia
 * entre ellas es lo que trae la respuesta, no el código.
 */
export function BrowseView() {
  const pagina = browsePage;

  /** Todas las pistas de la página, en orden, para "Reproducir" y "Aleatorio". */
  const pistas = () =>
    (pagina()?.shelves ?? []).flatMap((e) => e.items.filter((i) => i.kind === "track"));

  const reproducir = (desde = 0) => {
    const lista = pistas();
    const elegida = lista[desde];
    if (!elegida) return;
    playWithRadio({
      videoId: elegida.id,
      title: elegida.title,
      subtitle: elegida.subtitle,
      duration: elegida.duration,
      thumbnail: elegida.thumbnail,
    });
    togglePlayerView(true, false);
  };

  const aleatorio = () => {
    const lista = pistas();
    if (lista.length) reproducir(Math.floor(Math.random() * lista.length));
  };

  const abrir = (item: ShelfItem, i: number) => {
    if (item.kind === "track") {
      const desde = pistas().findIndex((t) => t.id === item.id);
      reproducir(desde >= 0 ? desde : i);
    } else {
      openBrowse(item.id.replace(/^VL/, "VL"), item.title);
    }
  };

  return (
    <div class="scroll-area h-full">
      <Show when={browseLoading()}>
        <div class="px-8 py-6">
          <div class="h-40 w-full animate-pulse rounded-2xl bg-white/8" />
        </div>
      </Show>

      <Show when={pagina()}>
        {/* Cabecera: portada a un lado, nombre y acciones al otro. */}
        <header class="flex items-end gap-6 px-8 pb-6 pt-8">
          <Show when={pagina()!.thumbnail}>
            <img
              src={thumbAt(pagina()!.thumbnail, 320)!}
              alt=""
              class="size-40 shrink-0 rounded-2xl object-cover shadow-[0_20px_50px_-15px_rgba(0,0,0,0.8)] ring-1 ring-white/10"
            />
          </Show>
          <div class="min-w-0 flex-1">
            <h1 class="truncate text-4xl font-extrabold tracking-tight text-white">
              {pagina()!.title ?? browseTitulo() ?? "Sin título"}
            </h1>
            <Show when={pagina()!.subtitle}>
              <p class="mt-1.5 truncate text-sm font-medium text-white/70">
                {pagina()!.subtitle}
              </p>
            </Show>
            <Show when={pagina()!.secondSubtitle}>
              <p class="mt-0.5 truncate text-[13px] text-white/50">
                {pagina()!.secondSubtitle}
              </p>
            </Show>
            <Show when={pagina()!.description}>
              <p class="mt-2 line-clamp-2 max-w-2xl text-[13px] leading-relaxed text-white/45">
                {pagina()!.description}
              </p>
            </Show>

            <Show when={pistas().length > 0}>
              <div class="mt-4 flex items-center gap-2">
                <button
                  class="flex items-center gap-2 rounded-full bg-white px-5 py-2 text-sm font-bold text-black transition-transform hover:scale-[1.03] active:scale-95"
                  onClick={() => reproducir(0)}
                >
                  <I.Play size={16} class="translate-x-[1px]" />
                  Reproducir
                </button>
                <button
                  class="flex items-center gap-2 rounded-full border border-white/15 bg-white/10 px-5 py-2 text-sm font-semibold text-white hover:bg-white/15"
                  onClick={aleatorio}
                >
                  <I.Shuffle size={16} />
                  Aleatorio
                </button>
                <button
                  class="flex items-center gap-2 rounded-full border border-white/15 bg-white/10 px-5 py-2 text-sm font-semibold text-white transition-all hover:bg-white/15 disabled:opacity-40"
                  onClick={() => saveBrowseAsPlaylist()}
                  disabled={savingBrowse()}
                  title="Copiar a una playlist tuya"
                >
                  <I.Plus size={16} />
                  {savingBrowse() ? "Guardando…" : "Guardar"}
                </button>
              </div>
            </Show>
          </div>
        </header>

        {/* Las pastillas de Explorar y de las categorías. Van antes que las
            estanterías porque son navegación, no contenido. */}
        <Show when={pagina()!.buttons.length > 0}>
          <div class="px-8 pb-2">
            <div class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
              <For each={pagina()!.buttons}>
                {(b) => (
                  <button
                    class="group relative overflow-hidden rounded-lg border border-white/10 bg-white/[0.07] py-3 pl-4 pr-3 text-left text-[13px] font-semibold text-white transition-colors hover:bg-white/[0.14]"
                    onClick={() => openBrowse(b.browseId, b.label, b.params ?? undefined)}
                  >
                    {/* La franja de color la manda YouTube con cada categoría.
                        Sin ella —los tres botones de arriba— la pastilla se
                        queda lisa, que es como se ven allí. */}
                    <Show when={b.stripe}>
                      <span
                        class="absolute inset-y-0 left-0 w-[3px]"
                        style={{ background: b.stripe! }}
                      />
                    </Show>
                    <span class="line-clamp-1">{b.label}</span>
                  </button>
                )}
              </For>
            </div>
          </div>
        </Show>

        {/* Las pistas van en lista vertical y lo demás en carrusel, que es como
            lo presenta la referencia: las canciones de un artista se leen en
            columna, sus álbumes se hojean en fila. */}
        <div class="space-y-9 px-8 pb-10">
          <For each={pagina()!.shelves}>
            {(e) => (
              <Show
                when={e.items.some((i) => i.kind !== "track")}
                fallback={<TrackList title={e.title} items={e.items} onPick={abrir} />}
              >
                <Shelf title={e.title} items={e.items} onPick={abrir} />
              </Show>
            )}
          </For>

          {/* Las listas largas llegan de 100 en 100. Se avisa en vez de dejar
              que parezca que la lista se acaba donde acaba la primera tanda. */}
          <Show when={browseCompleting()}>
            <p class="py-2 text-center text-[13px] text-white/40">
              Cargando el resto de la lista… {pistas().length} pistas
            </p>
          </Show>
        </div>
      </Show>

      <Show when={!browseLoading() && !pagina()}>
        <p class="mt-20 text-center text-sm text-white/40">
          No se pudo abrir esta página.
        </p>
      </Show>
    </div>
  );
}

/**
 * Lista vertical de pistas, para las estanterías que son una lista y no un
 * carrusel — la de canciones de un álbum, por ejemplo.
 */
function TrackList(p: { title: string; items: ShelfItem[]; onPick: (i: ShelfItem, n: number) => void }) {
  return (
    <section>
      {/* Un álbum trae su lista sin título: YouTube lo pone en la cabecera. */}
      <Show when={p.title}>
        <h2 class="mb-3 text-xl font-bold tracking-tight text-white">{p.title}</h2>
      </Show>
      <div class="flex flex-col gap-0.5">
        <For each={p.items}>
          {(t, i) => (
            <div class="group flex items-center gap-3.5 rounded-xl px-3 py-2 transition-colors hover:bg-white/[0.06]">
              <button
                class="flex min-w-0 flex-1 items-center gap-3.5 text-left"
                onClick={() => p.onPick(t, i())}
              >
                <span class="w-6 shrink-0 text-center text-xs tabular-nums text-white/35 group-hover:hidden">
                  {i() + 1}
                </span>
                <span class="hidden w-6 shrink-0 justify-center text-white group-hover:flex">
                  <I.Play size={13} />
                </span>
                <div class="min-w-0 flex-1">
                  <div class="truncate text-sm font-semibold text-white">{t.title}</div>
                  <div class="truncate text-xs text-white/55">{t.subtitle}</div>
                </div>
              </button>
              <span class="shrink-0 text-xs tabular-nums text-white/45">{t.duration ?? ""}</span>
              <TrackMenu
                artistId={t.artistId}
                albumId={t.albumId}
                track={{
                  videoId: t.id,
                  title: t.title,
                  author: t.subtitle,
                  thumbnail: t.thumbnail,
                }}
                class="opacity-0 group-hover:opacity-100"
              />
            </div>
          )}
        </For>
      </div>
    </section>
  );
}

/* ---------------------------------------------------------------- Library */

/** Favoritos e historial, ambos guardados en SQLite local. */
/**
 * Biblioteca: lo que es tuyo y vive en este equipo.
 *
 * Tres pestañas y no una lista sola porque son tres cosas distintas: lo que
 * armaste, lo que marcaste y lo que sonó. Todo sale de SQLite, sin red.
 */
export function LibraryView() {
  const [tab, setTab] = createSignal<"playlists" | "favorites" | "history">("playlists");
  const [rows, setRows] = createSignal<SavedTrack[]>([]);
  const [cargando, setCargando] = createSignal(false);

  const cargar = async () => {
    if (tab() === "playlists") {
      refreshPlaylists();
      return;
    }
    setCargando(true);
    try {
      setRows(tab() === "favorites" ? await api.favorites() : await api.history());
    } catch {
      setRows([]);
    } finally {
      setCargando(false);
    }
  };

  createEffect(on(tab, cargar));

  const pestanas = [
    ["playlists", "Playlists"],
    ["favorites", "Favoritos"],
    ["history", "Historial"],
  ] as const;

  return (
    <div class="scroll-area h-full px-8 py-6">
      <h1 class="mb-4 text-3xl font-extrabold tracking-tight text-white">Biblioteca</h1>

      <div class="mb-5 flex items-center gap-2">
        <For each={pestanas}>
          {([id, etiqueta]) => (
            <button
              class="rounded-full border px-4 py-1.5 text-xs font-semibold transition-all"
              classList={{
                "bg-white/20 border-white/25 text-white shadow-sm": tab() === id,
                "bg-white/[0.04] border-white/10 text-white/60 hover:bg-white/10 hover:text-white":
                  tab() !== id,
              }}
              onClick={() => setTab(id)}
            >
              {etiqueta}
            </button>
          )}
        </For>
      </div>

      <Show when={tab() === "playlists"}>
        <Show
          when={playlists().length > 0}
          fallback={
            <div class="py-16 text-center">
              <p class="text-sm text-white/40">Todavía no has creado ninguna playlist.</p>
              <button
                class="mt-4 rounded-full bg-white/10 px-5 py-2 text-sm font-semibold text-white hover:bg-white/15"
                onClick={() => setCreatingPlaylist(true)}
              >
                Crear la primera
              </button>
            </div>
          }
        >
          <div class="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
            <For each={playlists()}>
              {(l) => (
                <div class="group relative flex flex-col text-left">
                  {/* Encima de la portada, como en YouTube Music: la tarjeta
                      entera abre la lista, así que el menú no puede ir dentro
                      del botón — un botón dentro de otro no es HTML válido. */}
                  <PlaylistMenu
                    lista={l}
                    class="absolute right-1.5 top-1.5 z-10 rounded-full bg-black/45 opacity-0 backdrop-blur-sm transition-opacity group-hover:opacity-100"
                  />
                  <button class="flex flex-col text-left" onClick={() => showPlaylist(l)}>
                  <div class="relative aspect-square w-full overflow-hidden rounded-xl shadow-lg ring-1 ring-white/10">
                    <Show
                      when={l.thumbnail}
                      fallback={
                        <div class="grid size-full place-items-center bg-white/8 text-white/25">
                          <I.Music size={34} />
                        </div>
                      }
                    >
                      <img
                        src={thumbAt(l.thumbnail, 320)!}
                        alt=""
                        class="size-full object-cover transition-transform duration-300 group-hover:scale-105"
                      />
                    </Show>
                  </div>
                  <div class="mt-2.5 truncate text-[13px] font-bold text-white">{l.name}</div>
                  <div class="truncate text-[11.5px] text-white/50">
                    {l.count} {l.count === 1 ? "canción" : "canciones"}
                  </div>
                  </button>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>

      <Show when={tab() !== "playlists"}>
        <Show when={!cargando()} fallback={<SkeletonList />}>
          <Show
            when={rows().length > 0}
            fallback={
              <p class="py-16 text-center text-sm text-white/40">
                {tab() === "favorites"
                  ? "Aún no has guardado nada. Usa el pulgar del reproductor."
                  : "Todavía no has escuchado nada."}
              </p>
            }
          >
            <div class="flex flex-col gap-0.5">
              <For each={rows()}>
                {(t, i) => (
                  <div class="group flex items-center gap-3.5 rounded-xl px-3 py-2 transition-colors hover:bg-white/[0.06]">
                    <button
                      class="flex min-w-0 flex-1 items-center gap-3.5 text-left"
                      onClick={() => playSaved(rows(), i())}
                    >
                      <Show
                        when={t.thumbnail}
                        fallback={<div class="size-11 shrink-0 rounded-lg bg-white/8" />}
                      >
                        <img
                          src={thumbAt(t.thumbnail, 96)!}
                          alt=""
                          class="size-11 shrink-0 rounded-lg object-cover ring-1 ring-white/10"
                        />
                      </Show>
                      <div class="min-w-0 flex-1">
                        <div
                          class="truncate text-sm font-semibold text-white"
                          classList={{
                            "text-[var(--accent)]": playback.track?.videoId === t.videoId,
                          }}
                        >
                          {t.title}
                        </div>
                        <div class="truncate text-xs text-white/55">{t.author}</div>
                      </div>
                    </button>
                    <TrackMenu
                      track={{
                        videoId: t.videoId,
                        title: t.title,
                        author: t.author,
                        thumbnail: t.thumbnail,
                      }}
                      class="opacity-0 group-hover:opacity-100"
                    />
                  </div>
                )}
              </For>
            </div>
          </Show>
        </Show>
      </Show>
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
