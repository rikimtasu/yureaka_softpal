use std::borrow::Cow;

use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub rgba: Vec<u8>,
}

pub fn decode_image(bytes: &[u8]) -> anyhow::Result<DecodedImage> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return png::decode(bytes);
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return jpeg::decode(bytes);
    }
    if bytes.starts_with(b"BM") {
        return bmp::decode(bytes);
    }
    if bytes.len() >= 0x20 && bytes.get(0..3) == Some(b"GE ") {
        return decode_pgd_ge(bytes);
    }
    if bytes.len() >= 0x38 && bytes.get(0..4) == Some(b"PGD3") {
        anyhow::bail!("PGD3 delta image requires a base-resource resolver");
    }
    if looks_like_tga(bytes) {
        return decode_tga(bytes);
    }
    anyhow::bail!("unsupported image signature");
}

/// Reuse decoded images, including completed deltas, across sprite loads.
/// Delta keys include freshly resolved base content, so edits stay visible.
#[derive(Debug)]
pub struct ImageDecoder {
    cache: crate::asset_cache::ContentCache<DecodedImage>,
}

impl Default for ImageDecoder {
    fn default() -> Self { Self { cache: crate::asset_cache::ContentCache::new(128 * 1024 * 1024) } }
}

impl ImageDecoder {
    pub fn decode<F>(&mut self, bytes: &[u8], resolver: &mut F) -> anyhow::Result<DecodedImage>
    where F: FnMut(&str) -> anyhow::Result<Vec<u8>> {
        if bytes.len() >= 0x38 && bytes.starts_with(b"PGD3") {
            if !self.cache.can_store(0) {
                return decode_image_with_resolver(bytes, resolver);
            }
            return self.decode_delta_cached(bytes, resolver);
        }
        let key = crate::asset_cache::ContentCache::<DecodedImage>::key(bytes);
        if let Some(image) = self.cache.get(&key) { return Ok(image); }
        let image = decode_image(bytes)?;
        self.cache_image(key, &image);
        Ok(image)
    }

    fn decode_delta_cached<F>(
        &mut self,
        bytes: &[u8],
        resolver: &mut F,
    ) -> anyhow::Result<DecodedImage>
    where
        F: FnMut(&str) -> anyhow::Result<Vec<u8>>,
    {
        let mut inputs = vec![ImageCacheInput {
            bytes: Cow::Borrowed(bytes),
            key: crate::asset_cache::ContentCache::<DecodedImage>::key(bytes),
        }];
        // Resolve and hash the whole dependency chain before checking a final
        // image hit. Keep these reads for decoding a miss without reopening it.
        while let Some(input) = inputs.last() {
            if input.bytes.len() < 0x38 || !input.bytes.starts_with(b"PGD3") {
                break;
            }
            let name = pgd3_base_name(&input.bytes)?;
            let base = resolver(&name)
                .map_err(|err| anyhow::anyhow!("PGD3 base {name:?} decode failed: {err}"))?;
            let key = crate::asset_cache::ContentCache::<DecodedImage>::key(&base);
            anyhow::ensure!(
                !inputs.iter().any(|input| input.key == key),
                "PGD3 cyclic base dependency at {name:?}"
            );
            inputs.push(ImageCacheInput { bytes: Cow::Owned(base), key });
        }
        for index in (0..inputs.len() - 1).rev() {
            inputs[index].key = completed_delta_key(&inputs[index].key, &inputs[index + 1].key);
        }
        // Start from the nearest cached result, not a cloned base when the
        // completed top-level delta is already available.
        let cached = inputs.iter().enumerate().find_map(|(index, input)| {
            self.cache.get(&input.key).map(|image| (image, index))
        });
        let (mut image, pending) = match cached {
            Some(cached) => cached,
            None => {
                let index = inputs.len() - 1;
                let image = decode_image(&inputs[index].bytes)?;
                self.cache_image(inputs[index].key, &image);
                (image, index)
            }
        };
        for input in inputs[..pending].iter().rev() {
            let mut base = Some(image);
            image = decode_pgd3_with_base(&input.bytes, &mut |_| {
                base.take().ok_or_else(|| anyhow::anyhow!("PGD3 decoded base already consumed"))
            })?;
            self.cache_image(input.key, &image);
        }
        Ok(image)
    }

    fn cache_image(&mut self, key: [u8; 32], image: &DecodedImage) {
        if self.cache.can_store(image.rgba.len()) {
            self.cache.insert(key, image.clone(), image.rgba.len());
        }
    }
}

struct ImageCacheInput<'a> {
    bytes: Cow<'a, [u8]>,
    key: [u8; 32],
}

fn completed_delta_key(encoded_key: &[u8; 32], base_key: &[u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"PAL-PGD3-CACHE\0");
    hash.update(encoded_key);
    hash.update(base_key);
    hash.finalize().into()
}

mod bmp;
mod jpeg;
mod png;

