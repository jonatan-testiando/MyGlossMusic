# Roadmap

Reproductor de YouTube Music nativo. Objetivos, por orden: **que no se rompa**,
que sea bonito, que vaya rápido.

## Principio rector

Todo lo que Google puede cambiar vive aislado en `ytm-source`, detrás de tipos
estables, con cascada de fallbacks y diagnosticable en 10 segundos
(`ytm-spike probe`). El resto de la aplicación no se entera de que YouTube
existe. Esa es la única diferencia real frente a las alternativas actuales, que
inyectan código en la web de YouTube y se rompen cada vez que cambia una clase
CSS.

## Regla de oro: dos planos separados

| Plano | Cliente HTTP | Cliente InnerTube | Cookies |
|---|---|---|---|
| **Biblioteca** (playlists, likes, historial) | con `cookie_store` | `WEB_REMIX` | **sí** |
| **Audio** (resolver y descargar streams) | sin cookie store | `ios` / `android_vr` | **nunca** |

Son dos `reqwest::Client` distintos, sin nada compartido. Mezclar cookies con un
cliente suplantado es lo único que pone en riesgo la cuenta del usuario; con esta
separación ese caso no existe en el código.

---

## Fase 0 — Validación de riesgo ✅ COMPLETADA

Objetivo: probar que la extracción y reproducción funcionan, antes de invertir
en interfaz.

### Hallazgos medidos (2026-09-02)

**1. Clientes InnerTube vivos.** De seis probados, solo dos devuelven URLs de
audio directas (sin descifrado):

| Cliente | Estado | Audio directo |
|---|---|---|
| `ios` | OK | sí — itag 140, AAC-LC 130 kbps |
| `android_vr` | OK | sí — itag 140, AAC-LC 130 kbps |
| `tv`, `mweb`, `web_remix` | UNPLAYABLE | no |
| `tv_embedded` | ERROR | no |

**2. El throttling del parámetro `n` se esquiva con peticiones por rango.**
Este es el hallazgo que define la arquitectura del motor de audio:

| Estrategia | Velocidad |
|---|---|
| GET completo | **0.03 MB/s** (estrangulado) |
| Rango vía cabecera `Range:` | 29–33 MB/s |
| Rango vía query `&range=` | 18–24 MB/s |

Verificado con conexión nueva y rango en mitad del archivo, para descartar que
fuera reutilización de TLS o trato especial al primer trozo. Tiempo real exige
0.016 MB/s.

**Consecuencia: no necesitamos descifrar el parámetro `n`.**

> ⚠️ **CORREGIDO el 2026-09-03.** La frase original decía además "ni poToken".
> Era falso. Esta fase se validó con un único vídeo, `dQw4w9WgXcQ`, que resultó
> ser una excepción. Medido después con `ytm-spike limits` sobre 6 vídeos, 5 de
> 6 cortan en 1 MiB (~65 s) con 403. **Sí hace falta poToken.** Ver la sección
> "Limitación conocida" del README.
>
> Lección de método: validar el camino crítico con una sola muestra, y encima
> con la más conocida (que por eso mismo es la más permisiva), no es validar.

Efecto en la práctica sobre la misma canción: **106.5 s → 0.1 s**.

**3. `&range=` y `Range:` no se combinan.** El parámetro de query corta del lado
del servidor, así que la cabecera se aplica sobre el trozo ya cortado y devuelve
`416` en cualquier offset que no sea 0. Hay que usar uno u otro, no ambos.

**4. Symphonia 0.6.1 sigue sin soportar Opus** (`all-codecs = [aac, adpcm, alac,
flac, mp1, mp2, mp3, pcm, vorbis]`). Por eso el itag preferido es **140 (AAC-LC)**
y no 251 (Opus): se decodifica nativo, sin enlazar libopus, con diferencia
audible despreciable. Reordenar `select::AUDIO_ITAG_PREFERENCE` el día que
cambie.

**5. El bloqueo de anuncios no se implementa: es gratis.** Los anuncios son
entradas separadas (`adPlacements`, `playerAds`) y streams distintos. Leyendo
solo `streamingData.adaptiveFormats` nunca se descarga uno.

### Herramientas que deja la fase

```bash
cargo run -p ytm-spike -- probe <videoId>   # ¿qué clientes viven hoy?
cargo run -p ytm-spike -- bench <videoId>   # ¿hay throttling?
cargo run -p ytm-spike -- play  <videoId>   # extremo a extremo
```

`probe` es la herramienta de diagnóstico permanente del proyecto: cuando algo
deje de sonar, es lo primero que se ejecuta.

---

## Fase 1 — Motor de audio ✅ COMPLETADA

