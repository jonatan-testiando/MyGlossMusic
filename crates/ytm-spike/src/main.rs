//! Spike de Fase 0: valida el riesgo principal del proyecto antes de invertir
//! nada en interfaz.
//!
//! Responde a dos preguntas:
//!   1. `probe`: que clientes de InnerTube siguen vivos HOY y cual conviene.
//!   2. `play`:  se puede extraer y reproducir audio real, sin throttling.
//!
//! Uso:
//!   ytm-spike probe <videoId>
//!   ytm-spike play  <videoId> [--client <id>] [--keep]

mod bench;
mod engine;

use anyhow::{bail, Context, Result};
use std::io::Write as _;
use std::time::Instant;
use ytm_source::{clients, select, InnerTube};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ytm_spike=info,ytm_source=info".into()),
        )
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    match cmd {
        "probe" => {
            let id = video_id_arg(&args)?;
            probe(&id).await
        }
        "search" => {
            let q = args[1..].join(" ");
            anyhow::ensure!(!q.is_empty(), "falta la consulta");
            let it = InnerTube::new()?;
            let hits = it.search(&q, ytm_source::Filter::Songs).await?;
            println!("
  {} resultados para: {q}
", hits.len());
            for (i, h) in hits.iter().take(10).enumerate() {
                println!("  {:>2}. {}", i + 1, h.title);
                println!("      {} [{}]  {}", h.subtitle,
                    h.duration.as_deref().unwrap_or("?"), h.video_id);
            }
            println!();
            Ok(())
        }
        "engine" => {
            let id = video_id_arg(&args)?;
            engine::run(&id).await
        }
        "rawurl" => {
            let path = args.get(1).ok_or_else(|| anyhow::anyhow!("falta el archivo"))?;
            let url = std::fs::read_to_string(path)?;
            bench::raw_url(url.trim()).await
        }
        "attestdetail" => {
            let id = video_id_arg(&args)?;
            let vd = std::env::var("YTM_VISITOR_DATA")?;
            let pot = std::env::var("YTM_POTOKEN")?;
            bench::attest_detail(&id, &vd, &pot).await
        }
        "attest" => {
            let id = video_id_arg(&args)?;
            let vd = std::env::var("YTM_VISITOR_DATA")
                .map_err(|_| anyhow::anyhow!("falta YTM_VISITOR_DATA"))?;
            let pot = std::env::var("YTM_POTOKEN")
                .map_err(|_| anyhow::anyhow!("falta YTM_POTOKEN"))?;
            bench::attest(&id, &vd, &pot).await
        }
        "clientlimits" => {
            let id = video_id_arg(&args)?;
            bench::client_limits(&id).await
        }
        "limits" => {
            let ids: Vec<String> = args[1..].iter().map(|a| parse_video_id(a)).collect();
            anyhow::ensure!(!ids.is_empty(), "faltan videoIds");
            bench::limits(&ids).await
        }
        "follow" => {
            let id = video_id_arg(&args)?;
            bench::follow(&id).await
        }
        "recover" => {
            let id = video_id_arg(&args)?;
            bench::recover(&id).await
        }
        "params" => {
            let id = video_id_arg(&args)?;
            bench::params(&id).await
        }
        "fullget" => {
            let id = video_id_arg(&args)?;
            bench::full_get(&id).await
        }
        "bench" => {
            let id = video_id_arg(&args)?;
            bench::run(&id).await
        }
        "play" => {
            let id = video_id_arg(&args)?;
            let client = flag_value(&args, "--client");
            let keep = args.iter().any(|a| a == "--keep");
            let no_play = args.iter().any(|a| a == "--no-play");
            play(&id, client.as_deref(), keep, no_play).await
        }
        _ => {
            print_usage();
            bail!("comando desconocido: {cmd}");
        }
    }
}

fn print_usage() {
    eprintln!(
        "ytm-spike - validacion de extraccion y reproduccion\n\n\
         USO:\n  \
           ytm-spike probe <videoId>              prueba todos los clientes InnerTube\n  \
           ytm-spike play  <videoId> [opciones]   extrae y reproduce\n\n\
         OPCIONES de play:\n  \
           --client <id>   fuerza un cliente concreto (ios, android_vr, tv, ...)\n  \
           --keep          conserva el archivo descargado\n"
    );
}

fn video_id_arg(args: &[String]) -> Result<String> {
    let raw = args
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("falta el <videoId>"))?;
    Ok(parse_video_id(raw))
}