/// Limit the new general-purpose image decoders before allocating pixel data.
/// Decoder work buffers have a separate limit; this bounds the final RGBA image.
const MAX_RASTER_BYTES: usize = 256 * 1024 * 1024;

fn bounded_rgba_len(width: u32, height: u32) -> anyhow::Result<usize> {
    let len = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|&len| len > 0 && len <= MAX_RASTER_BYTES as u64)
        .ok_or_else(|| anyhow::anyhow!("image dimensions exceed decoded size limit"))?;
    Ok(usize::try_from(len)?)
}

pub fn decode_image_with_resolver<F>(bytes: &[u8], resolver: &mut F) -> anyhow::Result<DecodedImage>
where
    F: FnMut(&str) -> anyhow::Result<Vec<u8>>,
{
    if bytes.len() >= 0x38 && bytes.get(0..4) == Some(b"PGD3") {
        return decode_pgd3_delta(bytes, resolver);
    }
    decode_image(bytes)
}

fn decode_pgd3_delta<F>(bytes: &[u8], resolver: &mut F) -> anyhow::Result<DecodedImage>
where
    F: FnMut(&str) -> anyhow::Result<Vec<u8>>,
{
    decode_pgd3_with_base(bytes, &mut |name| {
        let base = resolver(name)?;
        decode_image_with_resolver(&base, resolver)
    })
}

fn decode_pgd3_with_base(
    bytes: &[u8], load_base: &mut impl FnMut(&str) -> anyhow::Result<DecodedImage>,
) -> anyhow::Result<DecodedImage> {
    let offset_x = read_u16(bytes, 0x04)? as i16 as i32;
    let offset_y = read_u16(bytes, 0x06)? as i16 as i32;
    let width = read_u16(bytes, 0x08)? as usize;
    let height = read_u16(bytes, 0x0A)? as usize;
    let bpp = read_u16(bytes, 0x0C)?;
    let base_name = pgd3_base_name(bytes)?;
    let unpacked_size = read_u32(bytes, 0x30)? as usize;
    let packed_size = read_u32(bytes, 0x34)? as usize;
    let data_end = 0x38usize
        .checked_add(packed_size)
        .ok_or_else(|| anyhow::anyhow!("PGD3 packed stream size overflow"))?;
    if data_end > bytes.len() {
        anyhow::bail!(
            "PGD3 packed stream out of bounds: end {data_end}, len {}",
            bytes.len()
        );
    }

    let pixel_size = match bpp {
        24 => 3,
        32 => 4,
        _ => anyhow::bail!("unsupported PGD3 bpp {bpp}"),
    };
    let mut base = load_base(&base_name)
        .map_err(|err| anyhow::anyhow!("PGD3 base {base_name:?} decode failed: {err}"))?;
    let mut stream = GeStream::new(Cursor::new(&bytes[..data_end], 0x38), unpacked_size);
    xor_pgd3_stream(&mut base, offset_x, offset_y, width, height, pixel_size, &mut stream)?;
    Ok(base)
}

fn pgd3_base_name(bytes: &[u8]) -> anyhow::Result<String> {
    let name_bytes = bytes
        .get(0x0E..0x30)
        .ok_or_else(|| anyhow::anyhow!("PGD3 base name out of bounds"))?;
    let end = name_bytes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(name_bytes.len());
    if end == 0 {
        anyhow::bail!("PGD3 base name is empty");
    }
    Ok(String::from_utf8_lossy(&name_bytes[..end]).into_owned())
}

fn looks_like_tga(bytes: &[u8]) -> bool {
    if bytes.len() < 18 {
        return false;
    }
    let color_map_type = bytes[1];
    let image_type = bytes[2];
    let width = u16::from_le_bytes([bytes[12], bytes[13]]);
    let height = u16::from_le_bytes([bytes[14], bytes[15]]);
    let bpp = bytes[16];
    width != 0
        && height != 0
        && color_map_type <= 1
        && matches!(image_type, 2 | 3 | 10 | 11)
        && matches!(bpp, 8 | 24 | 32)
}

