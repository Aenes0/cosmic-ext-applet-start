// Copyright 2023 System76 <info@system76.com>, 2026 Aenes0
// SPDX-License-Identifier: GPL-3.0-only

//! Pure picture helpers of the window capture.

use cosmic::cctk::screencopy::Formats;
use cosmic::cctk::wayland_client::protocol::wl_shm;

/// The layout does not describe a usable picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShrinkError;

/// Shrinks a raw RGBA picture to at most `max` pixels on its longer side.
pub fn shrink(
    raw: &[u8],
    w: u32,
    h: u32,
    stride: u32,
    max: u32,
    opaque: bool,
) -> Result<image::RgbaImage, ShrinkError> {
    let row = u128::from(w) * 4;
    let need = u128::from(stride) * u128::from(h).saturating_sub(1) + row;
    if w == 0 || h == 0 || max == 0 || u128::from(stride) < row || (raw.len() as u128) < need {
        return Err(ShrinkError);
    }
    // The layout is checked: every pixel read below is inside `raw`.
    let px = |x: u32, y: u32| -> [u8; 4] {
        let off = (u64::from(y) * u64::from(stride) + u64::from(x) * 4) as usize;
        match raw.get(off..off.saturating_add(4)) {
            Some(&[r, g, b, a]) => [r, g, b, if opaque { 255 } else { a }],
            _ => [0; 4],
        }
    };
    if w.max(h) <= max {
        return Ok(image::RgbaImage::from_fn(w, h, |x, y| {
            image::Rgba(px(x, y))
        }));
    }
    Ok(downscale((w, h), max, px))
}

/// Most pixels are never read: a 2x2 average every `k` pixels takes the
/// picture to about twice the thumbnail, the box filter does the rest.
fn downscale(size: (u32, u32), max: u32, px: impl Fn(u32, u32) -> [u8; 4]) -> image::RgbaImage {
    let (w, h) = size;
    let k = (w.max(h) / max.saturating_mul(2).max(1)).max(1);
    let (dw, dh) = ((w / k).max(1), (h / k).max(1));
    let next = u32::from(k > 1);
    let small = image::RgbaImage::from_fn(dw, dh, |x, y| {
        let (x0, y0) = (x * k, y * k);
        let (x1, y1) = ((x0 + next).min(w - 1), (y0 + next).min(h - 1));
        let corners = [px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1)];
        image::Rgba(std::array::from_fn(|ch| {
            (corners.iter().map(|p| u16::from(p[ch])).sum::<u16>() / 4) as u8
        }))
    });
    let scale = max as f32 / dw.max(dh) as f32;
    let width = ((dw as f32 * scale).round() as u32).clamp(1, dw);
    let height = ((dh as f32 * scale).round() as u32).clamp(1, dh);
    image::imageops::thumbnail(&small, width, height)
}

/// Pixel format picked for the capture buffer, and whether alpha is unused.
pub fn pick_format(formats: &Formats) -> Option<(wl_shm::Format, bool)> {
    [
        (wl_shm::Format::Abgr8888, false),
        (wl_shm::Format::Xbgr8888, true),
    ]
    .into_iter()
    .find(|(format, _)| formats.shm_formats.contains(format))
}
