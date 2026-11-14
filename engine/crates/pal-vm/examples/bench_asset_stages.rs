//! Read-only asset-loading breakdown and shared final-image cache-hit feasibility probe.
//! The candidate hit reopens and hashes every PGD3 dependency; it is not a runtime cache.
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use pal_asset::{AssetSource, Nls, PacArchive, ResourceManager};
use pal_vm::image::{decode_image_with_resolver, ImageDecoder};
use pal_vm::sprite::{PalVec3, SpriteSystem};
use sha2::{Digest, Sha256};

fn report<T>(
    stage: &str,
    iterations: usize,
    mut run: impl FnMut() -> anyhow::Result<T>,
) -> anyhow::Result<()> {
    for _ in 0..10 {
        black_box(run()?);
    }
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        let value = run()?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        black_box(value);
        samples.push(elapsed);
    }
    samples.sort_by(f64::total_cmp);
    let p95 = (iterations * 95).div_ceil(100) - 1;
    println!(
        "  {stage}: p50_ms={:.6} p95_ms={:.6}",
        samples[iterations / 2],
        samples[p95],
    );
    Ok(())
}

fn delta_base(bytes: &[u8]) -> anyhow::Result<Option<String>> {
    if !bytes.starts_with(b"PGD3") {
        return Ok(None);
    }
    let field = bytes
        .get(0x0e..0x30)
        .ok_or_else(|| anyhow::anyhow!("PGD3 header truncated"))?;
    let end = field
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(field.len());
    anyhow::ensure!(end > 0, "PGD3 base name is empty");
    Ok(Some(String::from_utf8_lossy(&field[..end]).into_owned()))
}

// Include the complete encoded dependency chain in a candidate final-image key.
// A changed base must invalidate a hit even when the delta itself is unchanged.
fn dependency_key(
    bytes: &[u8],
    resources: &mut ResourceManager,
    depth: usize,
) -> anyhow::Result<[u8; 32]> {
    anyhow::ensure!(depth < 32, "PGD3 dependency chain is too deep or cyclic");
    let mut hash = Sha256::new();
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    if let Some(name) = delta_base(bytes)? {
        let base = resources.open(&name)?;
        hash.update(dependency_key(&base.bytes, resources, depth + 1)?);
    }
    Ok(hash.finalize().into())
}

fn capture_chain(
    bytes: &[u8],
    resources: &mut ResourceManager,
    depth: usize,
) -> anyhow::Result<Vec<Vec<u8>>> {
    anyhow::ensure!(depth < 32, "PGD3 dependency chain is too deep or cyclic");
    let mut chain = vec![bytes.to_vec()];
    if let Some(name) = delta_base(bytes)? {
        let base = resources.open(&name)?;
        chain.extend(capture_chain(&base.bytes, resources, depth + 1)?);
    }
    Ok(chain)
}

