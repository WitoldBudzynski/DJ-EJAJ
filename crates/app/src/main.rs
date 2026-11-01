mod controls;
mod state;
mod ui;
use eframe::egui;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|s| s == "--list-devices") {
        for d in djejaj_audio::output_devices() {
            println!("{d}");
        }
        return Ok(());
    }
    if let Some(i) = args.iter().position(|s| s == "--export-samples") {
        let path = PathBuf::from(
            args.get(i + 1)
                .ok_or_else(|| anyhow::anyhow!("Podaj katalog banku"))?,
        );
        djejaj_sampler::export_bank(&path)?;
        println!("Zapisano oryginalny bank: {}", path.display());
        return Ok(());
    }
    if let Some(i) = args.iter().position(|s| s == "--audio-check") {
        let seconds = args
            .get(i + 1)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(30);
        let engine = djejaj_audio::Engine::new(None, 256, false)?;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(seconds) {
            std::thread::sleep(Duration::from_millis(100));
        }
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"device":engine.device_name,"sample_rate":engine.sample_rate,"channels":engine.channels,"requested_buffer":engine.buffer_requested,"seconds":start.elapsed().as_secs_f64(),"signal":"silence; no physical output or round-trip latency validation","diagnostics":engine.shared.diagnostics()})
            )?
        );
        return Ok(());
    }
    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let demo = args.iter().any(|a| a == "--demo");
    let size = args
        .iter()
        .position(|a| a == "--size")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?]))
        .unwrap_or([1440.0, 960.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DJ EJAJ · by NT4.pl")
            .with_inner_size(size)
            .with_min_inner_size([1120.0, 780.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DJ EJAJ",
        options,
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, demo, screenshot)?))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}
