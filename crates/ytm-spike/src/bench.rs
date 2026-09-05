//! Comparativa de estrategias de descarga sobre la misma URL.
//!
//! YouTube estrangula el GET completo de una URL cuyo parametro `n` no ha sido
//! descifrado, pero historicamente no aplica el mismo limite a las peticiones
//! por trozos. Si eso se confirma, el streaming por rangos que necesitamos de
//! todas formas en la Fase 1 esquiva el throttling sin motor JavaScript.

use anyhow::Result;
use std::time::{Duration, Instant};
use ytm_source::InnerTube;

const CHUNK: u64 = 1_048_576; // 1 MiB

/// Prueba SOLO un GET completo sobre una URL recien resuelta y sin tocar.
///
/// Importa que sea lo primero que se hace: si antes se gasta una peticion, el
/// contenido con licencia ya devuelve 403 y la medida no vale.
/// Prueba combinaciones de parametros de query en peticiones SECUENCIALES.
///
/// El reproductor web real no pide un rango a secas: manda `range`, un numero
/// de peticion `rn` que incrementa, y `rbuf`. La hipotesis es que googlevideo
/// rechaza como repeticion cualquier peticion que no incremente `rn`.
/// Inspecciona el cuerpo que devuelve `alr=yes` y prueba re-resolver por trozo.
/// Sigue la URL de respaldo que devuelve `alr=yes` y continua la descarga.
///
/// Es el mecanismo propio de googlevideo: cuando quiere redirigir, en vez de
/// datos devuelve una URL nueva en texto plano. Seguirla es lo que hace el
/// reproductor real.
/// Mide cuantos trozos de 1 MiB se pueden descargar antes del primer 403.
///
/// Sirve para distinguir contenido que se descarga entero de contenido que
/// corta a los ~65 s (1 MiB en itag 140).
/// Mide, POR CLIENTE, cuanto audio se puede descargar antes del primer 403.
///
/// Es la prueba que decide si hace falta poToken: si algun cliente entrega la
/// pista entera, no hace falta tocar BotGuard.
/// Prueba si una atestacion de sesion (visitorData + poToken) desbloquea la
/// descarga completa.
///
/// Es la prueba que decide toda la arquitectura: si funciona, hace falta acunar
/// el token en un webview oculto; si no, no hay camino sin reimplementar
/// BotGuard entero.
///
/// # RESPUESTA: no funciona (medido el 2026-09-05)
///
/// Se acuno un par real (`visitorData` + `pot`) en una sesion anonima de
/// music.youtube.com y se probo sobre dos pistas de YouTube Music que SIN
/// atestacion cortan a 1 MiB (`clientlimits` las da `CORTADO`, 1024 de 3807 KB
/// y de 4584 KB). Con la atestacion cortan exactamente igual, y la columna
/// `POT` sale `no` en todos los clientes: YouTube ni siquiera adjunta el token
/// a las URLs que devuelve, es decir, lo ignora.
///
/// La misma pista pedida con la URL ENTERA formada por el navegador se
/// descarga completa (4 028 192 de 4 028 192 bytes). Asi que el problema no es
/// que el token no sirva, sino que **la autorizacion no es un credencial
/// portable**: `sig`, `lsig`, `ns` y `pot` se acunan juntos para un cliente y
/// una sesion concretos, y no se pueden trasplantar a la URL de otro cliente.
///
/// Conclusion practica: no hay atajo de "un token por sesion y ya resolvemos
/// nativo". Lo unico que entrega pistas enteras sigue siendo traer la URL
/// completa de un navegador (el acunador) o delegar en yt-dlp. Ver [`graft`],
/// que prueba el otro angulo y falla igual.
/// Descarga entera una URL de googlevideo YA FORMADA (por ejemplo, capturada de
/// un navegador real). Sirve para comprobar si una URL con `pot` y firma valida
/// se puede consumir desde un cliente HTTP normal.
/// Detalle crudo de lo que devuelve cada cliente con atestacion.
///
/// Decide la arquitectura: si algun cliente devuelve URLs DIRECTAS (sin
/// `signatureCipher`) al mandarle el poToken, basta acunar el token una vez por
/// sesion. Si todas vienen cifradas, hace falta descifrar la firma, que solo se
/// puede hacer ejecutando el JavaScript del reproductor.
/// Prueba si basta con PEGAR el `pot` a una URL de cliente movil.
///
/// `pot` no aparece en `sparams`, es decir, no forma parte de la firma. Si
/// googlevideo lo valida por separado, se puede acunar un token una sola vez y
/// reutilizarlo con las URLs de `android`, que si sirven AAC (itag 140) y nos
/// ahorran tener que decodificar Opus.
///
/// # RESPUESTA: no basta (medido el 2026-09-05)
///
/// Con un `pot` acunado de verdad en el navegador, `android` e `ios` cortan en
/// el mismo sitio con el token pegado y sin el: 1024 KB de 3807. Que no este
/// en `sparams` no significa que se valide por separado; significa que
/// googlevideo lo comprueba contra la sesion que lo emitio. Ver [`attest`].
pub async fn graft(video_id: &str, visitor_data: &str, po_token: &str) -> Result<()> {
    let it = InnerTube::new()?;
    let client = reqwest::Client::new();
    let att = ytm_source::Attestation {
        visitor_data: visitor_data.to_string(),
        po_token: po_token.to_string(),
    };

    println!("
  Injertando `pot` en URLs de cliente movil - {video_id}
");
    println!("  {:<24} {:>7} {:>9} {:>9}  {}", "CLIENTE / VARIANTE", "TROZOS", "KB", "ESPERADO", "VEREDICTO");
    println!("  {}", "-".repeat(74));

    for cfg in [ytm_source::clients::ANDROID, ytm_source::clients::IOS] {
        // Con la atestacion tambien en el cuerpo de la peticion, por si el
        // token debe estar ligado al mismo visitorData.
        let Ok(res) = it.player_with(video_id, cfg, Some(&att)).await else { continue };
        let Some(streaming) = res.streaming_data else { continue };
        let Some(fmt) = ytm_source::select::best_audio(&streaming.adaptive_formats) else { continue };
        let Some(base) = fmt.url.clone() else { continue };
        let expected = fmt.content_length_bytes().unwrap_or(0);

        for (variante, url) in [
            ("sin pot", base.clone()),
            ("con &pot=", format!("{base}&pot={po_token}")),
        ] {
            let mut got = 0u64;
            let mut chunks = 0;
            loop {
                let r = client
                    .get(&url)
                    .header("Range", format!("bytes={got}-{}", got + CHUNK - 1))
                    .send()
                    .await;
                match r {
                    Ok(resp) if resp.status().is_success() => {
                        let n = resp.bytes().await.map(|b| b.len() as u64).unwrap_or(0);
                        if n == 0 { break; }
                        got += n;
                        chunks += 1;
                        if n < CHUNK || (expected > 0 && got >= expected) { break; }
                    }
                    _ => break,
                }
            }
            let ok = expected > 0 && got >= expected;
            println!(
                "  {:<24} {chunks:>7} {:>9} {:>9}  {}",
                format!("{} / {}", cfg.id, variante),
                got / 1024,
                expected / 1024,
                if ok { "COMPLETO" } else { "CORTADO" }
            );
        }
    }
    println!();
    Ok(())
}

pub async fn attest_detail(video_id: &str, visitor_data: &str, po_token: &str) -> Result<()> {
    let it = InnerTube::new()?;
    let att = ytm_source::Attestation {
        visitor_data: visitor_data.to_string(),
        po_token: po_token.to_string(),
    };

    println!("
  Detalle con atestacion - {video_id}
");
    for cfg in ytm_source::clients::ALL {
        let res = match it.player_with(video_id, *cfg, Some(&att)).await {
            Ok(r) => r,
            Err(e) => {
                println!("  {:<15} error: {}", cfg.id, e.to_string().chars().take(50).collect::<String>());
                continue;
            }
        };
        let status = res
            .playability_status
            .as_ref()
            .and_then(|p| p.status.clone())
            .unwrap_or_else(|| "?".into());
        let formats = res.streaming_data.map(|s| s.adaptive_formats).unwrap_or_default();
        let audio: Vec<_> = formats.iter().filter(|f| f.is_audio()).collect();
        let directas = audio.iter().filter(|f| f.url.is_some()).count();
        let cifradas = audio.iter().filter(|f| f.signature_cipher.is_some()).count();
        let con_pot = audio
            .iter()
            .filter(|f| f.url.as_deref().is_some_and(|u| u.contains("pot=")))
            .count();

        println!(
            "  {:<15} {:<16} audio={:<3} directas={:<3} cifradas={:<3} con_pot={}",
            cfg.id, status, audio.len(), directas, cifradas, con_pot
        );
    }
    println!();
    Ok(())
}

/// Sobre una URL ya acunada, prueba que pasa al quitar ciertos parametros.
///
/// `ump=1` hace que googlevideo devuelva tramas UMP (protobuf) en vez de los
/// bytes del medio, asi que hay que quitarlo. Este comando dice exactamente que
/// combinacion sigue siendo valida.
/// Prueba que cabeceras acepta googlevideo para una URL acunada.
///
/// Python funciona y reqwest no sobre la MISMA url, asi que la diferencia esta
/// en las cabeceras por defecto de cada cliente.
pub async fn headers(url: &str) -> Result<()> {
    let target = format!("{url}&range=0-262143&rn=0");

    println!("
  Cabeceras contra una URL acunada
");
    println!("  {:<40} {:>6}  {}", "CLIENTE", "HTTP", "CABECERA");
    println!("  {}", "-".repeat(66));

    let variantes: Vec<(&str, reqwest::Client)> = vec![
        ("por defecto (gzip on)", reqwest::Client::new()),
        (
            "sin gzip",
            reqwest::Client::builder().no_gzip().build()?,
        ),
        (
            "sin gzip + UA de navegador",
            reqwest::Client::builder()
                .no_gzip()
                .user_agent(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36                      (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
                )
                .build()?,
        ),
        (
            "solo UA de navegador",
            reqwest::Client::builder()
                .user_agent(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36                      (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
                )
                .build()?,
        ),
    ];

    for (label, client) in variantes {
        match client.get(&target).send().await {
            Ok(r) => {
                let st = r.status().as_u16();
                let b = r.bytes().await.unwrap_or_default();
                let head: String = b.iter().take(4).map(|x| format!("{x:02x}")).collect();
                let pista = if head == "1a45dfa3" { " <- WebM" } else { "" };
                println!("  {label:<40} {st:>6}  {head}{pista}");
            }
            Err(e) => println!("  {label:<40}  error: {e}"),
        }
    }
    println!();
    Ok(())
}

pub async fn strip(url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let combos: [&[&str]; 5] = [
        &[],
        &["ump"],
        &["ump", "srfvp"],
        &["ump", "alr"],
        &["ump", "srfvp", "alr"],
    ];

    println!("
  Efecto de quitar parametros de una URL acunada
");
    println!("  {:<26} {:>6} {:>9}  {}", "QUITANDO", "HTTP", "BYTES", "CABECERA");
    println!("  {}", "-".repeat(66));

    for combo in combos {
        let mut target = url.to_string();
        for k in combo {
            // Se elimina `&k=valor` de la query.
            while let Some(i) = target.find(&format!("&{k}=")) {
                let rest = &target[i + 1..];
                let end = rest.find('&').map(|j| i + 1 + j).unwrap_or(target.len());
                target.replace_range(i..end, "");
            }
        }

        let res = client
            .get(format!("{target}&range=0-262143"))
            .send()
            .await?;
        let status = res.status().as_u16();
        let bytes = res.bytes().await.unwrap_or_default();
        let head: String = bytes
            .iter()
            .take(4)
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join("");
        let etiqueta = if combo.is_empty() { "(nada)".to_string() } else { combo.join(",") };
        let pista = match head.as_str() {
            "1a45dfa3" => " <- WebM",
            _ if head.starts_with("3a") => " <- UMP",
            _ => "",
        };
        println!("  {etiqueta:<26} {status:>6} {:>9}  {head}{pista}", bytes.len());
    }
    println!();
    Ok(())
}

pub async fn raw_url(url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let expected: u64 = url
        .split("clen=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    println!("
  Descargando URL formada por el navegador");
    println!("  esperado: {} KB
", expected / 1024);

    let mut got = 0u64;
    let mut rn = 0u64;
    let t = Instant::now();
    loop {
        // Se usa el parametro de query, que es lo que hace el reproductor real,
        // con `rn` incremental.
        let res = client
            .get(format!("{url}&range={got}-{}&rn={rn}&rbuf=0", got + CHUNK - 1))
            .send()
            .await?;
        rn += 1;
        let status = res.status();
        let bytes = res.bytes().await?;
        if !status.is_success() {
            println!("  HTTP {} en offset {got}", status.as_u16());
            break;
        }
        if bytes.is_empty() {
            break;
        }
        got += bytes.len() as u64;
        println!("  +{} KB  (total {} KB)", bytes.len() / 1024, got / 1024);
        if expected > 0 && got >= expected {
            break;
        }
        if (bytes.len() as u64) < CHUNK {
            break;
        }
    }

    let secs = t.elapsed().as_secs_f64();
    println!(
        "
  {} KB en {:.1}s ({:.2} MB/s) - {}
",
        got / 1024,
        secs,
        got as f64 / 1_048_576.0 / secs,
        if expected > 0 && got >= expected { "COMPLETO" } else { "CORTADO" }
    );
    Ok(())
}

pub async fn attest(video_id: &str, visitor_data: &str, po_token: &str) -> Result<()> {
    let it = InnerTube::new()?;
    let client = reqwest::Client::new();
    let att = ytm_source::Attestation {
        visitor_data: visitor_data.to_string(),
        po_token: po_token.to_string(),
    };

    println!("
  Con atestacion de sesion - {video_id}
");
    println!("  {:<15} {:>7} {:>9} {:>9} {:>5}  {}", "CLIENTE", "TROZOS", "KB", "ESPERADO", "POT", "VEREDICTO");
    println!("  {}", "-".repeat(78));

    for cfg in ytm_source::clients::ALL {
        let Ok(res) = it.player_with(video_id, *cfg, Some(&att)).await else {
            continue;
        };
        let Some(streaming) = res.streaming_data else { continue };
        let Some(fmt) = ytm_source::select::best_audio(&streaming.adaptive_formats) else {
            continue;
        };
        let Some(url) = fmt.url.clone() else { continue };
        let expected = fmt.content_length_bytes().unwrap_or(0);
        // Si YouTube acepto la atestacion, devuelve la URL ya con `pot`.
        let has_pot = url.contains("&pot=") || url.contains("?pot=");

        let mut got = 0u64;
        let mut chunks = 0;
        loop {
            let r = client
                .get(&url)
                .header("Range", format!("bytes={got}-{}", got + CHUNK - 1))
                .header("User-Agent", cfg.user_agent)
                .send()
                .await;
            match r {
                Ok(resp) if resp.status().is_success() => {
                    let n = resp.bytes().await.map(|b| b.len() as u64).unwrap_or(0);
                    if n == 0 { break; }
                    got += n;
                    chunks += 1;
                    if n < CHUNK || (expected > 0 && got >= expected) { break; }
                }
                _ => break,
            }
        }

        let ok = expected > 0 && got >= expected;
        println!(
            "  {:<15} {chunks:>7} {:>9} {:>9} {:>5}  {}",
            cfg.id,
            got / 1024,
            expected / 1024,
            if has_pot { "si" } else { "no" },
            if ok { "COMPLETO" } else { "CORTADO" }
        );
    }
    println!();
    Ok(())
}

pub async fn client_limits(video_id: &str) -> Result<()> {
    let it = InnerTube::new()?;
    let client = reqwest::Client::new();

    println!("
  Limite de descarga por cliente - {video_id}
");
    println!("  {:<15} {:>7} {:>9} {:>9}  {}", "CLIENTE", "TROZOS", "KB", "ESPERADO", "VEREDICTO");
    println!("  {}", "-".repeat(72));

    for cfg in ytm_source::clients::ALL {
        let res = match it.player(video_id, *cfg).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let Some(streaming) = res.streaming_data else { continue };
        let Some(fmt) = ytm_source::select::best_audio(&streaming.adaptive_formats) else {
            continue;
        };
        let Some(url) = fmt.url.clone() else { continue };
        let expected = fmt.content_length_bytes().unwrap_or(0);

        let mut got = 0u64;
        let mut chunks = 0;
        loop {
            let r = client
                .get(&url)
                .header("Range", format!("bytes={got}-{}", got + CHUNK - 1))
                // El User-Agent se hace coincidir con el cliente que resolvio la
                // URL, por si acaso influye.
                .header("User-Agent", cfg.user_agent)
                .send()
                .await;
            match r {
                Ok(resp) if resp.status().is_success() => {
                    let n = resp.bytes().await.map(|b| b.len() as u64).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    got += n;
                    chunks += 1;
                    if n < CHUNK || (expected > 0 && got >= expected) {
                        break;
                    }
                }
                _ => break,
            }
        }

        let ok = expected > 0 && got >= expected;
        println!(
            "  {:<15} {chunks:>7} {:>9} {:>9}  {}",
            cfg.id,
            got / 1024,
            expected / 1024,
            if ok { "COMPLETO" } else { "CORTADO" }
        );
    }
    println!();
    Ok(())
}

pub async fn limits(ids: &[String]) -> Result<()> {
    let it = InnerTube::new()?;
    let client = reqwest::Client::new();

    println!("
  {:<14} {:>7} {:>8} {:>9}  {}", "VIDEO", "TROZOS", "KB", "ESPERADO", "TITULO");
    println!("  {}", "-".repeat(78));

    for id in ids {
        let Ok(resolved) = ytm_source::resolve(&it, id).await else {
            println!("  {id:<14} {:>7} {:>8} {:>9}  no se pudo resolver", "-", "-", "-");
            continue;
        };
        let expected = resolved.audio.size_bytes.unwrap_or(0);

        let mut got = 0u64;
        let mut chunks = 0;
        loop {
            let res = client
                .get(&resolved.audio.url)
                .header("Range", format!("bytes={got}-{}", got + CHUNK - 1))
                .send()
                .await;
            match res {
                Ok(r) if r.status().is_success() => {
                    let n = r.bytes().await.map(|b| b.len() as u64).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    got += n;
                    chunks += 1;
                    if n < CHUNK || (expected > 0 && got >= expected) {
                        break;
                    }
                }
                _ => break,
            }
        }

        let verdict = if expected > 0 && got >= expected { "COMPLETO" } else { "CORTADO" };
        println!(
            "  {id:<14} {chunks:>7} {:>8} {:>9}  {} · {}",
            got / 1024,
            expected / 1024,
            verdict,
            resolved.track.title.as_deref().unwrap_or("?").chars().take(34).collect::<String>()
        );
    }
    println!();
    Ok(())
}

pub async fn follow(video_id: &str) -> Result<()> {
    let it = InnerTube::new()?;
    let client = reqwest::Client::new();
    let resolved = ytm_source::resolve(&it, video_id).await?;
    let expected = resolved.audio.size_bytes.unwrap_or(0);

    println!("
  Siguiendo la redireccion de alr=yes");
    println!("  esperado: {} KB
", expected / 1024);

    let mut base = resolved.audio.url.clone();
    let mut offset = 0u64;
    let mut redirects = 0;
    // `rn` es el numero de peticion y debe incrementar en CADA peticion. Si se
    // repite, googlevideo la trata como un reintento y devuelve 403 en vez de
    // la redireccion.
    let mut rn = 0u64;
    let t = Instant::now();

    while expected == 0 || offset < expected {
        let end = offset + CHUNK - 1;
        let res = client
            .get(format!("{base}&range={offset}-{end}&rn={rn}&rbuf=0&alr=yes"))
            .send()
            .await?;
        rn += 1;
        let status = res.status();
        let bytes = res.bytes().await?;

        // Un cuerpo pequeno que empieza por http:// es una redireccion, no audio.
        let looks_like_url = bytes.len() < 8192 && bytes.starts_with(b"http");
        if looks_like_url {
            base = String::from_utf8_lossy(&bytes).trim().to_string();
            redirects += 1;
            println!("  redireccion #{redirects} en offset {offset}");
            if redirects > 12 {
                println!("  demasiadas redirecciones, abandono
");
                return Ok(());
            }
            continue;
        }

        if !status.is_success() {
            println!("  HTTP {} en offset {offset}
", status.as_u16());
            return Ok(());
        }
        if bytes.is_empty() {
            break;
        }
        offset += bytes.len() as u64;
        println!("  +{} KB  (total {} KB)", bytes.len() / 1024, offset / 1024);
        if (bytes.len() as u64) < CHUNK {
            break;
        }
    }

    let secs = t.elapsed().as_secs_f64();
    println!(
        "
  RESULTADO: {} KB en {:.1}s ({:.2} MB/s), {redirects} redirecciones",
        offset / 1024,
        secs,
        offset as f64 / 1_048_576.0 / secs
    );
    println!(
        "  completo: {}
",
        if expected > 0 && offset >= expected { "SI" } else { "NO" }
    );
    Ok(())
}

pub async fn recover(video_id: &str) -> Result<()> {
    let it = InnerTube::new()?;
    let client = reqwest::Client::new();

    // 1) Que hay dentro del cuerpo de 1 KB de alr=yes.
    let resolved = ytm_source::resolve(&it, video_id).await?;
    let _ = client
        .get(format!("{}&range=0-{}&rn=0&rbuf=0&alr=yes", resolved.audio.url, CHUNK - 1))
        .send()
        .await?
        .bytes()
        .await?;
    let second = client
        .get(format!(
            "{}&range={}-{}&rn=1&rbuf=0&alr=yes",
            resolved.audio.url,
            CHUNK,
            CHUNK * 2 - 1
        ))
        .send()
        .await?;
    let body = second.text().await.unwrap_or_default();
    println!("
  Cuerpo de alr=yes en el segundo trozo ({} bytes):", body.len());
    println!("  {}
", body.chars().take(220).collect::<String>());

    // 2) Re-resolver la URL para cada trozo.
    println!("  Re-resolviendo la URL en cada trozo:
");
    let mut codes = Vec::new();
    let mut total = 0usize;
    for i in 0..4u64 {
        let fresh = ytm_source::resolve(&it, video_id).await?;
        let start = i * CHUNK;
        let res = client
            .get(&fresh.audio.url)
            .header("Range", format!("bytes={start}-{}", start + CHUNK - 1))
            .send()
            .await?;
        let st = res.status().as_u16();
        let n = res.bytes().await.map(|b| b.len()).unwrap_or(0);
        total += n;
        codes.push(format!("{st}/{}KB", n / 1024));
    }
    println!("  {}", codes.join("  "));
    println!("  total: {} KB
", total / 1024);
    Ok(())
}

pub async fn params(video_id: &str) -> Result<()> {
    let it = InnerTube::new()?;

    let variants: [(&str, fn(&str, u64, u64, u64) -> String); 4] = [
        ("solo range", |u, s, e, _| format!("{u}&range={s}-{e}")),
        ("range+rn", |u, s, e, i| format!("{u}&range={s}-{e}&rn={i}")),
        ("range+rn+rbuf", |u, s, e, i| format!("{u}&range={s}-{e}&rn={i}&rbuf=0")),
        ("range+rn+rbuf+alr", |u, s, e, i| {
            format!("{u}&range={s}-{e}&rn={i}&rbuf=0&alr=yes")
        }),
    ];

    println!("
  Parametros de query, 4 trozos seguidos (URL nueva por variante)
");
    for (label, build) in variants {
        // URL recien resuelta para cada variante: una URL ya quemada daria 403
        // por el motivo equivocado.
        let resolved = ytm_source::resolve(&it, video_id).await?;
        let client = reqwest::Client::new();
        let mut codes = Vec::new();

        for i in 0..4u64 {
            let start = i * CHUNK;
            let target = build(&resolved.audio.url, start, start + CHUNK - 1, i);
            match client.get(&target).send().await {
                Ok(r) => {
                    let st = r.status().as_u16();
                    let n = r.bytes().await.map(|b| b.len()).unwrap_or(0);
                    codes.push(format!("{st}/{}KB", n / 1024));
                }
                Err(_) => codes.push("ERR".into()),
            }
        }
        println!("  {label:<22} {}", codes.join("  "));
    }
    println!();
    Ok(())
}

pub async fn full_get(video_id: &str) -> Result<()> {
    let it = InnerTube::new()?;
    let resolved = ytm_source::resolve(&it, video_id).await?;
    let expected = resolved.audio.size_bytes.unwrap_or(0);

    println!("
  GET completo - {}", resolved.track.title.as_deref().unwrap_or("?"));
    println!("  esperado: {} KB
", expected / 1024);

    let t = Instant::now();
    let res = reqwest::Client::new().get(&resolved.audio.url).send().await?;
    let status = res.status();
    let body = res.bytes().await?;
    let secs = t.elapsed().as_secs_f64();
    let mb = body.len() as f64 / 1_048_576.0;

    println!("  HTTP {}", status.as_u16());
    println!("  recibido  {} KB en {:.1}s ({:.2} MB/s)", body.len() / 1024, secs, mb / secs);
    println!(
        "  completo  {}",
        if expected > 0 && body.len() as u64 == expected { "SI" } else { "NO" }
    );
    let realtime = resolved.audio.bitrate.unwrap_or(130_000) as f64 / 8.0 / 1_048_576.0;
    println!("  veredicto {}
", if mb / secs > realtime * 3.0 { "USABLE" } else { "DEMASIADO LENTO" });
    Ok(())
}

pub async fn run(video_id: &str) -> Result<()> {
    let it = InnerTube::new()?;
    let resolved = ytm_source::resolve(&it, video_id).await?;
    let url = &resolved.audio.url;
    let http = reqwest::Client::new();

    let realtime = resolved.audio.bitrate.unwrap_or(130_000) as f64 / 8.0 / 1_048_576.0;

    println!(
        "\n  Estrategias de descarga - {} (via {})",
        resolved.track.title.as_deref().unwrap_or("?"),
        resolved.audio.via_client
    );
    println!("  Tiempo real exige {realtime:.3} MB/s. Trozo de prueba: 1 MiB.\n");
    println!(
        "  {:<30} {:>9} {:>9} {:>9}  {}",
        "ESTRATEGIA", "BYTES", "SEG", "MB/s", "VEREDICTO"
    );
    println!("  {}", "-".repeat(76));

    // 1) Rango como parametro de la query string (lo que usa yt-dlp).
    let t = Instant::now();
    let res = http
        .get(format!("{url}&range=0-{}", CHUNK - 1))
        .send()
        .await?;
    let status = res.status();
    let body = res.bytes().await?;
    report("query param &range=", body.len(), t.elapsed(), status);

    // 2) Cabecera HTTP Range estandar.
    let t = Instant::now();
    let res = http
        .get(url)
        .header("Range", format!("bytes=0-{}", CHUNK - 1))
        .send()
        .await?;
    let status = res.status();
    let body = res.bytes().await?;
    report("cabecera Range:", body.len(), t.elapsed(), status);

    // 3) Ambos, que es lo que hace el reproductor web real.
    let t = Instant::now();
    let res = http
        .get(format!("{url}&range=0-{}", CHUNK - 1))
        .header("Range", format!("bytes=0-{}", CHUNK - 1))
        .send()
        .await?;
    let status = res.status();
    let body = res.bytes().await?;
    report("ambos combinados", body.len(), t.elapsed(), status);

    // 4) Control: rango en mitad del archivo y con CONEXION NUEVA, para
    //    descartar que las cifras anteriores vengan de reutilizar el TLS o de
    //    un trato especial al primer trozo.
    if let Some(total) = resolved.audio.size_bytes {
        if total > CHUNK * 3 {
            let mid = total / 2;

            // Solo query param. OJO: `&range=` corta del lado del servidor, asi
            // que anadir ademas la cabecera `Range:` con offsets absolutos da
            // 416, porque la cabecera se aplica sobre el trozo ya cortado.
            let fresh = reqwest::Client::new();
            let t = Instant::now();
            let res = fresh
                .get(format!("{url}&range={mid}-{}", mid + CHUNK - 1))
                .send()
                .await?;
            let status = res.status();
            let body = res.bytes().await?;
            report("control query, conexion nueva", body.len(), t.elapsed(), status);

            // Solo cabecera, conexion nueva.
            let fresh = reqwest::Client::new();
            let t = Instant::now();
            let res = fresh
                .get(url)
                .header("Range", format!("bytes={mid}-{}", mid + CHUNK - 1))
                .send()
                .await?;
            let status = res.status();
            let body = res.bytes().await?;
            report("control header, conexion nueva", body.len(), t.elapsed(), status);
        }
    }

    // 5) La prueba que importa: VARIOS trozos seguidos, variando el
    //    User-Agent. Hipotesis: googlevideo valida que las peticiones de
    //    descarga vengan del mismo cliente que resolvio la URL.
    let ua = ytm_source::clients::by_id(resolved.audio.via_client)
        .map(|c| c.user_agent)
        .unwrap_or("");

    println!("
  Descarga secuencial de 4 trozos (1 MiB cada uno):
");
    for (label, with_ua) in [("sin User-Agent", false), ("con UA del cliente", true)] {
        let client = reqwest::Client::new();
        let mut codes = Vec::new();
        for i in 0..4u64 {
            let start = i * CHUNK;
            let mut req = client
                .get(url)
                .header("Range", format!("bytes={start}-{}", start + CHUNK - 1));
            if with_ua {
                req = req.header("User-Agent", ua);
            }
            match req.send().await {
                Ok(r) => {
                    let st = r.status().as_u16();
                    let n = r.bytes().await.map(|b| b.len()).unwrap_or(0);
                    codes.push(format!("{st}/{}KB", n / 1024));
                }
                Err(_) => codes.push("ERR".into()),
            }
        }
        println!("  {label:<22} {}", codes.join("  "));
    }

    println!();
    Ok(())
}

fn report(name: &str, len: usize, dur: Duration, status: reqwest::StatusCode) {
    let mb = len as f64 / 1_048_576.0;
    let secs = dur.as_secs_f64();
    let speed = mb / secs;
    let verdict = if !status.is_success() && status.as_u16() != 206 {
        format!("HTTP {}", status.as_u16())
    } else if speed > 1.0 {
        "sin throttling".into()
    } else if speed > 0.2 {
        "aceptable".into()
    } else {
        "ESTRANGULADO".into()
    };
    println!("  {name:<30} {len:>9} {secs:>9.2} {speed:>9.2}  {verdict}");
}
