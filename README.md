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
npm install --prefix apps/desktop
cargo run -p posible
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
