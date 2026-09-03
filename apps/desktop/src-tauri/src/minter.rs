//! Acunador de URLs de audio mediante un webview oculto.
//!
//! # Por que existe esto
//!
//! googlevideo sirve solo ~1 MiB (~65 s) de la mayoria del contenido y devuelve
//! 403 para el resto si la URL no lleva un `poToken` valido, ademas de la firma
//! `sig` y el parametro `n` ya descifrados. Los tres los produce el JavaScript
//! del reproductor de YouTube, y no hay forma de generarlos desde Rust sin
//! reimplementar BotGuard entero.
//!
//! Medido en `ytm-spike rawurl`: una URL formada por el navegador SI se descarga
//! entera desde un cliente HTTP normal (3.3 MB a 21 MB/s). Asi que el webview
//! solo hace falta para **acunar la URL**; la descarga, la decodificacion y toda
//! la interfaz siguen siendo nativas.
//!
//! # Como devuelve el dato una pagina remota
//!
//! Una pagina de YouTube no tiene acceso al IPC de Tauri. El truco: el script de
//! inicializacion engancha `fetch`/`XHR`, y cuando ve una URL de googlevideo
//! navega a `ytmint://<url>`. Rust intercepta esa navegacion con
//! `on_navigation`, extrae el dato y la bloquea.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Esquema propio que usamos como canal de vuelta.
const SCHEME: &str = "ytmint://";

/// Margen antes de dar por fallida la acunacion. Cargar YouTube Music y que
/// arranque el reproductor tarda unos segundos en frio.
const TIMEOUT: Duration = Duration::from_secs(25);