- [x] Descarga por trozos de 1 MiB con cabecera `Range:`
- [x] Caché progresivo en disco (se descartó el buffer circular: con 25 MB/s
      medidos, una pista entera cabe en ~0.15 s y el archivo da seek trivial,
      arranque instantáneo y reproducción offline gratis)
- [x] `seek` — medido exacto a 1:30
- [x] Cola con repetición off/all/one y aleatorio · 6 tests
- [x] Precarga de la siguiente pista
- [ ] Normalización de volumen con `loudness_db` (ya se extrae, falta aplicarlo)
- [ ] Renovación de URL caducada (~6 h) de forma transparente

**Medido:** 325 ms hasta el primer sonido. Pausa congela la posición, la
reanudación avanza.

## Fase 2 — Shell Tauri + SolidJS ✅ COMPLETADA

- [x] Tauri v2 sin marco de ventana, comandos hacia el motor
- [x] SolidJS + Tailwind v4 — 45 KB de JS
- [x] Barra lateral, panel central, panel derecho, reproductor flotante
- [x] Búsqueda contra InnerTube, parseada por recorrido recursivo del JSON
- [x] Modo mock fuera de Tauri para desarrollar la interfaz en el navegador

## Fase 3 — Estética ✅ COMPLETADA

- [x] Paleta en Oklab con k-means determinista, sobre la portada a 48×48 · 4 tests
- [x] Fondo: portada a 48 px estirada a pantalla completa, sin `backdrop-filter`
- [x] Paneles `rgba(255,255,255,0.055)` + borde sutil, sin blur
- [x] Transiciones por `opacity` y variables CSS
- [ ] Opcional: Mica/Acrylic con `window-vibrancy` para los bordes. Ojo: eso
      desenfoca **el escritorio**, no la portada; es otro efecto.

## Fase 4 — Biblioteca ⏳ PARCIAL

- [x] SQLite (`rusqlite`): favoritos, historial, ajustes persistentes · 3 tests
- [x] **Modo anónimo**: es el único modo que hay ahora mismo
- [ ] Login por cookies, cliente `WEB_REMIX` (plano separado, ver regla de oro)
- [ ] Playlists y Liked Music de la cuenta

> El inicio de sesión requiere las cookies de Google del usuario. Queda
> pendiente a propósito: es la parte con más riesgo para la cuenta y debe ser
> una decisión explícita suya, no algo que aparezca sin más.

## Fase 5 — Integración ✅ COMPLETADA

- [x] Letras vía LRCLIB, con búsqueda exacta y respaldo difuso · 4 tests
- [x] Barrido de texto interpolando la duración de línea con `background-clip`
- [x] `souvlaki`: teclas multimedia y SMTC de Windows — verificado activo
- [x] Discord Rich Presence — compila y degrada bien, **sin verificar en vivo**
      (haría falta Discord abierto y un app id propio; ver `discord.rs`)

## Fase 6 — Antifragilidad ⏳ PARCIAL

Lo que de verdad diferencia el proyecto.

- [x] Cascada de clientes con degradación, en `resolve()`
- [x] Panel de diagnóstico en la app, equivalente a `probe`
- [x] Reintento con cliente alternativo (implícito en la cascada)
- [x] Caché de pistas en disco: lo ya escuchado sobrevive a un corte de red
- [ ] Test de integración en CI que detecte roturas de YouTube antes que el usuario

## Pendiente, por orden de valor

0. **Extracción completa — integrada vía `yt-dlp` (opción B), pendiente de
   verificar en vivo** con el binario instalado. El motor acepta ahora una
   fuente externa que escribe el archivo de caché (`Provided::External`), con
   registro de precargas para que precargar y reproducir no abran dos descargas
   sobre el mismo archivo, y un corte de descarga ya no mata lo descargado.
   Descartada la opción A (protocolo en vivo de YouTube): medido que la
   autorización es posicional y ligada a la reproducción; ver README.
1. **CI que detecte roturas de YouTube** antes que el usuario.
2. **Normalización de volumen** con `loudness_db`, que ya se extrae.
3. **Renovación de URL caducada** (~6 h) sin cortar la reproducción.
4. **Inicio de sesión**, si lo quieres, con la separación de planos intacta.
5. Verificar Discord RPC con Discord abierto y un app id propio.

## Fuera de alcance en la v1

Sistema de plugins. Es precisamente lo que volvió inmantenibles a las
alternativas. La abstracción de fuentes (`ytm-source`) deja la puerta abierta a
archivos locales y Navidrome/Subsonic más adelante.

## Licencia

GPLv3, para que nadie pueda reempaquetar esto cerrado o con adware.
