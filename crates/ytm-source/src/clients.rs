//! Tabla de clientes InnerTube.
//!
//! Cada entrada describe un cliente de YouTube que podemos suplantar. Google
//! cierra clientes cada pocos meses, asi que esto es DATO, no logica: cuando uno
//! deja de funcionar se edita la tabla y se reordena `PREFERRED`, sin tocar
//! ninguna otra parte del proyecto.
//!
//! Ejecuta `ytm-spike probe <videoId>` para ver cuales siguen vivos hoy.

/// Configuracion de un cliente InnerTube.
#[derive(Debug, Clone, Copy)]
pub struct ClientConfig {
    /// Nombre corto que usamos nosotros en logs y CLI.
    pub id: &'static str,
    /// Valor de `context.client.clientName`.
    pub client_name: &'static str,
    /// Valor de `context.client.clientVersion`.
    pub client_version: &'static str,
    /// Cabecera `X-YouTube-Client-Name` (id numerico interno de YouTube).
    pub client_name_id: u8,
    /// User-Agent. Debe ser COHERENTE con el cliente declarado: la incoherencia
    /// es lo que dispara los flags anti-bot, no la suplantacion en si.
    pub user_agent: &'static str,
    /// Algunos clientes solo devuelven streams si el video se pide como
    /// "incrustado", lo que requiere estos campos extra en el contexto.
    pub embedded: bool,
    /// `true` si este cliente suele devolver URLs directas (sin `signatureCipher`),
    /// es decir, sin necesitar un motor JavaScript para descifrar.
    pub expects_direct_urls: bool,
}

/// Cliente de la app de iOS. Historicamente el mas fiable para URLs directas.
pub const IOS: ClientConfig = ClientConfig {
    id: "ios",
    client_name: "IOS",
    client_version: "20.10.4",
    client_name_id: 5,
    user_agent: "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)",
    embedded: false,
    expects_direct_urls: true,
};

/// Cliente de YouTube VR para Quest. Poco vigilado, sin requisito de poToken
/// durante mucho tiempo.
pub const ANDROID_VR: ClientConfig = ClientConfig {
    id: "android_vr",
    client_name: "ANDROID_VR",
    client_version: "1.62.27",
    client_name_id: 28,
    user_agent: "com.google.android.apps.youtube.vr.oculus/1.62.27 (Linux; U; Android 12; GB) gzip",
    embedded: false,
    expects_direct_urls: true,
};

/// Cliente de televisores. Suele funcionar cuando los moviles fallan.
pub const TV: ClientConfig = ClientConfig {
    id: "tv",
    client_name: "TVHTML5",
    client_version: "7.20250219.14.00",
    client_name_id: 7,
    user_agent: "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/Version",
    embedded: false,
    expects_direct_urls: false,
};

/// Reproductor incrustado de television. El clasico fallback para videos con
/// restricciones.
pub const TV_EMBEDDED: ClientConfig = ClientConfig {
    id: "tv_embedded",
    client_name: "TVHTML5_SIMPLY_EMBEDDED_PLAYER",
    client_version: "2.0",
    client_name_id: 85,
    user_agent: "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/Version",
    embedded: true,
    expects_direct_urls: false,
};

/// El cliente que usa music.youtube.com de verdad. Es el que usaremos para el
/// PLANO DE BIBLIOTECA (con cookies), donde su trafico es indistinguible del
/// navegador. Para streams normalmente exige descifrado, asi que va el ultimo.
pub const WEB_REMIX: ClientConfig = ClientConfig {
    id: "web_remix",
    client_name: "WEB_REMIX",
    client_version: "1.20250310.01.00",
    client_name_id: 67,
    user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
    embedded: false,
    expects_direct_urls: false,
};

/// Web movil. Fallback adicional.
pub const MWEB: ClientConfig = ClientConfig {
    id: "mweb",
    client_name: "MWEB",
    client_version: "2.20250310.01.00",
    client_name_id: 2,
    user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_3_2 like Mac OS X) \
                 AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.3 Mobile/15E148 Safari/604.1",
    embedded: false,
    expects_direct_urls: false,
};

/// Orden de preferencia para reproducir. El primero que devuelva un stream
/// usable, gana. Reordena esta lista segun lo que diga `probe`.
pub const PREFERRED: &[ClientConfig] = &[IOS, ANDROID_VR, TV_EMBEDDED, TV, MWEB, WEB_REMIX];

/// Todos los clientes conocidos, para el modo `probe`.
pub const ALL: &[ClientConfig] = &[IOS, ANDROID_VR, TV, TV_EMBEDDED, MWEB, WEB_REMIX];

/// Busca un cliente por su `id` corto.
pub fn by_id(id: &str) -> Option<ClientConfig> {
    ALL.iter().copied().find(|c| c.id == id)
}
