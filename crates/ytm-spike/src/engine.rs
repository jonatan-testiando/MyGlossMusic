//! Prueba end-to-end del motor de audio.
//!
//! Verifica lo que no cubren los tests unitarios porque necesita red y tarjeta
//! de sonido: arranque instantaneo, avance de la posicion, progreso de buffer,
//! seek y pausa.

use anyhow::Result;
use std::time::{Duration, Instant};
use ytm_audio::{Command, Engine};

pub async fn run(video_id: &str) -> Result<()> {
    // Sin acunador: el spike no tiene webview, asi que solo llegara al
    // limite de ~1 MiB. Sirve igual para probar cola, seek y pausa.
    let engine = Engine::start(None)?;

    println!("\n  Motor arrancado. Reproduciendo {video_id}\n");
    let t0 = Instant::now();
    engine.send(Command::PlayNow(video_id.to_string()));

    // 1) Cuanto tarda en sonar el primer byte.
    let mut first_sound: Option<Duration> = None;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let s = engine.state();
        if s.playing {
            first_sound = Some(t0.elapsed());
            break;
        }
        if let Some(e) = &s.error {
            anyhow::bail!("el motor fallo: {e}");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let Some(ttfs) = first_sound else {
        anyhow::bail!("no empezo a sonar en 30 s");
    };

    let s = engine.state();
    println!("  arranque     {} ms hasta el primer sonido", ttfs.as_millis());
    println!("  pista        {}", s.track.as_ref().map_or("?", |t| &t.title));
    println!("  duracion     {}", fmt_ms(s.duration_ms));
    println!();

    // 2) La posicion debe avanzar y el buffer llenarse.
    println!("  {:<10} {:>10} {:>10}  {}", "T", "POSICION", "BUFFER", "ESTADO");
    println!("  {}", "-".repeat(50));
    for i in 0..5 {
        tokio::time::sleep(Duration::from_millis(800)).await;
        let s = engine.state();
        println!(
            "  {:<10} {:>10} {:>9.0}%  {}",
            format!("{}s", (i + 1) * 8 / 10),
            fmt_ms(s.position_ms),
            s.buffered * 100.0,
            if s.playing { "sonando" } else { "parado" }
        );
    }

    let before = engine.state().position_ms;
    anyhow::ensure!(before > 0, "la posicion no avanzo: sigue en 0");

    // 3) Seek.
    println!("\n  Saltando a 1:30...");
    engine.send(Command::Seek(Duration::from_secs(90)));
    tokio::time::sleep(Duration::from_millis(600)).await;
    let after = engine.state();
    println!("  posicion     {}", fmt_ms(after.position_ms));
    if after.position_ms < 85_000 || after.position_ms > 100_000 {
        println!("  AVISO: el seek no dejo la posicion donde tocaba");
    } else {
        println!("  seek OK");
    }

    // 4) Pausa y reanudacion.
    engine.send(Command::Pause);
    tokio::time::sleep(Duration::from_millis(400)).await;
    let paused = engine.state();
    let p1 = paused.position_ms;
    tokio::time::sleep(Duration::from_millis(600)).await;
    let p2 = engine.state().position_ms;
    println!(
        "\n  pausa        {} -> {} ({})",
        fmt_ms(p1),
        fmt_ms(p2),
        if p2.abs_diff(p1) < 150 { "congelada OK" } else { "SIGUE AVANZANDO" }
    );

    engine.send(Command::Resume);
    tokio::time::sleep(Duration::from_millis(700)).await;
    let resumed = engine.state();
    println!(
        "  reanudacion  {} ({})",
        fmt_ms(resumed.position_ms),
        if resumed.position_ms > p2 { "avanza OK" } else { "NO AVANZA" }
    );

    engine.send(Command::Stop);
    println!("\n  Prueba completada.\n");
    Ok(())
}

fn fmt_ms(ms: u64) -> String {
    let total = ms / 1000;
    format!("{}:{:02}", total / 60, total % 60)
}
