# MyGlossMusic

Reproductor de YouTube Music nativo para escritorio. Rust + Tauri v2 + SolidJS.

> El proyecto se llamaba **Posible**. El nombre visible cambió a MyGlossMusic;
> las rutas de datos en disco (`posible-ytmusic/posible.db`) **no** se han
> tocado a propósito, para no dejar huérfanos los favoritos y el historial de
> quien ya lo tuviera instalado.

Tres objetivos, por orden: **que no se rompa**, que sea bonito, que vaya rápido.

## Por qué existe

Las alternativas actuales son un navegador que carga `music.youtube.com` y le
inyecta CSS y JavaScript por encima. Todo — el tema, las letras, el bloqueo de
anuncios, los plugins — es código parasitando una página que Google cambia cada
pocas semanas. De ahí vienen sus fallos.

Aquí no hay webview de YouTube. Se habla directamente con la API interna
(InnerTube), se decodifica el audio en Rust y la interfaz es propia. Cuando
YouTube cambia algo, solo hay que tocar un crate.

## Sobre la extracción (resuelto con yt-dlp)

**Verificado end-to-end: una pista de 1:30 suena entera a través de yt-dlp**, con
el muro de 1 MiB superado. Lo que sigue es la historia de por qué hizo falta.

~~La reproducción se corta a los ~48–65 segundos en casi todas las canciones.~~

Se ha investigado a fondo y está todo medido y reproducible con
`ytm-spike` (comandos `limits`, `clientlimits`, `strip`, `headers`, `rawurl`).
Resumen de lo que se sabe:

1. Sin **poToken**, googlevideo sirve 1 MiB y devuelve 403.
2. El poToken, la firma `sig` y el parámetro `n` solo los produce el JavaScript
   de YouTube. Un **webview oculto** (`minter.rs`) los acuña en ~2 s, en
   silencio, y la URL resultante se descarga desde Rust. Esto funciona.
3. Pero esa URL solo autoriza los bytes **que el reproductor de la página ya
   había pedido**: la autorización es *posicional* y avanza con la reproducción
   en vivo. Pedir un offset más alto devuelve 403; pedir el mismo offset ocho
   veces devuelve 200. Y el reproductor de YouTube bufferiza solo ~2–3 s por
   delante, así que no se le puede dejar descargar por nosotros.

Por tanto, hoy no hay forma de obtener una pista entera sin reimplementar el
protocolo de autorización en vivo de YouTube (UMP / `rbuf` / `cps`). Es un
protocolo activamente defendido y que cambia con frecuencia.

**Decisión tomada: `yt-dlp`.** La extracción se delega en `yt-dlp` como
proceso externo (`crates/ytm-source/src/ytdlp.rs`); el acuñador queda de
respaldo. El binario va como **sidecar** del bundle (`externalBin` en
`tauri.conf.json`); no se versiona en git — se descarga con:

```powershell
./scripts/fetch-ytdlp.ps1
```

Orden de detección en tiempo de ejecución: `POSIBLE_YTDLP`, `yt-dlp.exe` junto
al ejecutable (el sidecar), PATH, `python -m yt_dlp`. Sin ninguno, la app avisa
al arrancar y sigue con el respaldo capado a ~48 s. La versión activa se ve en
la pestaña **Diagnóstico**.

## Estado

| Fase | Estado |
|---|---|
| 0 · Validación de riesgo | ✅ |
| 1 · Motor de audio | ✅ |
| 2 · Aplicación Tauri + SolidJS | ✅ |
| 3 · Estética reactiva | ✅ |
| 4 · Persistencia local | ✅ favoritos, historial, ajustes · ⏳ sesión de YouTube |
| 5 · Letras, teclas multimedia, Discord | ✅ (Discord sin verificar en vivo) |
| 6 · Antifragilidad | ✅ diagnóstico y cascada · ⏳ CI |
| 7 · Navegación | ✅ inicio, búsqueda, artista, álbum, playlist, historial |
| 8 · Playlists propias | ✅ crear, añadir, quitar, borrar (locales) |

## Ejecutar

```bash
cd apps/desktop
npm install     # solo la primera vez
npm run tauri dev
```

`npm run tauri dev` levanta Vite y la aplicación juntos. `cargo run -p myglossmusic`
por su cuenta **no** basta: la app en modo desarrollo espera el servidor de Vite
en el puerto 1420.

Para generar un ejecutable de verdad:

```bash
npm run tauri build --prefix apps/desktop
```

## Qué se puede hacer sin cuenta

Todo lo que hay funciona **en modo anónimo**. Lo que eso permite y lo que no,
medido y no supuesto:

| | Sin sesión |
|---|---|
| Buscar (canciones, vídeos, artistas, álbumes, listas) | ✅ con paginación |
| Sugerencias mientras escribes | ✅ |
| Radio de una canción (~50 recomendadas) | ✅ |
| Página de artista, álbum y playlist | ✅ |
| Feed de inicio de YouTube | ⚠️ solo 2 carruseles genéricos |
| "Volver a escuchar" y recomendaciones personales | ❌ de Google · ✅ desde el historial local |
| Playlists propias | ✅ locales, en SQLite |
| Guardar una playlist **en tu cuenta de Google** | ❌ requiere sesión |

