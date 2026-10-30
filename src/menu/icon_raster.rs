//! Pre-rasterized app icons for the CPU renderer.
//!
//! The stock path redraws every icon from its source (SVG tree or a big PNG)
//! whenever the tiny-skia caches miss. Here an icon is drawn once, at load,
//! by the same steps the renderer takes, and kept as a small raster the
//! renderer only copies. Anything this module cannot match to the pixel is
//! left to the stock path (`None`).

use super::*;
use cosmic::iced::{
    advanced::graphics::{image as gimage, text as gtext},
    core::{image as core_image, svg as core_svg},
};
use resvg::{tiny_skia, usvg};
use std::sync::{Arc, Mutex};

/// Width, height and straight (not premultiplied) RGBA bytes.
type Raw = (u32, u32, Vec<u8>);

/// Largest SVG (file or bytes) that is pre-rasterized. The release build aborts
/// on a panic and `resvg::render` runs at load, so a huge or hostile SVG could
/// take the whole menu down: past this size the original handle is kept.
const MAX_SVG_BYTES: usize = 256 * 1024;

/// The handle that draws as `widget::icon(handle.clone()).size(size)` does, or
/// None to keep the original. Makes a new image id on each call: call it at
/// load, never in a view.
pub(super) fn prerender(handle: &icon::Handle, size: u16) -> Option<icon::Handle> {
    if handle.symbolic || size == 0 {
        return None;
    }
    let (width, height, pixels) = match &handle.data {
        icon::Data::Svg(svg) => raster_svg(svg, size)?,
        icon::Data::Image(image) => raster_image(image, size)?,
    };
    Some(icon::from_raster_pixels(width, height, pixels))
}

/// Length the widget gives an icon of `native` px that fills `size` px
/// (the f32 math of `ContentFit::Fill` in the image and svg widgets).
fn fitted(native: u32, size: u16) -> f32 {
    native as f32 * (f32::from(size) / native as f32)
}

/// True when the widget math gives exactly `size` for `native`. Otherwise the
/// stock path truncates to `size - 1` or shifts the icon by a pixel at integer
/// positions, which a copy cannot repeat.
fn fits_exactly(native: u32, size: u16) -> bool {
    native > 0 && fitted(native, size) == f32::from(size)
}

/// Size a tree of `tree` px is scaled to inside a `width` x `height` pixmap
/// (vector.rs of iced_tiny_skia): by width when wider than tall, else by height.
fn svg_target(tree: tiny_skia::IntSize, width: u32, height: u32) -> Option<tiny_skia::IntSize> {
    if width > height { tree.scale_to_width(width) } else { tree.scale_to_height(height) }
}

/// The transform of the stock SVG path: a scale from the origin, no centering.
fn svg_transform(tree: tiny_skia::IntSize, width: u32, height: u32) -> tiny_skia::Transform {
    match svg_target(tree, width, height) {
        Some(target) => {
            let (from, to) = (tree.to_size(), target.to_size());
            tiny_skia::Transform::from_scale(to.width() / from.width(), to.height() / from.height())
        }
        None => tiny_skia::Transform::default(),
    }
}

/// The size the raster path resamples to, or None when it draws the native
/// pixels (or when the float math would not match, see `fits_exactly`).
fn png_target(native: iced::Size<u32>, size: u16) -> Option<iced::Size<u32>> {
    let target = gimage::downsample_target(native, iced::Size::new(f32::from(size), f32::from(size)))?;
    let sides = [native.width, native.height, target.width, target.height];
    sides.into_iter().all(|side| fits_exactly(side, size)).then_some(target)
}

/// The font database of the text system, as the renderer's SVG cache keeps it.
fn fontdb() -> Arc<usvg::fontdb::Database> {
    static CACHE: Mutex<Option<(gtext::Version, Arc<usvg::fontdb::Database>)>> = Mutex::new(None);
    let Ok(system) = gtext::font_system().read() else { return Arc::default() };
    let version = system.version();
    let Ok(mut cache) = CACHE.lock() else { return Arc::new(system.db().clone()) };
    match &*cache {
        Some((cached, db)) if *cached == version => db.clone(),
        _ => {
            let db = Arc::new(system.db().clone());
            *cache = Some((version, db.clone()));
            db
        }
    }
}