fn decode_tga(bytes: &[u8]) -> anyhow::Result<DecodedImage> {
    if bytes.len() < 18 {
        anyhow::bail!("TGA header truncated");
    }
    let id_len = bytes[0] as usize;
    let color_map_type = bytes[1];
    let image_type = bytes[2];
    let color_map_len = u16::from_le_bytes([bytes[5], bytes[6]]) as usize;
    let color_map_bpp = bytes[7] as usize;
    let width = u16::from_le_bytes([bytes[12], bytes[13]]) as usize;
    let height = u16::from_le_bytes([bytes[14], bytes[15]]) as usize;
    let bpp = bytes[16] as usize;
    let descriptor = bytes[17];
    if width == 0 || height == 0 {
        anyhow::bail!("TGA has empty dimensions");
    }
    if color_map_type != 0 {
        anyhow::bail!("TGA color maps are not supported");
    }
    let pixel_bytes = match bpp {
        8 => 1,
        24 => 3,
        32 => 4,
        _ => anyhow::bail!("unsupported TGA bpp {bpp}"),
    };
    let color_map_bytes = color_map_len
        .checked_mul(color_map_bpp.saturating_add(7) / 8)
        .ok_or_else(|| anyhow::anyhow!("TGA color map size overflow"))?;
    let mut pos = 18usize
        .checked_add(id_len)
        .and_then(|value| value.checked_add(color_map_bytes))
        .ok_or_else(|| anyhow::anyhow!("TGA data offset overflow"))?;
    if pos > bytes.len() {
        anyhow::bail!("TGA data offset outside file");
    }
    let pixel_count = width
        .checked_mul(height)
        .ok_or_else(|| anyhow::anyhow!("TGA dimensions overflow"))?;
    let mut rgba = vec![0u8; pixel_count * 4];
    let origin_top = (descriptor & 0x20) != 0;
    let origin_right = (descriptor & 0x10) != 0;
    let mut write_pixel = |index: usize, px: &[u8]| -> anyhow::Result<()> {
        let x = index % width;
        let y = index / width;
        let dst_x = if origin_right { width - 1 - x } else { x };
        let dst_y = if origin_top { y } else { height - 1 - y };
        let dst = (dst_y * width + dst_x) * 4;
        match pixel_bytes {
            1 => {
                let g = px[0];
                rgba[dst..dst + 4].copy_from_slice(&[g, g, g, 255]);
            }
            3 => {
                rgba[dst..dst + 4].copy_from_slice(&[px[2], px[1], px[0], 255]);
            }
            4 => {
                rgba[dst..dst + 4].copy_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
            _ => unreachable!(),
        }
        Ok(())
    };

    match image_type {
        2 | 3 => {
            let need = pixel_count
                .checked_mul(pixel_bytes)
                .ok_or_else(|| anyhow::anyhow!("TGA pixel data size overflow"))?;
            if pos.checked_add(need).is_none_or(|end| end > bytes.len()) {
                anyhow::bail!("TGA pixel data truncated");
            }
            for index in 0..pixel_count {
                let start = pos + index * pixel_bytes;
                write_pixel(index, &bytes[start..start + pixel_bytes])?;
            }
        }
        10 | 11 => {
            let mut index = 0usize;
            while index < pixel_count {
                let Some(&header) = bytes.get(pos) else {
                    anyhow::bail!("TGA RLE packet header truncated");
                };
                pos += 1;
                let count = (usize::from(header & 0x7F)).saturating_add(1);
                if (header & 0x80) != 0 {
                    if pos
                        .checked_add(pixel_bytes)
                        .is_none_or(|end| end > bytes.len())
                    {
                        anyhow::bail!("TGA RLE pixel truncated");
                    }
                    let px = &bytes[pos..pos + pixel_bytes];
                    pos += pixel_bytes;
                    for _ in 0..count {
                        if index >= pixel_count {
                            anyhow::bail!("TGA RLE packet overruns image");
                        }
                        write_pixel(index, px)?;
                        index += 1;
                    }
                } else {
                    for _ in 0..count {
                        if index >= pixel_count {
                            anyhow::bail!("TGA raw packet overruns image");
                        }
                        if pos
                            .checked_add(pixel_bytes)
                            .is_none_or(|end| end > bytes.len())
                        {
                            anyhow::bail!("TGA raw pixel truncated");
                        }
                        write_pixel(index, &bytes[pos..pos + pixel_bytes])?;
                        pos += pixel_bytes;
                        index += 1;
                    }
                }
            }
        }
        _ => anyhow::bail!("unsupported TGA image type {image_type}"),
    }

    Ok(DecodedImage {
        width: width as u32,
        height: height as u32,
        cell_width: width as u32,
        cell_height: height as u32,
        offset_x: 0,
        offset_y: 0,
        rgba,
    })
}

/// The LZ token stores a 12-bit distance. A 4 KiB window replaces the full
/// unpacked patch, which can otherwise exceed 64 MiB beside its decoded base.
struct GeStream<'a> {
    input: Cursor<'a>,
    history: [u8; 4096],
    size: usize,
    produced: usize,
    control: u16,
    run: usize,
    distance: usize,
}

impl<'a> GeStream<'a> {
    fn new(input: Cursor<'a>, size: usize) -> Self {
        Self { input, history: [0; 4096], size, produced: 0,
            control: 2, run: 0, distance: 0 }
    }

    #[cfg(test)]
    fn read(&mut self) -> anyhow::Result<u8> {
        let mut byte = [0];
        self.read_exact(&mut byte)?;
        Ok(byte[0])
    }