El inicio combina las tres fuentes: tu historial, radios sembradas desde él, y
los carruseles anónimos de YouTube.

## Herramientas de diagnóstico

Cuando algo deje de sonar, esto es lo primero que hay que ejecutar:

```bash
cargo run -p ytm-spike -- probe dQw4w9WgXcQ
```

Dice en 10 segundos qué clientes de YouTube siguen vivos. La app lleva lo mismo
en su pestaña **Diagnóstico**.

Otros comandos:

```bash
cargo run -p ytm-spike -- search kastra fool for you   # búsqueda
cargo run -p ytm-spike -- suggest kastra               # sugerencias
cargo run -p ytm-spike -- radio dQw4w9WgXcQ            # recomendaciones
cargo run -p ytm-spike -- browse                       # feed de inicio
cargo run -p ytm-spike -- bench dQw4w9WgXcQ            # ¿hay throttling?
cargo run -p ytm-spike -- engine dQw4w9WgXcQ           # motor de audio
```

## Estructura

```
crates/
  ytm-source/   Todo lo que Google puede cambiar, aislado aquí
  ytm-audio/    Motor: caché progresivo, cola, reproducción
  ytm-spike/    Herramientas de diagnóstico
apps/desktop/
  src-tauri/    Comandos, paleta, letras, SQLite, teclas multimedia
  src/          Interfaz SolidJS
```

## Decisiones que conviene no deshacer

Están documentadas en el código, pero estas son las que más cuestan de
redescubrir:

- **Las descargas van por rangos, nunca de una pieza.** Un GET completo lo
  estrangula YouTube a 0.03 MB/s; por rangos van a ~29 MB/s. Esto es lo que nos
  evita necesitar un motor JavaScript para descifrar el parámetro `n`.
- **`&range=` en la query y la cabecera `Range:` no se combinan.** La query corta
  del lado del servidor, así que la cabecera se aplica sobre el trozo ya cortado
  y devuelve `416` en cualquier offset que no sea 0.
- **itag 140 (AAC), no 251 (Opus).** Symphonia 0.6 sigue sin soportar Opus.
- **La paleta se calcula en Oklab, no en RGB.** En RGB el k-means produce
  marrones sucios porque la distancia no se corresponde con lo que ve el ojo.
- **El fondo se desenfoca en pequeño y se amplía después.** La capa de la
  portada mide 320×180 px reales y se escala ×11 con `transform`. El orden de
  pintado es filtro primero, transformación después, así que `blur(9px)` sobre
  320 px equivale a ~100 px en pantalla pero se calcula sobre una superficie 120
  veces menor. Un `blur(100px)` a tamaño de ventana repinta cada fotograma; este
  no. Sin este truco quedan las formas del original a la vista, y taparlas exige
  una viñeta que apaga la ventana entera — que fue exactamente lo que pasó.
- **Debajo de la portada van focos de color, no en vez de ella.** `palette.rs`
  devuelve `stops[]` y la interfaz pinta un degradado radial por color. Rellenan
  donde la portada es oscura y son lo único que queda cuando aún no hay
  carátula. Medido contra la referencia: el tono de su fondo coincide con la
  portada desenfocada con 16-25° de diferencia, y con la portada nítida se va a
  78-94°, así que la imagen manda y los focos acompañan.
- **Los focos del fondo llevan el color en `background-color` y el recorte en
  `mask-image`.** Con el degradado en `background` el color no transiciona y el
  cambio de canción corta en seco. Y son siempre cinco, aunque la portada dé
  menos colores: si el número de nodos cambiara, los que sobran desaparecerían
  de golpe en vez de apagarse.
- **La búsqueda recorre el JSON en vez de indexar rutas fijas.** Las rutas de
  InnerTube tienen ~10 niveles y cambian; los nombres de renderer no.
- **Dos clientes HTTP separados.** El de streams es anónimo por construcción. El
  plano de biblioteca (que llevará cookies) es otro distinto. Mezclar cookies con
  un cliente suplantado es lo único que pone en riesgo la cuenta del usuario.

## Sobre los anuncios

No hay código de bloqueo de anuncios, y no hace falta. Los anuncios son entradas
separadas de la respuesta del reproductor (`adPlacements`, `playerAds`) y streams
distintos. Leyendo solo `streamingData.adaptiveFormats` nunca se descarga uno.

## Aviso

Esto viola los Términos de Servicio de YouTube. La app funciona en **modo anónimo
por defecto**: sin sesión iniciada no hay ninguna cuenta que Google pueda
sancionar. Si en el futuro se añade el inicio de sesión, será opcional y
explícito, y las peticiones de audio seguirán yendo sin cookies.

Para desarrollar, usa una cuenta de Google desechable.

## Licencia

GPLv3.
