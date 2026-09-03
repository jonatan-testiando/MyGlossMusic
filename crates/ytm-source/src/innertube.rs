//! Cliente del endpoint `player` de InnerTube.
//!
//! REGLA DE ORO DEL PROYECTO: este cliente es ANONIMO. Nunca lleva cookies ni
//! sesion. El plano de biblioteca (playlists, likes) usa un cliente HTTP
//! completamente distinto, con `WEB_REMIX` y cookies reales. Mezclar cookies con
//! un cliente suplantado es lo unico que pone en riesgo la cuenta del usuario.

use anyhow::{Context, Result};
use serde_json::json;

use crate::clients::ClientConfig;
use crate::model::PlayerResponse;

/// Atestacion de sesion: el par que googlevideo exige para servir una pista
/// entera. Se acuna en un navegador y va siempre junto: el token esta ligado al
/// `visitor_data` con el que se genero.
#[derive(Debug, Clone)]
pub struct Attestation {
    pub visitor_data: String,
    pub po_token: String,
}

const PLAYER_URL: &str = "https://www.youtube.com/youtubei/v1/player";
const MUSIC_PLAYER_URL: &str = "https://music.youtube.com/youtubei/v1/player";

/// Cliente HTTP anonimo para extraccion de streams.
#[derive(Clone)]
pub struct InnerTube {
    http: reqwest::Client,
}

impl InnerTube {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            // Sin cookie store: cualquier cookie que llegara aqui seria un bug.
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .context("no se pudo construir el cliente HTTP")?;
        Ok(Self { http })
    }

    /// Cliente HTTP interno, para otros endpoints anonimos (busqueda).
    pub(crate) fn http_ref(&self) -> &reqwest::Client {
        &self.http
    }

    /// Pide la respuesta del reproductor para un video usando un cliente concreto.
    pub async fn player(&self, video_id: &str, client: ClientConfig) -> Result<PlayerResponse> {
        self.player_with(video_id, client, None).await
    }

    /// Como [`Self::player`] pero adjuntando una atestacion de sesion.
    ///
    /// Sin ella googlevideo sirve solo ~1 MiB de la mayoria del contenido y
    /// devuelve 403 para el resto. El `visitor_data` DEBE ser el mismo con el
    /// que se acuno el `po_token`: el token esta ligado a esa sesion.
    pub async fn player_with(
        &self,
        video_id: &str,
        client: ClientConfig,
        session: Option<&Attestation>,
    ) -> Result<PlayerResponse> {
        let mut context = json!({
            "client": {
                "clientName": client.client_name,
                "clientVersion": client.client_version,
                "hl": "en",
                "gl": "US",
            }
        });

        if client.embedded {
            context["thirdParty"] = json!({ "embedUrl": "https://www.youtube.com/" });
        }
        if let Some(s) = session {
            context["client"]["visitorData"] = json!(s.visitor_data);
        }

        let mut body = json!({
            "context": context,
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true,
        });
        if let Some(s) = session {
            body["serviceIntegrityDimensions"] = json!({ "poToken": s.po_token });
        }

        // Los clientes de YouTube Music viven en su propio origen. Pedirles el
        // reproductor desde www.youtube.com devuelve UNPLAYABLE.
        let is_music = client.client_name.contains("MUSIC") || client.client_name == "WEB_REMIX";
        let (endpoint, origin) = if is_music {
            (MUSIC_PLAYER_URL, "https://music.youtube.com")
        } else {
            (PLAYER_URL, "https://www.youtube.com")
        };

        let res = self
            .http
            .post(endpoint)
            .header("User-Agent", client.user_agent)
            .header("X-YouTube-Client-Name", client.client_name_id.to_string())
            .header("X-YouTube-Client-Version", client.client_version)
            .header("Content-Type", "application/json")
            .header("Origin", origin)
            .header("Referer", format!("{origin}/"))
            .json(&body)
            .send()
            .await
            .with_context(|| format!("fallo la peticion player con cliente {}", client.id))?;

        let status = res.status();
        let text = res.text().await.context("no se pudo leer la respuesta")?;

        anyhow::ensure!(
            status.is_success(),
            "cliente {} devolvio HTTP {}: {}",
            client.id,
            status,
            text.chars().take(300).collect::<String>()
        );

        serde_json::from_str(&text)
            .with_context(|| format!("no se pudo parsear la respuesta de {}", client.id))
    }
}
