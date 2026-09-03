//! Integracion con los controles multimedia del sistema.
//!
//! En Windows expone la tarjeta de "reproduciendo ahora" (SMTC) y hace que las
//! teclas multimedia del teclado funcionen incluso con la aplicacion en segundo
//! plano. En Linux hace lo propio via MPRIS y en macOS via el centro de control.
//!
//! # Por que todo pasa por el hilo principal
//!
//! `MediaControls` NO es `Send` en Windows: contiene objetos COM ligados al hilo
//! que los creo y a su bucle de mensajes. Por eso vive en un `thread_local` del
//! hilo principal y toda actualizacion se encola con
//! `AppHandle::run_on_main_thread`.
//!
//! Todos los fallos aqui se registran y se ignoran: las teclas multimedia son un
//! extra, y no funcionar no debe impedir que suene la musica.

use std::cell::RefCell;

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition};
use tauri::{AppHandle, Emitter, Manager};
use ytm_audio::{Command, Engine, PlaybackState};

thread_local! {
    /// Solo se toca desde el hilo principal (ver nota del modulo).
    static CONTROLS: RefCell<Option<MediaControls>> = const { RefCell::new(None) };
}

/// Crea los controles del sistema. Debe llamarse desde el hilo principal.
pub fn init(app: &AppHandle, engine: Engine) {
    let Some(window) = app.get_webview_window("main") else {
        tracing::warn!("sin ventana principal: no hay controles multimedia");
        return;
    };

    #[cfg(target_os = "windows")]
    let hwnd = match window.hwnd() {
        Ok(h) => Some(h.0 as *mut std::ffi::c_void),
        Err(e) => {
            tracing::warn!(error = %e, "sin HWND: no hay controles multimedia");
            return;
        }
    };
    #[cfg(not(target_os = "windows"))]
    let hwnd = {
        let _ = &window;
        None
    };

    let config = souvlaki::PlatformConfig {
        display_name: "Posible",
        dbus_name: "posible",
        hwnd,
    };

    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = ?e, "no se pudieron crear los controles multimedia");
            return;
        }
    };

    // Las teclas del teclado y los botones de la tarjeta del sistema entran por
    // aqui y se traducen a ordenes del motor.
    let engine_for_events = engine.clone();
    let handle = app.clone();
    let result = controls.attach(move |event: MediaControlEvent| {
        match event {
            MediaControlEvent::Play => engine_for_events.send(Command::Resume),
            MediaControlEvent::Pause => engine_for_events.send(Command::Pause),
            MediaControlEvent::Toggle => engine_for_events.send(Command::TogglePlay),
            MediaControlEvent::Next => engine_for_events.send(Command::Next),
            MediaControlEvent::Previous => engine_for_events.send(Command::Prev),
            MediaControlEvent::Stop => engine_for_events.send(Command::Stop),
            MediaControlEvent::SetPosition(MediaPosition(pos)) => {
                engine_for_events.send(Command::Seek(pos))
            }
            MediaControlEvent::Quit => {
                let _ = handle.emit("quit-requested", ());
            }
            _ => {}
        }
    });

    if let Err(e) = result {
        tracing::warn!(error = ?e, "no se pudo enganchar el manejador multimedia");
        return;
    }

    CONTROLS.with(|c| *c.borrow_mut() = Some(controls));
    tracing::info!("controles multimedia del sistema activos");
}

/// Refleja el estado del reproductor en la tarjeta del sistema.
///
/// Debe ejecutarse en el hilo principal.
pub fn update(state: &PlaybackState) {
    CONTROLS.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let Some(controls) = borrow.as_mut() else {
            return;
        };

        if let Some(track) = &state.track {
            let _ = controls.set_metadata(MediaMetadata {
                title: Some(&track.title),
                artist: Some(&track.author),
                album: None,
                duration: (state.duration_ms > 0)
                    .then(|| std::time::Duration::from_millis(state.duration_ms)),
                cover_url: track.thumbnail.as_deref(),
            });
        }

        let progress = Some(MediaPosition(std::time::Duration::from_millis(
            state.position_ms,
        )));
        let playback = if state.track.is_none() {
            MediaPlayback::Stopped
        } else if state.playing {
            MediaPlayback::Playing { progress }
        } else {
            MediaPlayback::Paused { progress }
        };
        let _ = controls.set_playback(playback);
    });
}
