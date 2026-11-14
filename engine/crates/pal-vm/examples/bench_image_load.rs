//! Read-only sprite-loading probe. No game resources are embedded in this file.
use std::time::Instant;

use pal_asset::{Nls, ResourceManager};
use pal_vm::image::decode_image_with_resolver;
use pal_vm::image::ImageDecoder;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let root = args.next().ok_or_else(|| anyhow::anyhow!("usage: bench_image_load ROOT ITERATIONS RESOURCE..."))?;
    let iterations: usize = args.next().unwrap_or_else(|| "10".into()).parse()?;
    anyhow::ensure!((1..=1000).contains(&iterations), "iterations must be 1..1000");
    let names: Vec<_> = args.collect();
    anyhow::ensure!(!names.is_empty(), "supply at least one resource name");
    let mut resources = ResourceManager::bootstrap(root, Nls::ShiftJis)?;
    let mut decoder = ImageDecoder::default();
    for name in names {
        let mut samples = Vec::new();
        let mut open_ms = 0.0;
        let mut decode_ms = 0.0;
        for _ in 0..iterations {
            let start = Instant::now();
            let asset = resources.open(&name)?;
            let opened = Instant::now();
            let image = decode_image_with_resolver(&asset.bytes, &mut |base: &str| {
                Ok(resources.open(base)?.bytes)
            })?;
            std::hint::black_box(&image.rgba);
            let end = Instant::now();
            open_ms += (opened - start).as_secs_f64() * 1000.0;
            decode_ms += (end - opened).as_secs_f64() * 1000.0;
            samples.push((end - start).as_secs_f64() * 1000.0);
        }
        samples.sort_by(f64::total_cmp);
        println!("{name}: mean_open_ms={:.3} mean_decode_ms={:.3} p50_ms={:.3} max_ms={:.3}",
            open_ms / iterations as f64, decode_ms / iterations as f64,
            samples[iterations / 2], samples[iterations - 1]);
        let mut cached_samples = Vec::new();
        let mut first_ms = 0.0;
        for iteration in 0..iterations {
            let start = Instant::now();
            let asset = resources.open(&name)?;
            let image = decoder.decode(&asset.bytes, &mut |base: &str| Ok(resources.open(base)?.bytes))?;
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            if iteration == 0 { first_ms = elapsed; }
            std::hint::black_box(&image.rgba);
            cached_samples.push(elapsed);
        }
        cached_samples.sort_by(f64::total_cmp);
        println!("{name}: cached_first_ms={first_ms:.3} cached_p50_load_ms={:.3}", cached_samples[iterations / 2]);
    }
    Ok(())
}