    fn start_run(&mut self) -> anyhow::Result<()> {
        while self.run == 0 {
            self.control >>= 1;
            if self.control == 1 {
                self.control = u16::from(self.input.read_u8()?) | 0x100;
            }
            if self.control & 1 != 0 {
                let raw = usize::from(self.input.read_u16()?);
                let mut count = raw & 7;
                if raw & 8 == 0 {
                    count = (count << 8) | usize::from(self.input.read_u8()?);
                }
                self.run = count + 4;
                self.distance = raw >> 4;
                if self.distance == 0 || self.distance > self.produced {
                    anyhow::bail!("invalid PGD back-reference distance");
                }
            } else {
                self.run = usize::from(self.input.read_u8()?);
                self.distance = 0;
            }
            if self.run > self.size - self.produced {
                anyhow::bail!("PGD token exceeds unpacked stream");
            }
        }
        Ok(())
    }

    /// Decode token spans into a row, preserving the bounded 4 KiB history.
    /// Overlapping references repeat their initial period rather than copying
    /// one byte through the token state machine at a time.
    fn read_exact(&mut self, out: &mut [u8]) -> anyhow::Result<()> {
        if out.len() > self.size - self.produced {
            anyhow::bail!("PGD unpacked stream exhausted");
        }
        let mut pos = 0;
        while pos < out.len() {
            self.start_run()?;
            let n = self.run.min(out.len() - pos);
            if self.distance == 0 {
                self.input.read_exact(&mut out[pos..pos + n])?;
            } else {
                let seed = self.distance.min(n);
                let source = (self.produced - self.distance) & 4095;
                let first = seed.min(4096 - source);
                out[pos..pos + first].copy_from_slice(&self.history[source..source + first]);
                out[pos + first..pos + seed].copy_from_slice(&self.history[..seed - first]);
                let mut filled = seed;
                while filled < n {
                    let count = filled.min(n - filled);
                    out.copy_within(pos..pos + count, pos + filled);
                    filled += count;
                }
            }
            let keep = n.min(4096);
            let ring = (self.produced + n - keep) & 4095;
            let first = keep.min(4096 - ring);
            let tail = &out[pos + n - keep..pos + n];
            self.history[ring..ring + first].copy_from_slice(&tail[..first]);
            self.history[..keep - first].copy_from_slice(&tail[first..]);
            self.produced += n;
            self.run -= n;
            pos += n;
        }
        Ok(())
    }
}

fn xor_pgd3_stream(
    base: &mut DecodedImage,
    offset_x: i32,
    offset_y: i32,
    width: usize,
    height: usize,
    pixel_size: usize,
    stream: &mut GeStream<'_>,
) -> anyhow::Result<()> {
    if offset_x < 0 || offset_y < 0 || width == 0 || height == 0 {
        anyhow::bail!("PGD3 invalid patch geometry");
    }
    let (offset_x, offset_y) = (offset_x as usize, offset_y as usize);
    if offset_x.saturating_add(width) > base.width as usize
        || offset_y.saturating_add(height) > base.height as usize {
        anyhow::bail!("PGD3 patch exceeds base image");
    }
    visit_pal_rows(stream, width, height, pixel_size, |y, row| {
        let start = ((offset_y + y) * base.width as usize + offset_x) * 4;
        for (dst, patch) in base.rgba[start..start + width * 4].chunks_exact_mut(4)
            .zip(row.chunks_exact(pixel_size)) {
            dst[0] ^= patch[2];
            dst[1] ^= patch[1];
            dst[2] ^= patch[0];
            if pixel_size == 4 { dst[3] ^= patch[3]; }
        }
        Ok(())
    })
}

fn visit_pal_rows(
    stream: &mut GeStream<'_>, width: usize, height: usize, pixel_size: usize,
    mut visit: impl FnMut(usize, &[u8]) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let stride = width.checked_mul(pixel_size)
        .ok_or_else(|| anyhow::anyhow!("PGD row size overflow"))?;
    let required = stride.checked_add(1).and_then(|row| row.checked_mul(height))
        .ok_or_else(|| anyhow::anyhow!("PGD image size overflow"))?;
    if width == 0 || height == 0 || required > stream.size - stream.produced {
        anyhow::bail!("PGD row data out of bounds");
    }
    let mut controls = vec![0; height];
    stream.read_exact(&mut controls)?;
    let mut previous = vec![0u8; stride];
    let mut row = vec![0u8; stride];
    for (y, control) in controls.into_iter().enumerate() {
        if control & 1 == 0 && control & 2 != 0 && y == 0 {
            anyhow::bail!("PGD row references previous row before row 0");
        }
        stream.read_exact(&mut row)?;
        if control & 1 != 0 {
            if pixel_size == 4 { predict_horizontal::<4>(&mut row); }
            else { predict_horizontal::<3>(&mut row); }
        } else if control & 2 != 0 {
            for (pixel, prior) in row.iter_mut().zip(&previous) { *pixel = prior.wrapping_sub(*pixel); }
        } else if y != 0 {
            if pixel_size == 4 { predict_average::<4>(&mut row, &previous); }
            else { predict_average::<3>(&mut row, &previous); }
        }
        visit(y, &row)?;
        std::mem::swap(&mut row, &mut previous);
    }
    let mut tail = [0; 4096];
    while stream.produced < stream.size {
        let n = tail.len().min(stream.size - stream.produced);
        stream.read_exact(&mut tail[..n])?;
    }
    Ok(())
}