fn load_tree(handle: &core_svg::Handle) -> Option<usvg::Tree> {
    let options = usvg::Options { fontdb: fontdb(), ..usvg::Options::default() };
    match handle.data() {
        core_svg::Data::Path(path) => {
            if std::fs::metadata(path).ok()?.len() > MAX_SVG_BYTES as u64 {
                return None;
            }
            let text = std::fs::read_to_string(path).ok()?;
            usvg::Tree::from_str(&text, &options).ok()
        }
        core_svg::Data::Bytes(bytes) if bytes.len() > MAX_SVG_BYTES => None,
        core_svg::Data::Bytes(bytes) => usvg::Tree::from_data(bytes, &options).ok(),
    }
}

/// The SVG as the stock path draws it in a `size` px widget: rendered into a
/// `size` x `size` pixmap (bounds truncated), scaled from the origin.
fn raster_svg(handle: &core_svg::Handle, size: u16) -> Option<Raw> {
    let tree = load_tree(handle)?;
    // The widget measures the tree truncated to whole pixels.
    let measured = [tree.size().width() as u32, tree.size().height() as u32];
    if !measured.into_iter().all(|side| fits_exactly(side, size)) {
        return None;
    }
    let side = u32::from(size);
    let mut pixmap = tiny_skia::Pixmap::new(side, side)?;
    let transform = svg_transform(tree.size().to_int_size(), side, side);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some((side, side, unpremultiply_all(pixmap.data())?))
}

/// The image as the stock path resamples it before drawing: decoded by the
/// renderer's own loader, premultiplied and resized in premultiplied space.
fn raster_image(handle: &core_image::Handle, size: u16) -> Option<Raw> {
    let image = gimage::load(handle).ok()?;
    let native = iced::Size::new(image.width(), image.height());
    let target = png_target(native, size)?;
    let mut premultiplied = Vec::with_capacity(native.width as usize * native.height as usize * 4);
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        let color = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
        premultiplied.extend([color.red(), color.green(), color.blue(), color.alpha()]);
    }
    let resized = gimage::downsample_premultiplied(&premultiplied, native, target);
    Some((target.width, target.height, unpremultiply_all(&resized)?))
}

/// Largest overshoot of a color over its alpha that is still kept.
const MAX_OVERSHOOT: u32 = 8;

