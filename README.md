# Posible

Reproductor de YouTube Music nativo para escritorio. Rust + Tauri v2 + SolidJS.

Tres objetivos, por orden: **que no se rompa**, que sea bonito, que vaya rápido.

## Por qué existe

Las alternativas actuales son un navegador que carga `music.youtube.com` y le
inyecta CSS y JavaScript por encima. Todo — el tema, las letras, el bloqueo de
anuncios, los plugins — es código parasitando una página que Google cambia cada
pocas semanas. De ahí vienen sus fallos.

Aquí no hay webview de YouTube. Se habla directamente con la API interna
(InnerTube), se decodifica el audio en Rust y la interfaz es propia. Cuando
YouTube cambia algo, solo hay que tocar un crate.

## Limitación conocida y grave

**La reproducción se corta a los ~65 segundos en la mayoría de canciones.**

Medido con `ytm-spike limits` sobre 6 vídeos: googlevideo sirve exactamente
1 MiB (~65 s en itag 140) y devuelve 403 para el resto si la petición no lleva
un **poToken** (token de atestación de BotGuard). No se arregla reintentando,
ni re-resolviendo la URL, ni con `alr=yes`, ni cambiando cabeceras — todo eso
está probado y descartado en `crates/ytm-spike/src/bench.rs`.

La Fase 0 se validó con `dQw4w9WgXcQ`, que resulta ser una excepción y se
descarga entero. Fue un error de muestreo: se eligió por estar siempre
disponible, y esa misma propiedad lo hace anormalmente permisivo.

Para resolverlo hay que generar un poToken ejecutando el desafío JavaScript de
BotGuard. La buena noticia es que Tauri ya embebe un webview, así que se puede
hacer en una ventana oculta sin añadir dependencias nuevas. Es el siguiente
trabajo pendiente, y el más importante.

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

## Ejecutar

```bash
cd apps/desktop
npm install     # solo la primera vez
npm run tauri dev
```

`npm run tauri dev` levanta Vite y la aplicación juntos. `cargo run -p posible`
por su cuenta **no** basta: la app en modo desarrollo espera el servidor de Vite
en el puerto 1420.

Para generar un ejecutable de verdad:

```bash
npm run tauri build --prefix apps/desktop
```

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
- **El fondo no usa `backdrop-filter`.** La portada se pide a 48 px y se estira:
  el reescalado del navegador hace de desenfoque, y es gratis.
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