/// Acepta un id pelado o cualquier URL de YouTube / YouTube Music.
fn parse_video_id(input: &str) -> String {
    for marker in ["v=", "youtu.be/", "/shorts/", "/watch/"] {
        if let Some(pos) = input.find(marker) {
            let rest = &input[pos + marker.len()..];
            let end = rest
                .find(['&', '?', '#', '/'])
                .unwrap_or(rest.len());
            return rest[..end].to_string();
        }
    }
    input.to_string()
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let pos = args.iter().position(|a| a == flag)?;
    args.get(pos + 1).cloned()
}

/// Prueba cada cliente conocido y tabula el resultado.
///
/// Esta tabla es la que decide el orden de `clients::PREFERRED`. Reejecutala
/// cuando algo deje de funcionar: en 10 segundos sabes si YouTube cerro un
/// cliente y cual sigue en pie.
async fn probe(video_id: &str) -> Result<()> {
    let it = InnerTube::new()?;

    println!("\n  Sondeando clientes InnerTube para: {video_id}\n");
    println!(
        "  {:<13} {:<22} {:>6} {:>7}  {}",
        "CLIENTE", "ESTADO", "AUDIO", "DIRECTA", "MEJOR PISTA"
    );
    println!("  {}", "-".repeat(78));

    let mut working = Vec::new();

    for client in clients::ALL {
        let (status, n_audio, n_direct, best) = match it.player(video_id, *client).await {
            Err(e) => {
                let msg = e.to_string();
                let short: String = msg.lines().next().unwrap_or("error").chars().take(21).collect();
                (short, 0, 0, "-".to_string())
            }
            Ok(res) => {
                let status = res
                    .playability_status
                    .as_ref()
                    .and_then(|p| p.status.clone())
                    .unwrap_or_else(|| "SIN_ESTADO".into());

                let formats = res
                    .streaming_data
                    .as_ref()
                    .map(|s| s.adaptive_formats.clone())
                    .unwrap_or_default();

                let audio: Vec<_> = formats.iter().filter(|f| f.is_audio()).cloned().collect();
                let direct = audio.iter().filter(|f| f.is_playable_directly()).count();

                let best = match select::best_audio(&audio) {
                    Some(f) => format!(
                        "itag {} {} {} kbps",
                        f.itag,
                        f.codec().unwrap_or("?"),
                        f.bitrate.or(f.average_bitrate).unwrap_or(0) / 1000
                    ),
                    None if !audio.is_empty() => "solo cifradas".to_string(),
                    None => "-".to_string(),
                };

                if direct > 0 && status == "OK" {
                    working.push(client.id);
                }
                (status, audio.len(), direct, best)
            }
        };

        let mark = if n_direct > 0 && status == "OK" { "+" } else { " " };
        println!(
            "{} {:<13} {:<22} {:>6} {:>7}  {}",
            mark, client.id, status, n_audio, n_direct, best
        );
    }

    println!();
    if working.is_empty() {
        println!("  NINGUN cliente devolvio audio directo.");
        println!("  Toca revisar versiones de cliente en crates/ytm-source/src/clients.rs");
    } else {
        println!("  Clientes utilizables: {}", working.join(", "));
        println!("  Ordena clients::PREFERRED empezando por estos.");
    }
    println!();
    Ok(())
}

