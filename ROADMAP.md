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

**Consecuencia: no necesitamos motor JavaScript, ni descifrar `n`, ni poToken.**
Se elimina el riesgo más grande del proyecto y una dependencia enorme.

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

## Fase 1 — Motor de audio

Lo más difícil que queda. Diseño ya condicionado por los hallazgos de Fase 0.

- [ ] `ytm-audio`: descarga por trozos de 1 MiB con cabecera `Range:`
- [ ] Buffer circular con relleno anticipado (2–3 trozos de margen)
- [ ] `seek` → recolocar offset y purgar buffer
- [ ] Cola de reproducción, siguiente/anterior, repetición, aleatorio
- [ ] Precarga de la siguiente pista para transición sin corte
- [ ] Normalización de volumen usando `loudness_db`, que ya extraemos
- [ ] Renovación de URL caducada (expiran a las ~6 h) de forma transparente

## Fase 2 — Shell Tauri + SolidJS

- [ ] Tauri v2 sin marco de ventana, comandos hacia el motor
- [ ] SolidJS + Tailwind, estado por señales
- [ ] Layout base: barra lateral, panel central, reproductor flotante
- [ ] Búsqueda contra InnerTube

## Fase 3 — Estética

El objetivo visual es el fondo reactivo al color de la portada.

- [ ] Extracción de paleta en Rust, **en Oklab, no en RGB** (en RGB salen colores
      sucios y sin contraste). Sobre la portada a 64×64.
- [ ] Fondo: portada a 32×32 escalada a pantalla completa. El reescalado bilineal
      del navegador *es* el desenfoque, y cuesta cero. **Nada de `backdrop-filter`.**
- [ ] Paneles: `rgba(255,255,255,0.06)` + borde sutil. Sin blur.
- [ ] Transición entre canciones: crossfade de dos capas por `opacity`
- [ ] Opcional: Mica/Acrylic nativo con `window-vibrancy` para los bordes.
      Ojo: eso desenfoca **el escritorio**, no la portada; es otro efecto.

## Fase 4 — Biblioteca

- [ ] Login por cookies, cliente `WEB_REMIX` (plano separado, ver regla de oro)
- [ ] **Modo anónimo por defecto**; sesión opcional y explícita
- [ ] Playlists, Liked Music, historial
- [ ] Caché SQLite (`rusqlite`): metadatos, carátulas, resultados

## Fase 5 — Integración

- [ ] Letras: LRCLIB con cascada de proveedores
- [ ] Resaltado por palabra interpolando la duración de cada línea
      (LRCLIB da sincronía por línea; el efecto palabra a palabra se simula)
- [ ] `souvlaki`: teclas multimedia y SMTC de Windows
- [ ] Discord Rich Presence

## Fase 6 — Antifragilidad

Lo que de verdad diferencia el proyecto.

- [ ] Cascada de clientes con degradación (ya en `resolve()`)
- [ ] Panel de diagnóstico en la app, equivalente a `probe`
- [ ] Reintento con cliente alternativo al fallar una pista
- [ ] Caché de streams para que un corte de red no pare la música
- [ ] Test de integración en CI que detecte roturas de YouTube antes que el usuario

## Fuera de alcance en la v1

Sistema de plugins. Es precisamente lo que volvió inmantenibles a las
alternativas. La abstracción de fuentes (`ytm-source`) deja la puerta abierta a
archivos locales y Navidrome/Subsonic más adelante.

## Licencia

GPLv3, para que nadie pueda reempaquetar esto cerrado o con adware.
