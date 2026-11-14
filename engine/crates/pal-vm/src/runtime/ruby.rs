//! ADV ruby is an annotation above visible base glyphs, not revealable text.
use super::*;

pub(super) struct Annotation {
    start: usize,
    end: usize,
    reading: String,
}

#[derive(Clone, Debug)]
pub(super) struct RubyImage {
    pub line: usize,
    pub base_x: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub dense: Option<Vec<u8>>,
}

pub(super) fn extract(font: &PalFontSystem, text: &str) -> (String, Vec<Annotation>) {
    let mut body = String::with_capacity(text.len());
    let mut annotations = Vec::new();
    let mut open = None;
    let mut position = 0;
    for (unit, visible) in font.layout_units(text) {
        if let Some(tag) = unit.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
            if tag == "/r" {
                if let Some((start, reading)) = open.take() {
                    if position > start {
                        annotations.push(Annotation { start, end: position, reading });
                    }
                    continue;
                }
            } else if let Some(reading) = tag.strip_prefix('r') {
                if !reading.is_empty() && !reading.starts_with('/') {
                    open = Some((position, reading.trim_start_matches('=').to_owned()));
                    continue;
                }
            }
        }
        body.push_str(unit);
        if visible && unit != "\n" && unit != "\r" {
            position += 1;
        }
    }
    (body, annotations)
}

/// Reserve annotation ascent before composing glyphs, reveal masks or TIPS.
/// Reuse the existing inter-line gap before adding any extra space.
pub(super) fn reserve_headroom(
    font: &PalFontSystem,
    annotations: &[Annotation],
    width: u32,
    height: u32,
    pixels: &mut Vec<u8>,
    lines: &mut [AdvTextLineLayout],
) -> u32 {
    let images = rasterize(font, annotations, lines, width, None);
    if images.is_empty() {
        return height;
    }
    let old = lines.to_vec();
    let mut offset = 0;
    for (index, line) in lines.iter_mut().enumerate() {
        let ascent = images.iter().filter(|image| image.line == index)
            .map(|image| image.height + 2).max().unwrap_or(0);
        let gap = index.checked_sub(1).map(|previous|
            old[index].y.saturating_sub(old[previous].y + old[previous].height)).unwrap_or(0);
        offset += ascent.saturating_sub(gap);
        line.y += offset;
    }
    let new_height = height + offset;
    let mut shifted = vec![0; (width * new_height * 4) as usize];
    for (before, after) in old.iter().zip(lines) {
        let length = (width * before.height * 4) as usize;
        let source = (width * before.y * 4) as usize;
        let destination = (width * after.y * 4) as usize;
        shifted[destination..destination + length].copy_from_slice(&pixels[source..source + length]);
    }
    *pixels = shifted;
    new_height
}

pub(super) fn rasterize(
    font: &PalFontSystem,
    annotations: &[Annotation],
    lines: &[AdvTextLineLayout],
    text_width: u32,
    density: Option<u32>,
) -> Vec<RubyImage> {
    let mut font = font.clone();
    font.set_font_size((font.font_size() / 2).max(1));
    // A one-pixel outline keeps the small reading open at logical resolution.
    font.set_effect(1);
    let mut images = Vec::new();
    for annotation in annotations {
        let reading: Vec<_> = annotation.reading.chars().collect();
        for (index, line) in lines.iter().enumerate() {
            let start = annotation.start.max(line.char_start);
            let end = annotation.end.min(line.char_start + line.char_count);
            if start >= end {
                continue;
            }
            let count = annotation.end - annotation.start;
            let from = (start - annotation.start) * reading.len() / count;
            let to = (end - annotation.start) * reading.len() / count;
            let text: String = reading[from..to].iter().collect();
            if text.is_empty() {
                continue;
            }
            let (width, height, pixels) = font.rasterize(&text);
            let x0 = line.char_x[start - line.char_start];
            let x1 = line.char_x[end - line.char_start];
            let x = ((i64::from(x0) + i64::from(x1) - i64::from(width)) / 2)
                .clamp(0, i64::from(text_width.saturating_sub(width))) as i32;
            images.push(RubyImage {
                line: index,
                base_x: x0,
                x,
                y: line.y as i32 - height as i32 - 2,
                width,
                height,
                pixels,
                dense: density.map(|density| font.rasterize_dense(&text, density).2),
            });
        }
    }
    images
}

pub(super) fn compose(
    cache: &AdvTextPanelCache,
    density: u32,
    rgba: &mut [u8],
    reveal: Option<(usize, u32)>,
) {
    let d = density as i32;
    let width = cache.panel_width * density;
    let height = cache.panel_height * density;
    for image in &cache.ruby {
        let limit = match reveal {
            Some((line, _)) if image.line > line => 0,
            Some((line, x)) if image.line == line => x,
            _ => u32::MAX,
        };
        if limit <= image.base_x {
            continue;
        }
        let pixels = if density > 1 {
            image.dense.as_deref().unwrap_or(&image.pixels)
        } else {
            &image.pixels
        };
        for sy in 0..image.height * density {
            let y = (cache.text_origin_y as i32 + image.y) * d + sy as i32;
            if y < 0 || y >= height as i32 {
                continue;
            }
            for sx in 0..image.width * density {
                let local_x = image.x * d + sx as i32;
                if i64::from(local_x) >= i64::from(limit) * i64::from(density) {
                    continue;
                }
                let x = cache.text_origin_x as i32 * d + local_x;
                if x < 0 || x >= width as i32 {
                    continue;
                }
                let source = ((sy * image.width * density + sx) * 4) as usize;
                if let Some(pixel) = pixels.get(source..source + 4) {
                    let destination = ((y as u32 * width + x as u32) * 4) as usize;
                    alpha_blend_rgba(&mut rgba[destination..destination + 4], pixel);
                }
            }
        }
    }
}
