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
