# MyGlossMusic

MyGlossMusic es un reproductor de escritorio nativo, moderno y ultraligero para YouTube Music, construido con **Rust**, **Tauri v2** y **SolidJS**.

Diseñado para ofrecer una experiencia estética con interfaz translúcida (*glassmorphism*), iluminación ambiental dinámica que reacciona a la música, y un consumo de recursos mínimo frente a clientes tradicionales basados en Electron.

---

## Características Principales

* **Rendimiento Nativo**: Menos de 90 MB de memoria RAM y un uso de CPU inferior al 1.5% en reproducción activa.
* **Estética Ambiental Reactiva**: Fondo dinámico con mallas de color extraídas de las portadas en tiempo real y desenfoque acelerado por hardware.
* **Navegación Fluida**: Transición animada (FLIP) entre la vista completa del reproductor y el miniplayer flotante.
* **Búsqueda Inteligente**: Sugerencias en tiempo real, filtros por categorías (canciones, videos, álbumes, artistas) y carga continua.
* **Cola y Radio Automática**: Generación de mezclas continuas a partir de cualquier tema seleccionado.
* **Letras Sincronizadas**: Integración con desplazamiento suave y salto interactivo a cualquier punto de la pista.
* **Biblioteca y Listas Locales**: Gestión completa de playlists propias, canciones favoritas e historial almacenados localmente mediante SQLite.
* **Controles Multimedia Globales**: Soporte para teclas multimedia del sistema operativo (SMTC en Windows, MPRIS en Linux y CoreAudio en macOS).

---

## Sobre el Inicio de Sesión de YouTube (Modo Anónimo)

Una decisión arquitectónica deliberada de **MyGlossMusic** es operar actualmente en **Modo Anónimo por diseño**.

### ¿Por qué no incluye inicio de sesión con cuenta de Google?

1. **Protección de tu Cuenta**: El uso de credenciales o cookies en clientes no oficiales de terceros puede derivar en suspensiones o advertencias por parte de Google por uso de APIs no documentadas. Operar sin cuenta garantiza que tu perfil personal de Google permanezca completamente seguro e intocado.
2. **Privacidad Total**: No se recopilan datos de telemetría, no hay rastreadores publicitarios y tus hábitos de escucha se guardan de forma estrictamente local en tu computadora en una base de datos SQLite propia.
3. **Independencia**: Puedes crear listas de reproducción, marcar favoritos y explorar radios sin necesidad de vincular cuentas de correo ni depender de perfiles externos.

> En futuras versiones se evaluará el soporte de sesión opcional y aislado, garantizando siempre que el modo anónimo siga siendo el estándar de seguridad.

---

## Comparativa de Rendimiento

| Métrica | Cliente Web / Electron | MyGlossMusic |
| :--- | :--- | :--- |
| **Memoria RAM** | 400 MB – 650 MB | ~60 MB – 90 MB |
| **Uso de CPU (Segundo plano)** | 3% – 7% | < 0.8% |
| **Tamaño del instalador** | 120 MB – 180 MB | ~15 MB |
| **Motor de interfaz** | Chromium completo empaquetado | WebView nativo del sistema operativo |
| **Motor de audio** | Decodificación por navegador | Pipeline nativo en Rust (`symphonia` + `rodio`) |

---

## Instalación

### Descargas Precompiladas

Puedes descargar la última versión compilada para tu sistema operativo desde la sección de [Releases](https://github.com/jonatan-testiando/MyGlossMusic/releases):

* **Windows**: Instalador `.msi` o ejecutable portable `.exe`.
* **macOS**: Imagen de disco `.dmg` (versiones nativas para Apple Silicon M1/M2/M3/M4 y procesadores Intel).
* **Linux**: Paquetes `.AppImage` y `.deb`.

---

## Compilación desde Código Fuente

### Requisitos Previos

* [Node.js](https://nodejs.org/) (versión 20 o superior).
* [Rust](https://www.rust-lang.org/) (versión stable).
* En Linux: Dependencias de desarrollo de WebKitGTK (`libwebkit2gtk-4.0-dev`, `libasound2-dev`).

### Pasos

1. Clonar el repositorio:
   ```bash
   git clone https://github.com/jonatan-testiando/MyGlossMusic.git
   cd MyGlossMusic
   ```

2. Instalar dependencias del frontend:
   ```bash
   cd apps/desktop
   npm install
   ```

3. Ejecutar en modo desarrollo:
   ```bash
   npm run tauri dev
   ```

4. Compilar binario de producción:
   ```bash
   npm run tauri build
   ```

---

## Estructura del Proyecto

```
crates/
  ytm-source/     Motor de extracción y consultas a la API de YouTube Music
  ytm-audio/      Pipeline de reproducción de audio, caché progresivo y cola
apps/desktop/
  src-tauri/      Backend nativo en Rust (SQLite, paleta de colores, SMTC)
  src/            Frontend reactivo en SolidJS con Tailwind CSS
```

---

## Aviso Legal

Este proyecto ha sido desarrollado con fines educativos y de investigación sobre rendimiento en clientes de escritorio nativos. No está afiliado, respaldado ni asociado con Google LLC ni YouTube. Todas las marcas registradas pertenecen a sus respectivos propietarios.

## Licencia

Distribuido bajo licencia [GPLv3](LICENSE).