fn predict_horizontal<const N: usize>(row: &mut [u8]) {
    let mut left: [u8; N] = row[..N].try_into().unwrap();
    for pixel in row[N..].chunks_exact_mut(N) {
        for channel in 0..N {
            left[channel] = left[channel].wrapping_sub(pixel[channel]);
            pixel[channel] = left[channel];
        }
    }
}

fn predict_average<const N: usize>(row: &mut [u8], previous: &[u8]) {
    let mut left: [u8; N] = row[..N].try_into().unwrap();
    for (pixel, above) in row[N..].chunks_exact_mut(N).zip(previous[N..].chunks_exact(N)) {
        for channel in 0..N {
            let value = ((u16::from(above[channel]) + u16::from(left[channel])) / 2) as u8;
            left[channel] = value.wrapping_sub(pixel[channel]);
            pixel[channel] = left[channel];
        }
    }
}

fn decode_pgd_ge(bytes: &[u8]) -> anyhow::Result<DecodedImage> {
    let offset_x = read_u32(bytes, 0x04)? as i32;
    let offset_y = read_u32(bytes, 0x08)? as i32;
    let mut width = read_u32(bytes, 0x0C)? as usize;
    let mut height = read_u32(bytes, 0x10)? as usize;
    let cell_width = read_u32(bytes, 0x14)?.max(1);
    let cell_height = read_u32(bytes, 0x18)?.max(1);
    let method = read_u16(bytes, 0x1C)?;
    let unpacked_size = read_u32(bytes, 0x20)? as usize;
    let rgba = if method == 3 {
        let mut stream = GeStream::new(Cursor::new(bytes, 0x28), unpacked_size);
        let mut header = [0u8; 8];
        stream.read_exact(&mut header)?;
        let bpp = u16::from_le_bytes([header[2], header[3]]);
        width = usize::from(u16::from_le_bytes([header[4], header[5]]));
        height = usize::from(u16::from_le_bytes([header[6], header[7]]));
        let pixel_size = match bpp {
            24 => 3,
            32 => 4,
            _ => anyhow::bail!("unsupported PGD/GE bpp {bpp}"),
        };
        let length = bounded_rgba_len(width as u32, height as u32)?;
        image_allocation_begin(length);
        let mut pixels = vec![0u8; length];
        visit_pal_rows(&mut stream, width, height, pixel_size, |y, row| {
            let start = y * width * 4;
            for (dst, src) in pixels[start..start + width * 4].chunks_exact_mut(4)
                .zip(row.chunks_exact(pixel_size)) {
                dst.copy_from_slice(&[src[2], src[1], src[0], if pixel_size == 4 { src[3] } else { 255 }]);
            }
            Ok(())
        })?;
        pixels
    } else {
        let mut input = Cursor::new(bytes, 0x28);
        let unpacked = unpack_ge_pre(&mut input, unpacked_size)?;
        match method {
            1 => post_process_planes_bgra_to_rgba(&unpacked)?,
            2 => post_process_yuv_like_to_rgba(&unpacked, width, height)?,
            _ => anyhow::bail!("unsupported PGD/GE method {method}"),
        }
    };
    let expected = width
        .checked_mul(height)
        .and_then(|px| px.checked_mul(4))
        .ok_or_else(|| anyhow::anyhow!("PGD/GE dimensions overflow"))?;
    if rgba.len() != expected {
        anyhow::bail!(
            "PGD/GE decoded size mismatch: got {}, expected {}",
            rgba.len(),
            expected
        );
    }
    Ok(DecodedImage {
        width: width as u32,
        height: height as u32,
        cell_width: cell_width.min(width as u32).max(1),
        cell_height: cell_height.min(height as u32).max(1),
        offset_x,
        offset_y,
        rgba,
    })
}

fn image_allocation_begin(bytes: usize) {
    #[cfg(target_os = "vita")]
    if bytes >= 8 * 1024 * 1024 {
        unsafe extern "C" { fn sena_vita_image_allocation(bytes: usize); }
        unsafe { sena_vita_image_allocation(bytes); }
    }
    #[cfg(not(target_os = "vita"))]
    let _ = bytes;
}

#[cfg(test)]
mod stream_tests {
    use super::*;

    fn tga(pixel: [u8; 4]) -> Vec<u8> {
        let mut bytes = vec![0; 18];
        bytes[2] = 2;
        bytes[12] = 1;
        bytes[14] = 1;
        bytes[16] = 32;
        bytes[17] = 0x20;
        bytes.extend([pixel[2], pixel[1], pixel[0], pixel[3]]);
        bytes
    }