/// Extrae, descarga y reproduce. Mide la velocidad de descarga porque es el
/// detector temprano de throttling (el parametro `n` sin descifrar deja el
/// stream a ~50 KB/s, que no da para reproducir en tiempo real).
async fn play(
    video_id: &str,
    forced_client: Option<&str>,
    keep: bool,
    no_play: bool,
) -> Result<()> {
    let it = InnerTube::new()?;

    let t0 = Instant::now();
    let resolved = match forced_client {
        Some(id) => {
            let cfg = clients::by_id(id)
                .ok_or_else(|| anyhow::anyhow!("cliente desconocido: {id}"))?;
            let res = it.player(video_id, cfg).await?;
            let streaming = res
                .streaming_data
                .context("respuesta sin streamingData")?;
            let fmt = select::best_audio(&streaming.adaptive_formats)
                .context("sin audio con URL directa")?
                .clone();
            let details = res.video_details;
            ytm_source::Resolved {
                track: ytm_source::TrackInfo {
                    video_id: video_id.into(),
                    title: details.as_ref().and_then(|d| d.title.clone()),
                    author: details.as_ref().and_then(|d| d.author.clone()),
                    thumbnail: None,
                },
                audio: ytm_source::AudioStream {
                    url: fmt.url.clone().unwrap(),
                    itag: fmt.itag,
                    codec: fmt.codec().map(str::to_string),
                    bitrate: fmt.bitrate.or(fmt.average_bitrate),
                    size_bytes: fmt.content_length_bytes(),
                    duration_ms: fmt.approx_duration_ms.as_ref().and_then(|d| d.parse().ok()),
                    loudness_db: fmt.loudness_db,
                    via_client: cfg.id,
                },
            }
        }
        None => ytm_source::resolve(&it, video_id).await?,
    };
    let resolve_ms = t0.elapsed().as_millis();

    let a = &resolved.audio;
    println!("\n  {}", resolved.track.title.as_deref().unwrap_or("(sin titulo)"));
    println!("  {}", resolved.track.author.as_deref().unwrap_or("(sin autor)"));
    println!();
    println!("  cliente      {}", a.via_client);
    println!("  itag         {} ({})", a.itag, a.codec.as_deref().unwrap_or("?"));
    println!("  bitrate      {} kbps", a.bitrate.unwrap_or(0) / 1000);
    if let Some(l) = a.loudness_db {
        println!("  loudness     {l:.1} dB");
    }
    println!("  resolucion   {resolve_ms} ms");

    // Descarga completa a disco. El streaming con Range es la Fase 1; aqui solo
    // queremos saber si el stream llega entero y a que velocidad.
    // Descarga POR TROZOS, no de una sola pieza. Medido en `bench`: un GET
    // completo lo estrangula YouTube a ~0.03 MB/s, mientras que las peticiones
    // por rango van a 20-30 MB/s. Esto es lo que nos ahorra tener que descifrar
    // el parametro `n` con un motor JavaScript.
    const CHUNK: u64 = 1_048_576;
    let dl_start = Instant::now();
    let http = reqwest::Client::new();
    let mut buf: Vec<u8> = Vec::with_capacity(a.size_bytes.unwrap_or(4 << 20) as usize);
    let mut offset: u64 = 0;

    loop {
        let end = offset + CHUNK - 1;
        let res = http
            .get(&a.url)
            .header("Range", format!("bytes={offset}-{end}"))
            .send()
            .await
            .context("fallo una peticion de rango")?;

        if res.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            break; // pasado el final del archivo
        }
        let chunk = res.error_for_status()?.bytes().await?;
        let n = chunk.len() as u64;
        buf.extend_from_slice(&chunk);
        offset += n;

        if n < CHUNK {
            break; // ultimo trozo
        }
        if let Some(total) = a.size_bytes {
            if offset >= total {
                break;
            }
        }
    }
    let bytes = buf;
    let dl = dl_start.elapsed();

    let mb = bytes.len() as f64 / 1_048_576.0;
    let speed = mb / dl.as_secs_f64();
    println!("  descarga     {:.2} MB en {:.1}s ({:.2} MB/s)", mb, dl.as_secs_f64(), speed);

    // Un stream de 128 kbps consume 0.015 MB/s. Menos de ~0.2 MB/s significa
    // que YouTube nos esta limitando y habria que descifrar el parametro `n`.
    if speed < 0.2 {
        println!("\n  AVISO: velocidad muy baja, probable throttling del parametro n.");
    }

    let path = std::env::temp_dir().join(format!("ytm-spike-{video_id}.m4a"));
    let mut file = std::fs::File::create(&path)
        .with_context(|| format!("no se pudo crear {}", path.display()))?;
    file.write_all(&bytes)?;
    drop(file);

    println!("  archivo      {}", path.display());

    if no_play {
        println!("\n  (--no-play: no se reproduce)\n");
        if !keep {
            let _ = std::fs::remove_file(&path);
        }
        return Ok(());
    }

    println!("\n  Reproduciendo... (Ctrl+C para parar)\n");

    // rodio bloquea, asi que lo sacamos del runtime asincrono.
    let play_path = path.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let device = rodio::DeviceSinkBuilder::open_default_sink()
            .context("no se pudo abrir el dispositivo de audio")?;
        let player = rodio::Player::connect_new(device.mixer());
        let file = std::fs::File::open(&play_path)?;
        let source = rodio::Decoder::try_from(file).context("no se pudo decodificar el audio")?;
        player.append(source);
        player.sleep_until_end();
        Ok(())
    })
    .await??;

    if !keep {
        let _ = std::fs::remove_file(&path);
    }

    println!("  Listo.\n");
    Ok(())
}