/// Se inyecta ANTES de los scripts de la pagina.
///
/// Engancha las dos vias por las que el reproductor pide audio y avisa en cuanto
/// ve la primera URL de googlevideo. Tambien intenta pulsar "play", porque sin
/// interaccion el navegador puede bloquear la reproduccion automatica y entonces
/// no se pediria ningun segmento.
const HOOK: &str = r#"
(function () {
  // SILENCIO ABSOLUTO.
  //
  // El webview esta oculto pero reproduce audio de verdad: sin esto, cada
  // acunacion suelta un par de segundos de musica (y anuncios) por los
  // altavoces, sin ventana visible que lo explique. Pasa de verdad.
  //
  // No basta con poner `muted` en el elemento cuando lo encontramos: el
  // reproductor de la pagina puede empezar antes y puede volver a activarlo. Se
  // interceptan las propiedades en el prototipo, ANTES de que corra ningun
  // script de la pagina, para que ni siquiera se pueda desactivar el silencio.
  try {
    var P = HTMLMediaElement.prototype;
    var mutedDesc = Object.getOwnPropertyDescriptor(P, 'muted');
    var volDesc = Object.getOwnPropertyDescriptor(P, 'volume');

    Object.defineProperty(P, 'muted', {
      get: function () { return true; },
      set: function () { if (mutedDesc && mutedDesc.set) mutedDesc.set.call(this, true); },
      configurable: true,
    });
    Object.defineProperty(P, 'volume', {
      get: function () { return 0; },
      set: function () { if (volDesc && volDesc.set) volDesc.set.call(this, 0); },
      configurable: true,
    });

    var origPlay = P.play;
    P.play = function () {
      try {
        if (mutedDesc && mutedDesc.set) mutedDesc.set.call(this, true);
        if (volDesc && volDesc.set) volDesc.set.call(this, 0);
      } catch (e) {}
      return origPlay.apply(this, arguments);
    };

    // Red de seguridad para elementos creados despues.
    setInterval(function () {
      document.querySelectorAll('video,audio').forEach(function (el) {
        try {
          if (mutedDesc && mutedDesc.set) mutedDesc.set.call(el, true);
          if (volDesc && volDesc.set) volDesc.set.call(el, 0);
        } catch (e) {}
      });
    }, 200);

    // La Web Audio API es otra via de salida.
    if (window.AudioContext) {
      var OrigCtx = window.AudioContext;
      window.AudioContext = function () {
        var ctx = new OrigCtx(arguments[0]);
        try { ctx.suspend(); } catch (e) {}
        return ctx;
      };
    }
  } catch (e) {}

  // Intento de forzar AAC: se le dice a la pagina que este equipo no soporta
  // WebM, para que el reproductor elija mp4/AAC (itag 140), que symphonia si
  // decodifica. Solo tiene sentido en www.youtube.com; music.youtube.com sirve
  // el audio unicamente en WebM/Opus y bloquearlo deja al reproductor sin nada.
  if (window.__YTM_BLOCK_WEBM__) {
    try {
      if (window.MediaSource && MediaSource.isTypeSupported) {
        var oi = MediaSource.isTypeSupported;
        MediaSource.isTypeSupported = function (t) {
          if (typeof t === 'string' && t.indexOf('webm') !== -1) return false;
          return oi.apply(this, arguments);
        };
      }
      var proto = window.HTMLMediaElement && HTMLMediaElement.prototype;
      if (proto && proto.canPlayType) {
        var oc = proto.canPlayType;
        proto.canPlayType = function (t) {
          if (typeof t === 'string' && t.indexOf('webm') !== -1) return '';
          return oc.apply(this, arguments);
        };
      }
    } catch (e) {}
  }
  // MODO MEDICION (__YTM_OBSERVE__): no se para al primer URL. Se registran
  // todos los rangos que pide el reproductor durante unos segundos y se
  // informa del maximo alcanzado, para saber a que velocidad bufferiza.
  if (window.__YTM_OBSERVE__) {
    var maxEnd = 0, count = 0, t0 = Date.now(), firstUrl = null;
    function observe(url) {
      if (!url || url.indexOf('googlevideo.com') === -1) return;
      if (url.indexOf('mime=audio') === -1) return;
      count++;
      if (!firstUrl) firstUrl = url;
      var m = /[?&]range=(\d+)-(\d+)/.exec(url);
      if (m) { var e = parseInt(m[2], 10); if (e > maxEnd) maxEnd = e; }
    }
    var of0 = window.fetch;
    window.fetch = function (input) {
      try { observe(typeof input === 'string' ? input : (input && input.url)); } catch (e) {}
      return of0.apply(this, arguments);
    };
    var oo0 = XMLHttpRequest.prototype.open;
    XMLHttpRequest.prototype.open = function (m, url) {
      try { observe(url); } catch (e) {}
      return oo0.apply(this, arguments);
    };
    setInterval(function () {
      try {
        var v = document.querySelector('video');
        if (v && v.paused) { var p = v.play(); if (p && p.catch) p.catch(function () {}); }
        var btn = document.querySelector('#play-pause-button');
        if (btn && v && v.paused) btn.click();
      } catch (e) {}
    }, 400);
    // Muestras a 5, 10 y 20 s.
    [5000, 10000, 20000].forEach(function (ms) {
      setTimeout(function () {
        var v = document.querySelector('video');
        var buffered = 0;
        try { if (v && v.buffered.length) buffered = v.buffered.end(v.buffered.length - 1); } catch (e) {}
        var msg = 'OBS t=' + ms + 'ms count=' + count + ' maxEndKiB=' + Math.round(maxEnd / 1024)
          + ' bufferedS=' + buffered.toFixed(1) + ' curS=' + (v ? v.currentTime.toFixed(1) : '-');
        if (ms === 20000) location.href = 'ytmint://' + encodeURIComponent(msg);
        else console.log(msg);
      }, ms);
    });
    return;
  }

  var done = false;
  function report(url) {
    if (done || !url || url.indexOf('googlevideo.com') === -1) return;
    // Solo nos sirve una pista de AUDIO. La pagina tambien pide video.
    if (url.indexOf('mime=audio') === -1 && url.indexOf('mime=audio%2F') === -1) return;
    done = true;
    try {
      var u = new URL(url);
      // Se limpian tres grupos de parametros:
      //
      //  - range/rn/rbuf: son de la peticion concreta de la pagina; nosotros
      //    pedimos nuestros propios rangos.
      //  - ump/srfvp: activan el empaquetado UMP de YouTube, en el que el
      //    cuerpo NO son los bytes del medio sino tramas protobuf. Con ump=1 lo
      //    que se descarga son 6 bytes de estado, no audio.
      //  - alr: hace que ante una redireccion devuelva una URL en texto plano
      //    en vez de datos, lo que confunde al descargador.
      // range/rn/rbuf son de la peticion concreta de la pagina: nosotros
      // pedimos nuestros propios rangos.
      //
      // `ump=1` es critico: activa el empaquetado UMP de YouTube, en el que el
      // cuerpo NO son los bytes del medio sino tramas protobuf. Medido con
      // `ytm-spike strip`: con `ump` la respuesta empieza por `3a02...`; sin el,
      // por `1a45dfa3`, que es la firma de WebM. `srfvp` y `alr` se pueden
      // dejar: no cambian nada.
      ['range', 'rn', 'rbuf', 'ump'].forEach(function (k) { u.searchParams.delete(k); });
      // Cortar el consumo de la pagina ANTES de avisar.
      //
      // Mientras el reproductor de YouTube sigue pidiendo rangos de esta misma
      // URL, googlevideo rechaza con 403 a cualquier otro consumidor. Si no se
      // para aqui, nuestra descarga falla aunque la URL sea perfecta.
      try {
        document.querySelectorAll('video,audio').forEach(function (v) {
          v.pause();
          v.removeAttribute('src');
          v.load();
        });
      } catch (e) {}

      location.href = 'ytmint://' + encodeURIComponent(u.toString());
    } catch (e) {}
  }

  var of = window.fetch;
  window.fetch = function (input) {
    try { report(typeof input === 'string' ? input : (input && input.url)); } catch (e) {}
    return of.apply(this, arguments);
  };

  var oo = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (m, url) {
    try { report(url); } catch (e) {}
    return oo.apply(this, arguments);
  };

  // Algunos caminos crean el stream via MediaSource sin pasar por fetch/XHR.
  var interval = setInterval(function () {
    if (done) { clearInterval(interval); return; }
    try {
      var e = performance.getEntriesByType('resource');
      for (var i = 0; i < e.length; i++) {
        if (e[i].name.indexOf('googlevideo.com') !== -1) { report(e[i].name); break; }
      }
      // Empujar la reproduccion si sigue parada.
      var v = document.querySelector('video');
      if (v && v.paused) {
        var p = v.play();
        if (p && p.catch) p.catch(function () {});
        var btn = document.querySelector('#play-pause-button, .play-pause-button');
        if (btn) btn.click();
      }
    } catch (e) {}
  }, 400);
})();
"#;

