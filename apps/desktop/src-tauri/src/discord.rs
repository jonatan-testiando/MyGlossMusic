//! Discord Rich Presence.
//!
//! Muestra la cancion actual en el perfil de Discord. Es completamente
//! opcional: si Discord no esta abierto, la conexion falla y se reintenta de
//! vez en cuando sin molestar a nadie.
//!
//! Corre en su propia tarea y solo recibe estado por un canal `watch`, asi que
//! un Discord colgado no puede bloquear la reproduccion.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use ytm_audio::PlaybackState;

/// Id de aplicacion de Discord. Sin uno propio, Discord no muestra nada.
///
/// Este es un marcador: crea el tuyo en https://discord.com/developers y
/// sustituyelo, o deja la variable de entorno `POSIBLE_DISCORD_APP_ID`.
const DEFAULT_APP_ID: &str = "1313131313131313131";

/// Reintento tras un fallo de conexion. Discord puede abrirse en cualquier
/// momento, pero no merece la pena insistir cada segundo.
const RECONNECT: Duration = Duration::from_secs(30);

pub fn spawn(mut rx: tokio::sync::watch::Receiver<PlaybackState>) {
    let app_id =
        std::env::var("POSIBLE_DISCORD_APP_ID").unwrap_or_else(|_| DEFAULT_APP_ID.to_string());

    tauri::async_runtime::spawn(async move {
        let mut client: Option<DiscordIpcClient> = None;
        let mut last_key = String::new();
        let mut last_attempt = std::time::Instant::now() - RECONNECT;

        while rx.changed().await.is_ok() {
            let state = rx.borrow().clone();

            // Solo se habla con Discord cuando cambia algo visible: pista,
            // pausa o salto. Si no, se enviarian 10 actualizaciones por segundo.
            let key = format!(
                "{}|{}|{}",
                state.track.as_ref().map(|t| t.video_id.as_str()).unwrap_or(""),
                state.playing,
                state.position_ms / 5_000
            );
            if key == last_key {
                continue;
            }
            last_key = key;

            if client.is_none() {
                if last_attempt.elapsed() < RECONNECT {
                    continue;
                }
                last_attempt = std::time::Instant::now();
                let mut c = DiscordIpcClient::new(&app_id);
                if c.connect().is_ok() {
                    tracing::info!("Discord Rich Presence conectado");
                    client = Some(c);
                } else {
                    tracing::debug!("Discord no disponible");
                }
            }

            let Some(c) = client.as_mut() else { continue };

            let ok = match &state.track {
                None => c.clear_activity().is_ok(),
                Some(track) => {
                    let details = truncate(&track.title, 128);
                    let party = truncate(&track.author, 128);
                    let thumb = track.thumbnail.clone().unwrap_or_default();

                    let mut assets = Assets::new().large_text(&party);
                    if !thumb.is_empty() {
                        assets = assets.large_image(&thumb);
                    }

                    let mut activity = Activity::new()
                        .details(&details)
                        .state(&party)
                        .assets(assets);

                    // La barra de tiempo de Discord se deduce de un instante de
                    // fin: se calcula a partir de lo que queda de cancion.
                    let ts;
                    if state.playing && state.duration_ms > 0 {
                        let remaining = state.duration_ms.saturating_sub(state.position_ms);
                        let end = now_secs() + (remaining / 1000) as i64;
                        ts = Timestamps::new().end(end);
                        activity = activity.timestamps(ts);
                    }

                    c.set_activity(activity).is_ok()
                }
            };

            if !ok {
                // La conexion se cayo (Discord cerrado). Se reintenta luego.
                tracing::debug!("se perdio la conexion con Discord");
                client = None;
                last_attempt = std::time::Instant::now();
            }
        }
    });
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // Se corta por limite de caracter para no partir un UTF-8 por la mitad.
    let cut = s
        .char_indices()
        .take_while(|(i, _)| *i < max - 1)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    format!("{}…", &s[..cut])
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
