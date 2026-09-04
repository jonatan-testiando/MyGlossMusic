import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { thumbAt, type Playlist } from "../lib/api";
import {
  addTrackToPlaylist,
  addingTo,
  createPlaylist,
  deletePlaylist,
  openPlaylist,
  playSaved,
  playlists,
  removeTrackFromPlaylist,
  renamePlaylist,
  setAddingTo,
  setCreatingPlaylist,
  showPlaylist,
} from "../lib/store";
import { MenuKebab } from "./MenuKebab";
import * as I from "./Icons";

/**
 * Diálogo modal reutilizable.
 *
 * Cierra con Escape y pulsando fuera. Lo segundo se hace comparando con
 * `e.target`, no con `contains`: un clic que empieza dentro del cuadro y
 * termina fuera — al seleccionar texto y soltar — no debe cerrar el diálogo.
 */
function Modal(p: { title: string; onClose: () => void; children: any }) {
  onMount(() => {
    const alPulsar = (e: KeyboardEvent) => {
      if (e.key === "Escape") p.onClose();
    };
    window.addEventListener("keydown", alPulsar);
    onCleanup(() => window.removeEventListener("keydown", alPulsar));
  });

  return (
    <div
      class="fixed inset-0 z-50 grid place-items-center bg-black/50 p-6 backdrop-blur-sm"
      onClick={(e) => {
        if (e.target === e.currentTarget) p.onClose();
      }}
    >
      <div class="glass-card w-full max-w-md overflow-hidden rounded-2xl">
        <div class="flex items-center justify-between border-b border-white/10 px-5 py-3.5">
          <h2 class="text-base font-bold text-white">{p.title}</h2>
          <button class="icon-btn size-7 rounded-full hover:bg-white/10" onClick={p.onClose} title="Cerrar">
            <I.Close size={14} />
          </button>
        </div>
        {p.children}
      </div>
    </div>
  );
}

/** Pide un nombre y crea la playlist. */
export function CreatePlaylistDialog() {
  const [nombre, setNombre] = createSignal("");
  let campo!: HTMLInputElement;

  onMount(() => campo.focus());

  const crear = async () => {
    const n = nombre().trim();
    if (!n) return;
    const id = await createPlaylist(n);
    setCreatingPlaylist(false);
    // Se abre recién creada: si no, no hay señal de que haya pasado nada.
    const lista = playlists().find((l) => l.id === id);
    if (lista) showPlaylist(lista);
  };

  return (
    <Modal title="Nueva playlist" onClose={() => setCreatingPlaylist(false)}>
      <form
        class="space-y-4 p-5"
        onSubmit={(e) => {
          e.preventDefault();
          crear();
        }}
      >
        <div>
          <label class="mb-1.5 block text-[11px] font-bold uppercase tracking-wider text-white/45">
            Nombre
          </label>
          <input
            ref={campo}
            value={nombre()}
            onInput={(e) => setNombre(e.currentTarget.value)}
            placeholder="Para programar, gimnasio, lo que sea"
            maxlength={80}
            class="w-full rounded-xl border border-white/10 bg-white/[0.06] px-3.5 py-2.5 text-sm text-white outline-none placeholder:text-white/30 focus:border-white/25"
          />
        </div>
        <div class="flex justify-end gap-2">
          <button
            type="button"
            class="rounded-full px-4 py-2 text-sm font-semibold text-white/70 hover:text-white"
            onClick={() => setCreatingPlaylist(false)}
          >
            Cancelar
          </button>
          <button
            type="submit"
            class="rounded-full bg-white px-5 py-2 text-sm font-bold text-black transition-opacity disabled:opacity-30"
            disabled={!nombre().trim()}
          >
            Crear
          </button>
        </div>
      </form>
    </Modal>
  );
}

