//! Icon-related parts of loading the apps: the pre-rasters kept per app and
//! the signature that tells when the apps (or the icon theme) changed.

use super::*;

/// The application folders' times, and their subfolders': a file added,
/// removed or renamed in one changes it.
pub(super) fn apps_signature() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let stamp = |path: &Path, hasher: &mut std::collections::hash_map::DefaultHasher| {
        if let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) {
            path.hash(hasher);
            modified.hash(hasher);
        }
    };
    for dir in fde::default_paths() {
        stamp(&dir, &mut hasher);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    stamp(&entry.path(), &mut hasher);
                }
            }
        }
    }
    signature_with_theme(hasher.finish(), &cosmic::icon_theme::default())
}

/// The 41 and 28 px rasters of an icon, only for the CPU renderer: the wgpu
/// one keeps the original handle and does no work here.
pub(super) fn rasters_for(icon: &icon::Handle, cpu: bool) -> AppRasters {
    if !cpu {
        return AppRasters::default();
    }
    AppRasters {
        big: icon_raster::prerender(icon, px(36.0)),
        small: icon_raster::prerender(icon, px(24.0)),
    }
}

/// Folds the icon theme into the apps signature: a new theme reloads the handles.
fn signature_with_theme(base: u64, theme: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    base.hash(&mut hasher);
    theme.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use load::tests::entry;

    fn square_svg() -> icon::Handle {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64"><circle cx="32" cy="32" r="26" fill="#e53935"/></svg>"##;
        icon::from_svg_bytes(svg.as_bytes().to_vec())
    }

    /// The image id a handle draws with: two handles share it only if one is a clone.
    fn hid(h: &icon::Handle) -> String {
        match &h.data {
            icon::Data::Svg(svg) => format!("{:?}", svg.id()),
            icon::Data::Image(image) => format!("{:?}", image.id()),
        }
    }

    #[test]
    fn icon_at_picks_the_raster_of_its_size_only() {
        let (big, small) = (square_svg(), square_svg());
        let mut app = entry("a", None);
        app.rasters = AppRasters { big: Some(big.clone()), small: Some(small.clone()) };
        assert_eq!(hid(&app.icon_at(41)), hid(&big));
        assert_eq!(hid(&app.icon_at(28)), hid(&small));
        assert_eq!(hid(&app.icon_at(37)), hid(&app.icon));
        let plain = entry("b", None);
        assert_eq!(hid(&plain.icon_at(41)), hid(&plain.icon));
    }

    #[test]
    fn no_rasters_without_the_cpu_renderer() {
        let r = rasters_for(&square_svg(), false);
        assert!(r.big.is_none() && r.small.is_none());
    }

    #[test]
    fn rasters_for_an_svg_fill_both_sizes_on_cpu() {
        let r = rasters_for(&square_svg(), true);
        assert!(r.big.is_some() && r.small.is_some());
    }

    #[test]
    fn rasters_for_a_symbol_stay_empty() {
        let r = rasters_for(&square_svg().symbolic(true), true);
        assert!(r.big.is_none() && r.small.is_none());
    }

    #[test]
    fn signature_changes_with_the_icon_theme_only() {
        assert_ne!(signature_with_theme(7, "Cosmic"), signature_with_theme(7, "Adwaita"));
        assert_eq!(signature_with_theme(7, "Cosmic"), signature_with_theme(7, "Cosmic"));
    }
}
