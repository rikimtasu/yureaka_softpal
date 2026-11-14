//! Read-only audio loading benchmark, including first loads and repeated loads.
use std::time::Instant;
use pal_asset::{Nls, ResourceManager};
use pal_vm::audio::{decode_static_sound, AudioConfig, AudioSystem, PalSoundGroup};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let root = args.next().ok_or_else(|| anyhow::anyhow!("usage: bench_audio_load ROOT ITERATIONS RESOURCE..."))?;
    let iterations: usize = args.next().unwrap_or_else(|| "20".into()).parse()?;
    anyhow::ensure!((1..=1000).contains(&iterations), "iterations must be 1..1000");
    let mut resources = ResourceManager::bootstrap(root, Nls::ShiftJis)?;
    let mut audio = AudioSystem::new(AudioConfig { enabled: false })?;
    for name in args {
        let asset = resources.open(&name)?;
        let start = Instant::now();
        let expected = decode_static_sound(&asset.bytes)?;
        let decode_ms = start.elapsed().as_secs_f64() * 1000.0;
        let mut samples = Vec::new();
        for _ in 0..iterations {
            let start = Instant::now();
            let handle = audio.load_static_from_resource(&mut resources, &name, PalSoundGroup::GROUP2)?;
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
            audio.release(handle)?;
        }
        let first = samples[0];
        samples.sort_by(f64::total_cmp);
        println!("{name}: frames={} decode_ms={decode_ms:.3} first_load_ms={first:.3} p50_load_ms={:.3}", expected.frames.len(), samples[iterations / 2]);
    }
    Ok(())
}