/** Elige a qué playlist va una pista. */
export function AddToPlaylistDialog() {
  const pista = () => addingTo()!;

  return (
    <Modal title="Guardar en playlist" onClose={() => setAddingTo(null)}>
      <div class="max-h-[50vh] overflow-y-auto p-2">
        <Show
          when={playlists().length > 0}
          fallback={
            <p class="px-4 py-8 text-center text-sm text-white/40">
              Todavía no tienes ninguna playlist.
            </p>
          }
        >
          <For each={playlists()}>
            {(l) => (
              <button
                class="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-colors hover:bg-white/[0.08]"
                onClick={() => {
                  addTrackToPlaylist(l.id, pista());
                  setAddingTo(null);
                }}
              >
                <Show
                  when={l.thumbnail}
                  fallback={
                    <div class="grid size-10 shrink-0 place-items-center rounded-lg bg-white/8 text-white/40">
                      <I.Music size={16} />
                    </div>
                  }
                >
                  <img src={thumbAt(l.thumbnail, 80)!} alt="" class="size-10 shrink-0 rounded-lg object-cover" />
                </Show>
                <div class="min-w-0 flex-1">
                  <div class="truncate text-sm font-semibold text-white">{l.name}</div>
                  <div class="text-[11.5px] text-white/45">
                    {l.count} {l.count === 1 ? "canción" : "canciones"}
                  </div>
                </div>
              </button>
            )}
          </For>
        </Show>
      </div>

      <div class="border-t border-white/10 p-3">
        <button
          class="flex w-full items-center justify-center gap-2 rounded-full bg-white/10 py-2.5 text-sm font-semibold text-white hover:bg-white/15"
          onClick={() => {
            // El diálogo de crear sustituye a este: encadenarlos apilaría dos
            // modales y el fondo quedaría oscurecido dos veces.
            setAddingTo(null);
            setCreatingPlaylist(true);
          }}
        >
          <I.Plus size={15} />
          Nueva playlist
        </button>
      </div>
    </Modal>
  );
}

/**
 * Menú de una playlist: cambiar nombre y eliminar.
 *
 * Va en los tres sitios donde aparece una lista — barra lateral, biblioteca y
 * su propia cabecera — porque antes borrar solo se podía desde una papelera sin
 * etiqueta dentro de la lista, y no había forma humana de dar con ella.
 */
export function PlaylistMenu(p: { lista: Playlist; class?: string }) {
  const [renombrando, setRenombrando] = createSignal(false);
  const [borrando, setBorrando] = createSignal(false);

  return (
    <>
      <MenuKebab
        titulo="Opciones de la playlist"
        class={p.class}
        acciones={() => [
          { etiqueta: "Cambiar nombre", icono: I.Pencil, hacer: () => setRenombrando(true) },
          { etiqueta: "Eliminar playlist", icono: I.Trash, hacer: () => setBorrando(true) },
        ]}
      />

      <Show when={renombrando()}>
        <RenamePlaylistDialog lista={p.lista} onClose={() => setRenombrando(false)} />
      </Show>
      <Show when={borrando()}>
        <DeletePlaylistDialog lista={p.lista} onClose={() => setBorrando(false)} />
      </Show>
    </>
  );
}

function RenamePlaylistDialog(p: { lista: Playlist; onClose: () => void }) {
  const [nombre, setNombre] = createSignal(p.lista.name);
  let campo!: HTMLInputElement;

  onMount(() => {
    campo.focus();
    campo.select();
  });

  return (
    <Modal title="Cambiar nombre" onClose={p.onClose}>
      <form
        class="space-y-4 p-5"
        onSubmit={(e) => {
          e.preventDefault();
          renamePlaylist(p.lista.id, nombre());
          p.onClose();
        }}
      >
        <input
          ref={campo}
          value={nombre()}
          onInput={(e) => setNombre(e.currentTarget.value)}
          maxlength={80}
          class="w-full rounded-xl border border-white/10 bg-white/[0.06] px-3.5 py-2.5 text-sm text-white outline-none placeholder:text-white/30 focus:border-white/25"
        />
        <div class="flex justify-end gap-2">
          <button
            type="button"
            class="rounded-full px-4 py-2 text-sm font-semibold text-white/70 hover:text-white"
            onClick={p.onClose}
          >
            Cancelar
          </button>
          <button
            type="submit"
            class="rounded-full bg-white px-5 py-2 text-sm font-bold text-black transition-opacity disabled:opacity-30"
            disabled={!nombre().trim()}
          >
            Guardar
          </button>
        </div>
      </form>
    </Modal>
  );
}

/**
 * Confirmación de borrado.
 *
 * Propia y no `window.confirm`: el diálogo del sistema es feo, se sale del
 * estilo de todo lo demás y su comportamiento dentro de un WebView no está
 * garantizado. Aquí además cabe decir qué se pierde exactamente.
 */