    fn delta(base_name: &str, patch_bgra: [u8; 4]) -> Vec<u8> {
        let mut bytes = vec![0; 0x38];
        bytes[..4].copy_from_slice(b"PGD3");
        bytes[8] = 1;
        bytes[10] = 1;
        bytes[12] = 32;
        bytes[14..14 + base_name.len()].copy_from_slice(base_name.as_bytes());
        bytes[0x30] = 5;
        bytes[0x34] = 7;
        bytes.extend([0, 5, 1]);
        bytes.extend(patch_bgra);
        bytes
    }

    #[test]
    fn completed_delta_images_cache_the_final_pixels() {
        let mut decoder = ImageDecoder { cache: crate::asset_cache::ContentCache::new(8) };
        let base = tga([10, 20, 30, 255]);
        let encoded = delta("base", [3, 2, 1, 0]);
        let key = completed_delta_key(
            &crate::asset_cache::ContentCache::<DecodedImage>::key(&encoded),
            &crate::asset_cache::ContentCache::<DecodedImage>::key(&base),
        );
        let mut first = decoder.decode(&encoded, &mut |_| Ok(base.clone())).unwrap();
        assert_eq!(first.rgba, [11, 22, 29, 255]);
        let cached = decoder.cache.get(&key).expect("completed delta image must be cached");
        assert_eq!(cached.rgba, first.rgba);
        first.rgba.fill(0);
        assert_eq!(decoder.decode(&encoded, &mut |_| Ok(base.clone())).unwrap().rgba, cached.rgba);
    }

    #[test]
    fn completed_delta_cache_rechecks_nested_bases_and_changed_patches() {
        use std::collections::HashMap;

        let mut decoder = ImageDecoder::default();
        let top = delta("middle", [6, 5, 4, 0]);
        let mut sources = HashMap::from([
            ("base", tga([10, 20, 30, 255])),
            ("middle", delta("base", [3, 2, 1, 0])),
        ]);
        let load = |decoder: &mut ImageDecoder, bytes: &[u8], sources: &HashMap<&str, Vec<u8>>| {
            decoder.decode(bytes, &mut |name| {
                sources.get(name).cloned().ok_or_else(|| anyhow::anyhow!("missing {name}"))
            })
        };
        assert_eq!(load(&mut decoder, &top, &sources).unwrap().rgba, [15, 19, 27, 255]);
        assert_eq!(load(&mut decoder, &top, &sources).unwrap().rgba, [15, 19, 27, 255]);

        sources.insert("base", tga([40, 50, 60, 255]));
        assert_eq!(load(&mut decoder, &top, &sources).unwrap().rgba, [45, 53, 57, 255]);
        sources.insert("middle", delta("base", [9, 8, 7, 0]));
        assert_eq!(load(&mut decoder, &top, &sources).unwrap().rgba, [43, 63, 51, 255]);
        let changed_top = delta("middle", [12, 11, 10, 0]);
        assert_eq!(load(&mut decoder, &changed_top, &sources).unwrap().rgba, [37, 49, 57, 255]);

        sources.remove("base");
        assert!(load(&mut decoder, &changed_top, &sources).is_err());
    }

    #[test]
    fn completed_deltas_share_the_existing_cache_budget_with_bases() {
        let mut decoder = ImageDecoder { cache: crate::asset_cache::ContentCache::new(8) };
        let base = tga([10, 20, 30, 255]);
        let base_key = crate::asset_cache::ContentCache::<DecodedImage>::key(&base);
        let first = delta("base", [3, 2, 1, 0]);
        let second = delta("base", [6, 5, 4, 0]);
        let first_key = completed_delta_key(
            &crate::asset_cache::ContentCache::<DecodedImage>::key(&first), &base_key,
        );
        let second_key = completed_delta_key(
            &crate::asset_cache::ContentCache::<DecodedImage>::key(&second), &base_key,
        );
        decoder.decode(&first, &mut |_| Ok(base.clone())).unwrap();
        decoder.decode(&second, &mut |_| Ok(base.clone())).unwrap();
        assert!(decoder.cache.get(&first_key).is_none());
        assert!(decoder.cache.get(&base_key).is_some());
        assert!(decoder.cache.get(&second_key).is_some());
        assert_eq!(decoder.decode(&first, &mut |_| Ok(base.clone())).unwrap().rgba, [11, 22, 29, 255]);
        assert!(decoder.cache.get(&second_key).is_none());
        assert!(decoder.cache.get(&first_key).is_some());
    }

    #[test]
    fn completed_delta_cache_stays_disabled_at_zero_budget() {
        let mut decoder = ImageDecoder { cache: crate::asset_cache::ContentCache::new(0) };
        let base = tga([10, 20, 30, 255]);
        let encoded = delta("base", [3, 2, 1, 0]);
        let key = completed_delta_key(
            &crate::asset_cache::ContentCache::<DecodedImage>::key(&encoded),
            &crate::asset_cache::ContentCache::<DecodedImage>::key(&base),
        );
        for _ in 0..2 {
            assert_eq!(decoder.decode(&encoded, &mut |_| Ok(base.clone())).unwrap().rgba, [11, 22, 29, 255]);
            assert!(decoder.cache.get(&key).is_none());
        }
    }