/// Acuna la URL de audio de una pista.
///
/// Abre un webview oculto, deja que YouTube forme la URL y la devuelve. La
/// ventana se cierra siempre, tambien si algo falla.
pub async fn mint(app: &AppHandle, video_id: &str) -> Result<String> {
    mint_from(app, video_id, Origin::Music).await
}

/// De donde se acuna la URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// music.youtube.com. Sirve el audio solo en WebM/Opus.
    Music,
    /// www.youtube.com. Ofrece tambien mp4/AAC, pero NO FUNCIONA para acunar:
    /// no arranca la reproduccion sin un gesto real del usuario, asi que nunca
    /// llega a pedir un segmento y la acunacion caduca. Probado con y sin
    /// bloqueo de WebM. Se conserva por si cambia el comportamiento.
    Watch,
}

pub async fn mint_from(app: &AppHandle, video_id: &str, origin: Origin) -> Result<String> {
    // Etiqueta unica: dos acunaciones simultaneas no deben pisarse.
    let label = format!(
        "minter-{}-{}",
        video_id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );

    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let slot = Arc::new(Mutex::new(Some(tx)));

    let target = match origin {
        Origin::Music => format!("https://music.youtube.com/watch?v={video_id}"),
        Origin::Watch => format!("https://www.youtube.com/watch?v={video_id}"),
    };
    let url = target.parse().context("URL de YouTube Music invalida")?;

    let slot_for_nav = Arc::clone(&slot);
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title("")
        .visible(false)
        .focused(false)
        .skip_taskbar(true)
        .initialization_script(&format!(
            "window.__YTM_BLOCK_WEBM__ = {};
window.__YTM_OBSERVE__ = {};
{HOOK}",
            std::env::var("POSIBLE_BLOCK_WEBM").is_ok() && origin == Origin::Watch,
            std::env::var("POSIBLE_OBSERVE").is_ok()
        ))
        .on_navigation(move |url| {
            let s = url.as_str();
            if let Some(encoded) = s.strip_prefix(SCHEME) {
                let decoded = percent_decode(encoded);
                if let Some(tx) = slot_for_nav.lock().ok().and_then(|mut g| g.take()) {
                    let _ = tx.send(decoded);
                }
                return false; // no navegar: era solo el canal de vuelta
            }
            true
        })
        .build()
        .context("no se pudo crear el webview de acunacion")?;

    let limit = if std::env::var("POSIBLE_OBSERVE").is_ok() {
        Duration::from_secs(40)
    } else {
        TIMEOUT
    };
    let minted = tokio::time::timeout(limit, rx).await;

    // La ventana se cierra pase lo que pase.
    let _ = window.close();
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.destroy();
    }
    // Margen para que el webview suelte de verdad sus conexiones: mientras siga
    // pidiendo rangos de la misma URL, googlevideo nos devuelve 403.
    tokio::time::sleep(Duration::from_millis(500)).await;

    match minted {
        Ok(Ok(url)) if url.contains("googlevideo.com") => {
            tracing::info!(video_id, "URL acunada");
            Ok(url)
        }
        Ok(Ok(msg)) if msg.starts_with("OBS ") => {
            tracing::info!(%msg, "OBSERVACION");
            anyhow::bail!("modo observacion: {msg}")
        }
        Ok(Ok(_)) => anyhow::bail!("el webview devolvio una URL que no es de googlevideo"),
        Ok(Err(_)) => anyhow::bail!("el webview se cerro sin devolver la URL"),
        Err(_) => anyhow::bail!(
            "tiempo agotado acunando la URL de {video_id}: YouTube no formo el stream en {} s",
            TIMEOUT.as_secs()
        ),
    }
}

/// Decodifica el porcentaje de una URL. Es todo lo que necesitamos: el script
/// manda `encodeURIComponent`, asi que solo hay `%XX` y nada de `+`.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodifica_el_porcentaje() {
        assert_eq!(
            percent_decode("https%3A%2F%2Fa.com%2Fb%3Fc%3D1%26d%3D2"),
            "https://a.com/b?c=1&d=2"
        );
    }

    #[test]
    fn deja_intacto_lo_que_no_esta_codificado() {
        assert_eq!(percent_decode("https://a.com/b"), "https://a.com/b");
    }

    #[test]
    fn tolera_un_porcentaje_suelto_al_final() {
        assert_eq!(percent_decode("abc%"), "abc%");
        assert_eq!(percent_decode("abc%z"), "abc%z");
    }
}