/// RGBA bytes with the colors divided by alpha again (rounded). A resampling
/// filter can overshoot, leaving a color above its alpha, which straight RGBA
/// cannot hold: alpha is raised to that color, so the color is kept and the
/// output alpha (and the share of the background that shows through) changes
/// by the overshoot. None when that is more than `MAX_OVERSHOOT`.
fn unpremultiply_all(premultiplied: &[u8]) -> Option<Vec<u8>> {
    let mut straight = Vec::with_capacity(premultiplied.len());
    for pixel in premultiplied.chunks_exact(4) {
        let colors = &pixel[..3];
        let brightest = colors.iter().copied().map(u32::from).max().unwrap_or(0);
        let alpha = u32::from(pixel[3]);
        if brightest > alpha + MAX_OVERSHOOT {
            return None;
        }
        let alpha = alpha.max(brightest);
        for &color in colors {
            let value = if alpha == 0 { 0 } else { (u32::from(color) * 255 + alpha / 2) / alpha };
            straight.push(value.min(255) as u8);
        }
        straight.push(alpha as u8);
    }
    Some(straight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::iced::{
        mouse,
        runtime::user_interface::{Cache, UserInterface},
    };
    use iced_tiny_skia::graphics::Viewport;
    use std::path::{Path, PathBuf};

    const CANVAS: u32 = 144;
    /// Offsets of the icon in the canvas: integer and subpixel, 41 = one icon.
    const SPOTS: [f32; 10] = [0.0, 0.25, 0.5, 0.75, 3.0, 3.5, 41.0, 41.25, 82.0, 82.75];

    /// Outcome of drawing the stock handle and the pre-rasterized one.
    struct Verdict {
        worst: u8,
        bad: usize,
        ink: usize,
    }

    fn new_renderer() -> cosmic::Renderer {
        cosmic::Renderer::Secondary(iced_tiny_skia::Renderer::new(iced::Font::default(), iced::Pixels(14.0)))
    }

    /// `widget::icon(handle).size(size)` drawn by the real tiny-skia renderer at `at`.
    fn paint(renderer: &mut cosmic::Renderer, handle: &icon::Handle, size: u16, at: (f32, f32)) -> Vec<u8> {
        let padding = iced::Padding { top: at.1, right: 0.0, bottom: 0.0, left: at.0 };
        let element: cosmic::Element<'_, ()> = widget::container(widget::icon(handle.clone()).size(size))
            .padding(padding)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        let bounds = iced::Size::new(CANVAS as f32, CANVAS as f32);
        let mut ui = UserInterface::build(element, bounds, Cache::default(), renderer);
        let style = iced::advanced::renderer::Style {
            icon_color: Color::WHITE,
            text_color: Color::WHITE,
            scale_factor: 1.0,
        };
        ui.draw(renderer, &cosmic::Theme::dark(), &style, mouse::Cursor::Unavailable);
        let cosmic::Renderer::Secondary(inner) = renderer else { panic!("tiny-skia renderer expected") };
        let mut pixmap = tiny_skia::Pixmap::new(CANVAS, CANVAS).expect("canvas");
        let mut mask = tiny_skia::Mask::new(CANVAS, CANVAS).expect("mask");
        let viewport = Viewport::with_physical_size(iced::Size::new(CANVAS, CANVAS), 1.0);
        let full = iced::Rectangle::with_size(bounds);
        inner.draw(&mut pixmap.as_mut(), &mut mask, &viewport, &[full], Color::TRANSPARENT);
        pixmap.data().to_vec()
    }

    /// Draws both handles at every spot. None when `prerender` keeps the original.
    fn compare(handle: &icon::Handle, size: u16) -> Option<Verdict> {
        let fast = prerender(handle, size)?;
        let (mut slow_renderer, mut fast_renderer) = (new_renderer(), new_renderer());
        let mut verdict = Verdict { worst: 0, bad: 0, ink: 0 };
        for (i, &x) in SPOTS.iter().enumerate() {
            let at = (x, SPOTS[(i * 3) % SPOTS.len()]);
            let slow = paint(&mut slow_renderer, handle, size, at);
            let quick = paint(&mut fast_renderer, &fast, size, at);
            for (a, b) in slow.chunks_exact(4).zip(quick.chunks_exact(4)) {
                let worst = a.iter().zip(b).map(|(p, q)| p.abs_diff(*q)).max().unwrap_or(0);
                verdict.worst = verdict.worst.max(worst);
                verdict.bad += usize::from(worst > 8);
                verdict.ink += usize::from(a[3] > 0);
            }
        }
        Some(verdict)
    }

    fn assert_equivalent(handle: &icon::Handle, size: u16, what: &str) {
        let verdict = compare(handle, size).unwrap_or_else(|| panic!("{what} @{size}: prerender kept the original"));
        println!("{what} @{size}: max diff {}, px > 8: {}, ink px {}", verdict.worst, verdict.bad, verdict.ink);
        assert!(verdict.ink > 500, "{what} @{size}: the scene is blank");
        assert_eq!(verdict.bad, 0, "{what} @{size}: pixels differ by more than 8");
    }

    fn svg_source(w: u32, h: u32) -> String {
        let (cx, cy, r) = (w as f32 / 2.0, h as f32 / 2.0, w.min(h) as f32 * 0.4);
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#e53935"/>
<stop offset="1" stop-color="#1e88e5" stop-opacity="0.6"/></linearGradient></defs>
<circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#g)"/>
<rect x="3.3" y="2.7" width="9.1" height="7.3" rx="1.7" fill="#43a047" fill-opacity="0.8"/></svg>"##
        )
    }

    fn svg_icon(w: u32, h: u32) -> icon::Handle {
        icon::from_svg_bytes(svg_source(w, h).into_bytes())
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("icon_raster_{}_{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// A soft-edged disc with a diagonal gradient; `alpha_ramp` fades it left to right.
    fn png_icon(dir: &Path, side: u32, alpha_ramp: bool) -> icon::Handle {
        let image = image::RgbaImage::from_fn(side, side, |x, y| {
            let (fx, fy) = (x as f32 / side as f32, y as f32 / side as f32);
            let dist = ((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt();
            let edge = ((0.45 - dist) * side as f32).clamp(0.0, 1.0);
            let ramp = if alpha_ramp { 0.3 + 0.7 * fx } else { 1.0 };
            let byte = |v: f32| (v * 255.0).clamp(0.0, 255.0) as u8;
            image::Rgba([byte(fx), byte(fy), byte(1.0 - fx), byte(edge * ramp)])
        });
        let path = dir.join(format!("icon{side}.png"));
        image.save(&path).expect("write png");
        icon::from_path(path)
    }

    fn rgba_of(handle: &icon::Handle) -> Option<Raw> {
        match &handle.data {
            icon::Data::Image(core_image::Handle::Rgba { width, height, pixels, .. }) => {
                Some((*width, *height, pixels.to_vec()))
            }
            _ => None,
        }
    }

    #[test]
    fn unpremultiply_divides_by_alpha() {
        let straight = unpremultiply_all(&[0, 0, 0, 0, 50, 100, 25, 100, 255, 0, 128, 255]).expect("fits");
        assert_eq!(straight, [0, 0, 0, 0, 128, 255, 64, 100, 255, 0, 128, 255]);
    }

    #[test]
    fn unpremultiply_keeps_a_small_overshoot() {
        // Color 60 over alpha 56: alpha is raised so the color is not clipped.
        let straight = unpremultiply_all(&[60, 30, 0, 56]).expect("small overshoot");
        assert_eq!(straight, [255, 128, 0, 60]);
    }

    #[test]
    fn unpremultiply_rejects_a_large_overshoot() {
        assert!(unpremultiply_all(&[60, 30, 0, 40]).is_none());
        assert!(unpremultiply_all(&[20, 0, 0, 0]).is_none());
    }

    #[test]
    fn svg_target_square() {
        let tree = tiny_skia::IntSize::from_wh(48, 48).expect("size");
        assert_eq!(svg_target(tree, 41, 41).map(|s| (s.width(), s.height())), Some((41, 41)));
    }

    #[test]
    fn svg_target_wide() {
        let tree = tiny_skia::IntSize::from_wh(48, 32).expect("size");
        assert_eq!(svg_target(tree, 60, 40).map(|s| (s.width(), s.height())), Some((60, 40)));
        // Not wider than tall scales by height: 48 * 41 / 32 = 61.5 rounds up.
        assert_eq!(svg_target(tree, 41, 41).map(|s| (s.width(), s.height())), Some((62, 41)));
    }

    #[test]
    fn svg_target_tall() {
        let tree = tiny_skia::IntSize::from_wh(32, 48).expect("size");
        assert_eq!(svg_target(tree, 41, 41).map(|s| (s.width(), s.height())), Some((28, 41)));
    }

    #[test]
    fn svg_target_zero() {
        let tree = tiny_skia::IntSize::from_wh(48, 48).expect("size");
        assert!(svg_target(tree, 0, 0).is_none());
        // A zero side that is not the longer one scales by the other, as the renderer does.
        assert_eq!(svg_target(tree, 41, 0).map(|s| s.width()), Some(41));
    }

    #[test]
    fn png_target_matches_downsample_target() {
        for (native, size) in [(128, 41), (128, 28), (64, 28), (256, 41), (48, 28)] {
            let native = iced::Size::new(native, native);
            let expected = gimage::downsample_target(native, iced::Size::new(f32::from(size), f32::from(size)));
            assert_eq!(png_target(native, size), expected, "{native:?} @{size}");
        }
        assert_eq!(png_target(iced::Size::new(128, 128), 41), Some(iced::Size::new(44, 44)));
        assert_eq!(png_target(iced::Size::new(48, 48), 41), None);
    }

    #[test]
    fn png_without_resample_stays_original() {
        let dir = temp_dir("noresample");
        let handle = png_icon(&dir, 48, true);
        assert!(prerender(&handle, 41).is_none());
        assert!(prerender(&handle, 28).is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn oversized_svg_stays_original() {
        let mut source = svg_source(64, 64);
        source.insert_str(source.len() - "</svg>".len(), &format!("<!--{}-->", " ".repeat(MAX_SVG_BYTES)));
        assert!(source.len() > MAX_SVG_BYTES);
        assert!(prerender(&icon::from_svg_bytes(source.into_bytes()), 41).is_none());
        assert!(prerender(&svg_icon(64, 64), 41).is_some());
    }

    #[test]
    fn symbolic_stays_original() {
        let handle = svg_icon(64, 64).symbolic(true);
        assert!(prerender(&handle, 41).is_none());
        assert!(prerender(&svg_icon(64, 64), 41).is_some());
    }

    #[test]
    fn prerender_twice_gives_the_same_pixels() {
        let handle = svg_icon(64, 64);
        let first = prerender(&handle, 41).and_then(|h| rgba_of(&h)).expect("pixels");
        let second = prerender(&handle, 41).and_then(|h| rgba_of(&h)).expect("pixels");
        assert_eq!((first.0, first.1), (41, 41));
        assert_eq!(first, second);
    }

    #[test]
    fn inexact_fit_stays_original() {
        let odd = (1..=512u32).find(|n| !fits_exactly(*n, 41)).expect("some size is inexact");
        assert!(prerender(&svg_icon(odd, odd), 41).is_none(), "{odd}px tree");
        assert!(fits_exactly(64, 41) && fits_exactly(128, 41) && fits_exactly(32, 28));
        assert!(!fits_exactly(0, 41));
    }

    #[test]
    fn equivalent_svg_square() {
        for size in [41, 28] {
            assert_equivalent(&svg_icon(64, 64), size, "svg 64x64");
        }
    }

    #[test]
    fn equivalent_svg_wide() {
        for size in [41, 28] {
            assert_equivalent(&svg_icon(48, 32), size, "svg 48x32");
        }
    }

    #[test]
    fn equivalent_svg_tall() {
        for size in [41, 28] {
            assert_equivalent(&svg_icon(32, 48), size, "svg 32x48");
        }
    }

    #[test]
    fn equivalent_png_128() {
        let dir = temp_dir("png128");
        for size in [41, 28] {
            assert_equivalent(&png_icon(&dir, 128, false), size, "png 128x128");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn equivalent_png_48_with_alpha() {
        let dir = temp_dir("png48");
        // 48 px needs no resampling for 41, only for 28.
        assert_equivalent(&png_icon(&dir, 48, true), 28, "png 48x48 alpha");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Every installed app, as load_apps finds its icon. Run with
    /// `cargo test icon_raster -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads the installed apps"]
    fn host_icons_are_equivalent() {
        let locales = load::locales();
        let mut seen = std::collections::HashSet::new();
        let (mut kept, mut done, mut bad_total, mut worst_total) = (0, 0, 0, 0u8);
        for path in fde::Iter::new(fde::default_paths()) {
            let Ok(de) = desktop_entry::read(&path, &locales) else { continue };
            if !seen.insert(de.id().to_owned()) || de.hidden() || de.no_display() {
                continue;
            }
            let handle = fde::IconSource::from_unknown(de.icon().unwrap_or(&de.appid)).as_cosmic_icon();
            for size in [41, 28] {
                let Some(v) = compare(&handle, size) else {
                    println!("{:<40} @{size}: kept the original", de.appid);
                    kept += 1;
                    continue;
                };
                println!("{:<40} @{size}: max diff {:>3}, px > 8: {}", de.appid, v.worst, v.bad);
                done += 1;
                bad_total += v.bad;
                worst_total = worst_total.max(v.worst);
            }
        }
        println!("pre-rasterized {done}, kept original {kept}, worst diff {worst_total}, px > 8: {bad_total}");
        assert_eq!(bad_total, 0);
    }
}