    #[test]
    fn completed_delta_cache_rejects_cyclic_dependencies() {
        let mut decoder = ImageDecoder::default();
        let encoded = delta("base", [3, 2, 1, 0]);
        let error = decoder.decode(&encoded, &mut |_| Ok(encoded.clone())).unwrap_err();
        assert!(error.to_string().contains("cyclic base dependency"));
    }

    #[test]
    fn image_cache_isolates_mutation_and_resolves_changed_delta_bases() {
        let mut decoder = ImageDecoder::default();
        let mut base = tga([10, 20, 30, 255]);
        let mut first = decoder.decode(&base, &mut |_| anyhow::bail!("no base")).unwrap();
        first.rgba.fill(0);
        assert_eq!(decoder.decode(&base, &mut |_| anyhow::bail!("no base")).unwrap().rgba, [10, 20, 30, 255]);
        let mut delta = vec![0; 0x38];
        delta[..4].copy_from_slice(b"PGD3");
        delta[8] = 1;
        delta[10] = 1;
        delta[12] = 32;
        delta[14..18].copy_from_slice(b"base");
        delta[0x30] = 5;
        delta[0x34] = 7;
        delta.extend([0, 5, 1, 0, 0, 0, 0]);
        assert_eq!(decoder.decode(&delta, &mut |_| Ok(base.clone())).unwrap().rgba, [10, 20, 30, 255]);
        base = tga([40, 50, 60, 255]);
        assert_eq!(decoder.decode(&delta, &mut |_| Ok(base.clone())).unwrap().rgba, [40, 50, 60, 255]);
    }