function DeletePlaylistDialog(p: { lista: Playlist; onClose: () => void }) {
  return (
    <Modal title="Eliminar playlist" onClose={p.onClose}>
      <div class="space-y-5 p-5">
        <p class="text-sm leading-relaxed text-white/70">
          Se borra <span class="font-bold text-white">{p.lista.name}</span> y sus{" "}
          {p.lista.count} {p.lista.count === 1 ? "canción" : "canciones"}. Las
          canciones en sí no se tocan: siguen en YouTube y en tu caché.
        </p>
        <div class="flex justify-end gap-2">
          <button
            class="rounded-full px-4 py-2 text-sm font-semibold text-white/70 hover:text-white"
            onClick={p.onClose}
          >
            Cancelar
          </button>
          <button
            class="rounded-full bg-red-500/90 px-5 py-2 text-sm font-bold text-white hover:bg-red-500"
            onClick={() => {
              deletePlaylist(p.lista.id);
              p.onClose();
            }}
          >
            Eliminar
          </button>
        </div>
      </div>
    </Modal>
  );
}

/** Contenido de una playlist local. */
export function PlaylistView() {
  const datos = openPlaylist;

  const reproducir = (desde = 0) => {
    const t = datos()?.tracks ?? [];
    if (t.length) playSaved(t, desde);
  };

  return (
    <div class="scroll-area h-full">
      <Show
        when={datos()}
        fallback={<p class="mt-20 text-center text-sm text-white/40">Playlist no encontrada.</p>}
      >
        <header class="flex items-end gap-6 px-8 pb-6 pt-8">
          <Show
            when={datos()!.lista.thumbnail}
            fallback={
              <div class="grid size-40 shrink-0 place-items-center rounded-2xl bg-white/8 text-white/25">
                <I.Music size={44} />
              </div>
            }
          >
            <img
              src={thumbAt(datos()!.lista.thumbnail, 320)!}
              alt=""
              class="size-40 shrink-0 rounded-2xl object-cover shadow-[0_20px_50px_-15px_rgba(0,0,0,0.8)] ring-1 ring-white/10"
            />
          </Show>

          <div class="min-w-0 flex-1">
            <div class="text-[10.5px] font-bold uppercase tracking-widest text-white/45">
              Playlist tuya
            </div>
            <h1 class="truncate text-4xl font-extrabold tracking-tight text-white">
              {datos()!.lista.name}
            </h1>
            <p class="mt-1.5 text-sm font-medium text-white/60">
              {datos()!.tracks.length}{" "}
              {datos()!.tracks.length === 1 ? "canción" : "canciones"}
            </p>

            <div class="mt-4 flex items-center gap-2">
              <button
                class="flex items-center gap-2 rounded-full bg-white px-5 py-2 text-sm font-bold text-black transition-transform hover:scale-[1.03] active:scale-95 disabled:opacity-30"
                onClick={() => reproducir(0)}
                disabled={datos()!.tracks.length === 0}
              >
                <I.Play size={16} class="translate-x-[1px]" />
                Reproducir
              </button>
              <PlaylistMenu lista={datos()!.lista} />
            </div>
          </div>
        </header>

        <div class="px-8 pb-10">
          <Show
            when={datos()!.tracks.length > 0}
            fallback={
              <p class="py-12 text-center text-sm text-white/40">
                Esta playlist está vacía. Usa el menú de una canción para añadirla.
              </p>
            }
          >
            <For each={datos()!.tracks}>
              {(t, i) => (
                <div class="group flex items-center gap-3.5 rounded-xl px-3 py-2 transition-colors hover:bg-white/[0.06]">
                  <button
                    class="flex min-w-0 flex-1 items-center gap-3.5 text-left"
                    onClick={() => reproducir(i())}
                  >
                    <span class="w-6 shrink-0 text-center text-xs tabular-nums text-white/35 group-hover:hidden">
                      {i() + 1}
                    </span>
                    <span class="hidden w-6 shrink-0 justify-center text-white group-hover:flex">
                      <I.Play size={13} />
                    </span>
                    <Show
                      when={t.thumbnail}
                      fallback={<div class="size-10 shrink-0 rounded-lg bg-white/8" />}
                    >
                      <img
                        src={thumbAt(t.thumbnail, 80)!}
                        alt=""
                        class="size-10 shrink-0 rounded-lg object-cover ring-1 ring-white/10"
                      />
                    </Show>
                    <div class="min-w-0 flex-1">
                      <div class="truncate text-sm font-semibold text-white">{t.title}</div>
                      <div class="truncate text-xs text-white/55">{t.author}</div>
                    </div>
                  </button>
                  <button
                    class="icon-btn size-8 shrink-0 text-white/35 opacity-0 hover:!bg-white/10 hover:text-white group-hover:opacity-100"
                    onClick={() => removeTrackFromPlaylist(datos()!.lista.id, t.videoId)}
                    title="Quitar de la playlist"
                  >
                    <I.Close size={13} />
                  </button>
                </div>
              )}
            </For>
          </Show>
        </div>
      </Show>
    </div>
  );
}
