//! Actualizacion en dos tiempos: se descarga sola, la instalas tu.
//!
//! # Por que dos tiempos y no un boton que lo hace todo
//!
//! Porque lo que se hace largo es la descarga, no la instalacion. Un unico
//! "actualizar" deja al usuario mirando una barra con la aplicacion a punto de
//! cerrarse, y en un reproductor eso es peor que en otro programa: la musica se
//! corta mientras espera.
//!
//! Aqui la comprobacion y la descarga van solas en segundo plano poco despues
//! de abrir, sin interrumpir nada. Cuando el paquete esta bajado y con la firma
//! verificada, la interfaz lo ensena discretamente. Al pulsar "instalar" ya no
//! queda nada que bajar: son unos segundos de instalador silencioso y la
//! aplicacion vuelve sola.
//!
//! # Por que no se instala al cerrar, que seria lo comodo
//!
//! Con `installMode: "quiet"` el plugin manda siempre `/S /R` al instalador de
//! NSIS, y esa `/R` relanza la aplicacion al terminar. Instalar al salir la
//! resucitaria justo despues de haberla cerrado. Quitar la `/R` obliga a montar
//! los argumentos a mano (`installMode: "basicUi"` + `installerArgs`), y su
//! orden en la linea de comandos no se puede comprobar sin publicar una release
//! de verdad.
//!
//! # Silencio ante los fallos
//!
//! Nada de esto devuelve error a la interfaz cuando corre en segundo plano. Sin
//! red, con el endpoint caido o con una release a medio publicar, lo correcto es
//! callarse: que no haya actualizacion no es un problema del usuario, y el
//! primer objetivo del proyecto es que la aplicacion no se rompa.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Margen antes de mirar si hay actualizacion.
///
/// Arrancar ya mueve red de sobra —el feed de inicio, las portadas, la primera
/// pista— y la actualizacion es lo menos urgente de todo lo que pasa al abrir.
const MARGEN_DE_ARRANQUE: Duration = Duration::from_secs(20);

/// Margen entre parar la musica y lanzar el instalador.
///
/// El instalador de NSIS tiene que poder sobrescribir los archivos de la
/// carpeta de instalacion, y `yt-dlp.exe` es uno de ellos. Parar la
/// reproduccion evita que empiecen descargas nuevas; esta pausa le da a las que
/// estuvieran a medias el momento de terminar.
const MARGEN_ANTES_DE_INSTALAR: Duration = Duration::from_millis(1500);

/// Un paquete ya descargado y con la firma verificada, esperando a instalarse.
struct Pendiente {
    update: Update,
    bytes: Vec<u8>,
}

#[derive(Default)]
pub struct EstadoActualizacion {
    pendiente: Mutex<Option<Pendiente>>,
    /// Hay una descarga en marcha. Sin esto, pulsar "buscar actualizaciones"
    /// mientras la de fondo sigue bajando lanza una segunda descarga entera.
    descargando: AtomicBool,
}

/// Suelta la marca de "descargando" pase lo que pase, tambien si la descarga
/// falla a la mitad.
struct Testigo<'a>(&'a AtomicBool);

impl Drop for Testigo<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Lo que la interfaz necesita saber de una actualizacion lista.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Disponible {
    pub version: String,
    pub notes: Option<String>,
}

/// Progreso de la descarga en segundo plano.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progreso {
    percent: u8,
    version: String,
}

/// Arranca la comprobacion y la descarga en segundo plano.
pub fn comprobar_en_segundo_plano(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(MARGEN_DE_ARRANQUE).await;
        match comprobar_y_descargar(&app).await {
            Ok(Some(v)) => tracing::info!(version = %v, "actualizacion descargada y lista"),
            Ok(None) => tracing::debug!("no hay actualizacion pendiente"),
            Err(e) => tracing::warn!(error = %e, "no se pudo preparar la actualizacion"),
        }
    });
}

async fn comprobar_y_descargar(app: &AppHandle) -> Result<Option<String>, String> {
    let estado = app.state::<EstadoActualizacion>();

    // Si ya hay una lista de una comprobacion anterior, no se vuelve a bajar.
    if estado.pendiente.lock().unwrap().is_some() {
        return Ok(None);
    }
    if estado
        .descargando
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(None); // ya hay una descarga en marcha
    }
    let _testigo = Testigo(&estado.descargando);

    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(None);
    };

    let version = update.version.clone();
    let notes = update.body.clone();

    // El progreso se emite solo cuando cambia el porcentaje ENTERO. Por trozo
    // serian miles de eventos para pintar exactamente la misma barra, y cada uno
    // despierta la interfaz.
    let mut bajado: u64 = 0;
    let mut ultimo: u8 = u8::MAX;
    let app_progreso = app.clone();
    let version_progreso = version.clone();

    let bytes = update
        .download(
            move |trozo, total| {
                bajado += trozo as u64;
                let Some(total) = total.filter(|t| *t > 0) else {
                    return;
                };
                let porcentaje = ((bajado * 100) / total).min(100) as u8;
                if porcentaje != ultimo {
                    ultimo = porcentaje;
                    let _ = app_progreso.emit(
                        "actualizacion-progreso",
                        Progreso {
                            percent: porcentaje,
                            version: version_progreso.clone(),
                        },
                    );
                }
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    app.state::<EstadoActualizacion>()
        .pendiente
        .lock()
        .unwrap()
        .replace(Pendiente { update, bytes });

    let _ = app.emit(
        "actualizacion-lista",
        Disponible {
            version: version.clone(),
            notes,
        },
    );
    Ok(Some(version))
}

/// Que hay descargado y esperando, si es que hay algo.
///
/// La interfaz pregunta ademas de escuchar el evento: el paquete puede haberse
/// descargado antes de que se montara el componente que lo pinta.
#[tauri::command]
pub fn update_pending(estado: tauri::State<'_, EstadoActualizacion>) -> Option<Disponible> {
    estado
        .pendiente
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| Disponible {
            version: p.update.version.clone(),
            notes: p.update.body.clone(),
        })
}

/// Comprobacion a peticion del usuario, desde Ajustes. Si encuentra algo, lo
/// deja descargado y lo devuelve.
///
/// Esta si devuelve el error: aqui lo ha pedido una persona que esta mirando, y
/// un boton que no hace nada sin decir por que es peor que un mensaje feo.
#[tauri::command]
pub async fn update_check_now(app: AppHandle) -> Result<Option<Disponible>, String> {
    comprobar_y_descargar(&app).await?;
    Ok(update_pending(app.state::<EstadoActualizacion>()))
}

/// Instala lo ya descargado.
///
/// En Windows esto **no vuelve**: el plugin lanza el instalador y mata el
/// proceso; la `/R` de NSIS relanza la aplicacion al terminar. Por eso se para
/// la musica antes, y no despues.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    let pendiente = app
        .state::<EstadoActualizacion>()
        .pendiente
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| "no hay ninguna actualizacion descargada".to_string())?;

    // Parar la musica no es cosmetico: mientras suena puede haber un `yt-dlp`
    // escribiendo en la carpeta de instalacion, y el instalador necesita poder
    // sobrescribir ese mismo archivo.
    if let Some(estado) = app.try_state::<crate::App>() {
        estado.engine.send(ytm_audio::Command::Stop);
    }
    tokio::time::sleep(MARGEN_ANTES_DE_INSTALAR).await;

    pendiente
        .update
        .install(&pendiente.bytes)
        .map_err(|e| e.to_string())
}