    #[test]
    fn channel_predictors_match_the_byte_reference_for_both_pixel_sizes() {
        for n in [3, 4] {
            for width in [1, 2, 127, 2048] {
                let previous: Vec<_> = (0..width * n).map(|x| (x * 71) as u8).collect();
                let input: Vec<_> = (0..width * n).map(|x| (x * 29) as u8).collect();
                for average in [false, true] {
                    let mut expected = input.clone();
                    for x in n..expected.len() {
                        let prediction = if average {
                            ((u16::from(previous[x]) + u16::from(expected[x - n])) / 2) as u8
                        } else { expected[x - n] };
                        expected[x] = prediction.wrapping_sub(expected[x]);
                    }
                    let mut actual = input.clone();
                    match (n, average) {
                        (3, false) => predict_horizontal::<3>(&mut actual),
                        (4, false) => predict_horizontal::<4>(&mut actual),
                        (3, true) => predict_average::<3>(&mut actual, &previous),
                        _ => predict_average::<4>(&mut actual, &previous),
                    }
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    #[test]
    fn bulk_lz_matches_reference_across_overlap_ring_wrap_and_row_boundaries() {
        let mut tokens = Vec::new();
        for i in 0..24 {
            if i % 4 == 0 {
                let mut literal = vec![255];
                literal.extend((0..255).map(|v| (v + i) as u8));
                tokens.push((false, literal));
            } else {
                let distance = if i < 4 { 1 } else if i % 4 == 1 { 4095 } else { 3 };
                let count = 2047_usize;
                let length = count - 4;
                let raw = ((distance as u16) << 4) | (length >> 8) as u16;
                let mut reference = raw.to_le_bytes().to_vec();
                reference.push(length as u8);
                tokens.push((true, reference));
            }
        }
        let mut packed = Vec::new();
        for group in tokens.chunks(8) {
            packed.push(group.iter().enumerate().fold(0, |mask, (i, (reference, _))| mask | (u8::from(*reference) << i)));
            for (_, bytes) in group { packed.extend(bytes); }
        }
        let size = 6 * 255 + 18 * 2047;
        let expected = unpack_ge_pre(&mut Cursor::new(&packed, 0), size).unwrap();
        for chunk in [1, 3, 31, 4093, 8192] {
            let mut stream = GeStream::new(Cursor::new(&packed, 0), size);
            let mut actual = vec![0; size];
            for out in actual.chunks_mut(chunk) { stream.read_exact(out).unwrap(); }
            assert_eq!(actual, expected, "chunk={chunk}");
            assert!(stream.read().is_err());
        }
    }

    #[test]
    fn bulk_lz_rejects_invalid_references_and_truncated_tokens() {
        for bytes in [&[1, 0x18, 0][..], &[0, 3, 1][..]] {
            let mut stream = GeStream::new(Cursor::new(bytes, 0), 4);
            assert!(stream.read_exact(&mut [0; 4]).is_err());
        }
    }
}

fn unpack_ge_pre(input: &mut Cursor<'_>, unpacked_size: usize) -> anyhow::Result<Vec<u8>> {
    image_allocation_begin(unpacked_size);
    let mut output = vec![0u8; unpacked_size];
    let mut dst = 0usize;
    let mut ctl = 2u16;
    while dst < output.len() {
        ctl >>= 1;
        if ctl == 1 {
            ctl = u16::from(input.read_u8()?) | 0x100;
        }
        let count = if (ctl & 1) != 0 {
            let raw = input.read_u16()? as usize;
            let mut count = raw & 7;
            if (raw & 8) == 0 {
                count = (count << 8) | usize::from(input.read_u8()?);
            }
            count += 4;
            let offset = raw >> 4;
            if offset == 0 || offset > dst {
                anyhow::bail!("invalid PGD/GE back-reference offset {offset} at {dst}");
            }
            copy_overlapped(&mut output, dst - offset, dst, count)?;
            count
        } else {
            let count = usize::from(input.read_u8()?);
            input.read_exact(&mut output[dst..dst + count])?;
            count
        };
        dst = dst.saturating_add(count);
    }
    Ok(output)
}

fn post_process_planes_bgra_to_rgba(input: &[u8]) -> anyhow::Result<Vec<u8>> {
    if input.len() % 4 != 0 {
        anyhow::bail!("PGD/GE method 1 payload is not four planes");
    }
    let plane = input.len() / 4;
    let (a, rest) = input.split_at(plane);
    let (r, rest) = rest.split_at(plane);
    let (g, b) = rest.split_at(plane);
    let mut output = Vec::with_capacity(input.len());
    for i in 0..plane {
        output.extend_from_slice(&[r[i], g[i], b[i], a[i]]);
    }
    Ok(output)
}

fn post_process_yuv_like_to_rgba(
    input: &[u8],
    width: usize,
    height: usize,
) -> anyhow::Result<Vec<u8>> {
    if width % 2 != 0 || height % 2 != 0 {
        anyhow::bail!("PGD/GE method 2 dimensions must be even");
    }
    let segment = width
        .checked_mul(height)
        .ok_or_else(|| anyhow::anyhow!("PGD/GE dimensions overflow"))?
        / 4;
    if input.len() < segment * 3 {
        anyhow::bail!("PGD/GE method 2 payload too short");
    }
    let mut src0 = 0usize;
    let mut src1 = segment;
    let mut src2 = segment * 2;
    let stride = width * 4;
    let mut output = vec![0u8; stride * height];
    let points = [0usize, 1, width, width + 1];
    for y in 0..height / 2 {
        for x in 0..width / 2 {
            let i0 = input[src0] as i8 as i32;
            let i1 = input[src1] as i8 as i32;
            let b_delta = 226 * i0;
            let g_delta = -43 * i0 - 89 * i1;
            let r_delta = 179 * i1;
            src0 += 1;
            src1 += 1;
            for point in points {
                let base = i32::from(input[src2 + point]) << 7;
                let px = ((y * 2) * width + (x * 2) + point) * 4;
                output[px] = clamp_u8((base + r_delta) >> 7);
                output[px + 1] = clamp_u8((base + g_delta) >> 7);
                output[px + 2] = clamp_u8((base + b_delta) >> 7);
                output[px + 3] = 255;
            }
            src2 += 2;
        }
        src2 += width;
    }
    Ok(output)
}


fn copy_overlapped(output: &mut [u8], src: usize, dst: usize, count: usize) -> anyhow::Result<()> {
    let end = dst
        .checked_add(count)
        .ok_or_else(|| anyhow::anyhow!("PGD/GE overlap copy overflow"))?;
    if end > output.len() {
        anyhow::bail!("PGD/GE overlap copy out of bounds");
    }
    for i in 0..count {
        output[dst + i] = output[src + i];
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> anyhow::Result<u16> {
    let raw = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| anyhow::anyhow!("u16 read out of bounds at 0x{offset:X}"))?;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> anyhow::Result<u32> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| anyhow::anyhow!("u32 read out of bounds at 0x{offset:X}"))?;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn clamp_u8(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], pos: usize) -> Self {
        Self { bytes, pos }
    }

    fn read_u8(&mut self) -> anyhow::Result<u8> {
        let byte = *self
            .bytes
            .get(self.pos)
            .ok_or_else(|| anyhow::anyhow!("PGD/GE input out of bounds"))?;
        self.pos += 1;
        Ok(byte)
    }

    fn read_u16(&mut self) -> anyhow::Result<u16> {
        let value = read_u16(self.bytes, self.pos)?;
        self.pos += 2;
        Ok(value)
    }

    fn read_exact(&mut self, dst: &mut [u8]) -> anyhow::Result<()> {
        let end = self
            .pos
            .checked_add(dst.len())
            .ok_or_else(|| anyhow::anyhow!("PGD/GE input range overflow"))?;
        let src = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| anyhow::anyhow!("PGD/GE input range out of bounds"))?;
        dst.copy_from_slice(src);
        self.pos = end;
        Ok(())
    }
}