// A name-indexed cache could retain encoded inputs and compare every byte on a
// hit instead of recomputing SHA-256. Account for these bytes in its memory cap.
fn matches_chain(
    bytes: &[u8],
    expected: &[Vec<u8>],
    open_base: &mut impl FnMut(&str) -> anyhow::Result<Vec<u8>>,
) -> anyhow::Result<bool> {
    let Some((original, ancestors)) = expected.split_first() else {
        return Ok(false);
    };
    if bytes != original {
        return Ok(false);
    }
    match delta_base(bytes)? {
        Some(name) => matches_chain(&open_base(&name)?, ancestors, open_base),
        None => Ok(ancestors.is_empty()),
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: bench_asset_stages ROOT ITERATIONS RESOURCE..."))?;
    let iterations: usize = args.next().unwrap_or_else(|| "100".into()).parse()?;
    anyhow::ensure!(
        (20..=1000).contains(&iterations),
        "iterations must be 20..1000"
    );
    let names: Vec<_> = args.collect();
    anyhow::ensure!(!names.is_empty(), "supply at least one resource name");
    let mut resources = ResourceManager::bootstrap(root, Nls::ShiftJis)?;
    let mut decoder = ImageDecoder::default();
    for name in names {
        let asset = resources.open(&name)?;
        let expected =
            decode_image_with_resolver(&asset.bytes, &mut |base| Ok(resources.open(base)?.bytes))?;
        let warm = decoder.decode(&asset.bytes, &mut |base| Ok(resources.open(base)?.bytes))?;
        anyhow::ensure!(
            warm.rgba == expected.rgba,
            "cached RGBA mismatch for {name}"
        );
        let key = dependency_key(&asset.bytes, &mut resources, 0)?;
        let encoded_chain = capture_chain(&asset.bytes, &mut resources, 0)?;
        let pixels = Arc::new(expected.rgba.clone());
        let mut sprites = SpriteSystem::new();
        println!(
            "{name}: encoded_bytes={} rgba_bytes={} dimensions={}x{} delta_base={:?} source={:?}",
            asset.bytes.len(),
            expected.rgba.len(),
            expected.width,
            expected.height,
            delta_base(&asset.bytes)?,
            asset.source,
        );
        report("resource_open", iterations, || Ok(resources.open(&name)?))?;
        if let AssetSource::Pac { pac_path, key } = &asset.source {
            let pac = PacArchive::from_file(pac_path)?;
            report("direct_pac_read", iterations, || {
                pac.read_key(key)?
                    .ok_or_else(|| anyhow::anyhow!("PAC entry disappeared"))
            })?;
        }
        report("encoded_sha256", iterations, || {
            Ok(Sha256::digest(black_box(&asset.bytes)))
        })?;
        report(
            "rgba_clone",
            iterations,
            || Ok(black_box(&expected).clone()),
        )?;
        report("shared_pixels_clone", iterations, || {
            Ok(Arc::clone(black_box(&pixels)))
        })?;
        report("cached_decode_top_encoded_in_memory", iterations, || {
            decoder.decode(&asset.bytes, &mut |base| Ok(resources.open(base)?.bytes))
        })?;
        report("current_cached_load", iterations, || {
            let current = resources.open(&name)?;
            decoder.decode(&current.bytes, &mut |base| Ok(resources.open(base)?.bytes))
        })?;
        report("candidate_validated_shared_hit", iterations, || {
            let current = resources.open(&name)?;
            let current_key = dependency_key(&current.bytes, &mut resources, 0)?;
            anyhow::ensure!(
                current_key == key,
                "resource changed during the hit-only probe"
            );
            Ok(Arc::clone(&pixels))
        })?;
        report("candidate_validated_owned_hit", iterations, || {
            let current = resources.open(&name)?;
            let current_key = dependency_key(&current.bytes, &mut resources, 0)?;
            anyhow::ensure!(
                current_key == key,
                "resource changed during the hit-only probe"
            );
            Ok(black_box(&expected).clone())
        })?;
        println!(
            "  candidate_retained_encoded_bytes={}",
            encoded_chain.iter().map(Vec::len).sum::<usize>()
        );
        report("candidate_byte_validated_shared_hit", iterations, || {
            let current = resources.open(&name)?;
            anyhow::ensure!(
                matches_chain(&current.bytes, &encoded_chain, &mut |base| {
                    Ok(resources.open(base)?.bytes)
                })?,
                "resource changed during the hit-only probe"
            );
            Ok(Arc::clone(&pixels))
        })?;
        report("candidate_byte_validated_shared_sprite", iterations, || {
            let current = resources.open(&name)?;
            anyhow::ensure!(
                matches_chain(&current.bytes, &encoded_chain, &mut |base| {
                    Ok(resources.open(base)?.bytes)
                })?,
                "resource changed during the hit-only probe"
            );
            let handle = sprites
                .create_shared_rgba_sprite(
                    expected.width,
                    expected.height,
                    Arc::clone(&pixels),
                    PalVec3::default(),
                    0,
                    &name,
                )
                .ok_or_else(|| anyhow::anyhow!("shared sprite creation failed"))?;
            // Include release overhead rather than retaining hundreds of sprites.
            anyhow::ensure!(sprites.release(handle), "shared sprite release failed");
            Ok(handle)
        })?;
        if let Some(base_name) = delta_base(&asset.bytes)? {
            let base = resources.open(&base_name)?;
            let base_image = decoder.decode(&base.bytes, &mut |ancestor| {
                Ok(resources.open(ancestor)?.bytes)
            })?;
            println!(
                "  base={base_name} encoded_bytes={} rgba_bytes={}",
                base.bytes.len(),
                base_image.rgba.len()
            );
            report("base_resource_open", iterations, || {
                Ok(resources.open(&base_name)?)
            })?;
            report("base_encoded_sha256", iterations, || {
                Ok(Sha256::digest(black_box(&base.bytes)))
            })?;
            report("base_cached_decode_encoded_in_memory", iterations, || {
                decoder.decode(&base.bytes, &mut |ancestor| {
                    Ok(resources.open(ancestor)?.bytes)
                })
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(name: &str) -> Vec<u8> {
        let mut bytes = vec![0; 0x38];
        bytes[..4].copy_from_slice(b"PGD3");
        bytes[0x0e..0x0e + name.len()].copy_from_slice(name.as_bytes());
        bytes
    }

    #[test]
    fn byte_validation_rejects_changed_delta_base_and_nested_base() {
        let top = delta("middle");
        let middle = delta("base");
        let base = vec![10, 20, 30, 40];
        let chain = vec![top.clone(), middle.clone(), base.clone()];
        let mut open = |name: &str| {
            Ok(if name == "middle" {
                middle.clone()
            } else {
                base.clone()
            })
        };
        assert!(matches_chain(&top, &chain, &mut open).unwrap());

        let mut changed_top = top.clone();
        changed_top[0x37] ^= 1;
        assert!(!matches_chain(&changed_top, &chain, &mut |_| anyhow::bail!(
            "must not open"
        ))
        .unwrap());

        let mut changed_middle = middle.clone();
        changed_middle[0x37] ^= 1;
        assert!(!matches_chain(&top, &chain, &mut |_| Ok(changed_middle.clone())).unwrap());

        let mut changed_base = base.clone();
        changed_base[3] ^= 1;
        assert!(!matches_chain(&top, &chain, &mut |name| {
            Ok(if name == "middle" {
                middle.clone()
            } else {
                changed_base.clone()
            })
        })
        .unwrap());
        assert!(matches_chain(&top, &chain, &mut |_| anyhow::bail!("missing base")).is_err());
    }

    #[test]
    fn shared_sprite_mutation_does_not_change_cached_pixels() {
        let pixels = Arc::new(vec![10, 20, 30, 255]);
        let mut sprites = SpriteSystem::new();
        let handle = sprites
            .create_shared_rgba_sprite(1, 1, Arc::clone(&pixels), PalVec3::default(), 0, "probe")
            .unwrap();
        let id = sprites.get(handle).unwrap().surface;
        let surface = sprites.surface_mut(id).unwrap();
        assert!(Arc::ptr_eq(&pixels, &surface.pixels));
        surface.pixels_mut()[0] = 99;
        assert!(!Arc::ptr_eq(&pixels, &surface.pixels));
        assert_eq!(pixels.as_slice(), [10, 20, 30, 255]);
        assert_eq!(surface.pixels.as_slice(), [99, 20, 30, 255]);
    }
}
