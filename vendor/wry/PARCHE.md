# wry 0.55.1 con parche local

Copia identica de wry 0.55.1 de crates.io con UN solo cambio, en
`src/webview2/mod.rs` (`parent_subclass_proc`):

- El original, al destruirse el webview, re-registraba el subclass del padre
  con `dwrefdata = NULL`; cualquier `WM_MOVE`/`WM_SETFOCUS`/`WM_SIZE` posterior
  desreferenciaba ese NULL: panico no desenrollable en el hilo principal.
  Sintoma: al minimizar/restaurar, la ventana muere para siempre mientras el
  audio sigue sonando. Reproducido de forma deterministica con
  `scripts/harness-minimize.ps1` (congelada en el ciclo 2).
- Backport del arreglo de wry 0.56.1: `RemoveWindowSubclass` en la destruccion,
  mas una guarda de nulo a la entrada del proc.

Se engancha con `[patch.crates-io]` en el Cargo.toml raiz porque
tauri-runtime-wry (hasta 2.11.4) exige wry ^0.55 y el arreglo oficial vive en
0.56.1, que no es semver-compatible.

**Cuando Tauri publique un runtime que use wry >= 0.56: borrar este directorio
y el bloque `[patch.crates-io]`, y volver a pasar el arnes.**
