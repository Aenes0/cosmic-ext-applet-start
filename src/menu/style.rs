use super::*;

/// App and window icons are this much larger than their base size.
const ICON_SCALE: f32 = 1.15;

/// A base icon size, scaled.
pub(super) fn px(base: f32) -> u16 {
    (base * ICON_SCALE).round() as u16
}

/// Windows 11's own colors, dark or light as the theme is; the accent is
/// the theme's.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub popup: Color,
    pub border: Color,
    pub hover: Color,
    pub selected: Color,
    pub card: Color,
    pub folder: Color,
    pub text: Color,
    pub dim: Color,
    pub footer: Color,
    pub field: Color,
    pub accent: Color,
}

pub fn palette(theme: &cosmic::Theme) -> Palette {
    let cosmic = theme.cosmic();
    let accent: Color = cosmic.accent_color().into();
    let rgba = |r: u8, g: u8, b: u8, a: f32| Color::from_rgba8(r, g, b, a);
    if cosmic.is_dark {
        Palette {
            bg: rgba(36, 36, 36, 1.0),
            popup: rgba(44, 44, 44, 1.0),
            border: rgba(255, 255, 255, 0.08),
            hover: rgba(255, 255, 255, 0.06),
            selected: rgba(255, 255, 255, 0.09),
            card: rgba(43, 43, 43, 1.0),
            folder: rgba(255, 255, 255, 0.05),
            text: rgba(255, 255, 255, 1.0),
            dim: rgba(255, 255, 255, 0.6),
            footer: rgba(28, 28, 28, 1.0),
            field: rgba(28, 28, 28, 1.0),
            accent,
        }
    } else {
        Palette {
            bg: rgba(243, 243, 243, 1.0),
            popup: rgba(249, 249, 249, 1.0),
            border: rgba(0, 0, 0, 0.09),
            hover: rgba(0, 0, 0, 0.04),
            selected: rgba(0, 0, 0, 0.06),
            card: rgba(250, 250, 250, 1.0),
            folder: rgba(255, 255, 255, 0.75),
            text: rgba(26, 26, 26, 1.0),
            dim: rgba(0, 0, 0, 0.6),
            footer: rgba(236, 236, 236, 1.0),
            field: rgba(255, 255, 255, 1.0),
            accent,
        }
    }
}

fn button_class(style: impl Fn(bool, bool) -> button::Style + Clone + 'static) -> theme::Button {
    let (active, hovered, pressed) = (style.clone(), style.clone(), style.clone());
    theme::Button::Custom {
        active: Box::new(move |_, _| active(false, false)),
        disabled: Box::new(move |_| style(false, false)),
        hovered: Box::new(move |_, _| hovered(true, false)),
        pressed: Box::new(move |_, _| pressed(true, true)),
    }
}

/// The lists' and the grid's: nothing until the mouse is on it.
pub fn flat_class(p: Palette, selected: bool) -> theme::Button {
    button_class(move |hovered, pressed| button::Style {
        background: (hovered || selected).then(|| {
            Background::Color(if pressed { p.card } else if hovered { p.hover } else { p.selected })
        }),
        border_radius: 4.0.into(),
        text_color: Some(p.text),
        icon_color: Some(p.text),
        ..button::Style::new()
    })
}

/// "Show all": a quiet button, a shade on the menu.
pub(super) fn pill_class(p: Palette) -> theme::Button {
    button_class(move |hovered, _| button::Style {
        background: Some(Background::Color(if hovered { p.selected } else { p.folder })),
        border_radius: 4.0.into(),
        border_width: 1.0,
        border_color: p.border,
        text_color: Some(p.text),
        icon_color: Some(p.text),
        ..button::Style::new()
    })
}

/// The categories' cards.
pub(super) fn folder_class(p: Palette) -> theme::Button {
    button_class(move |hovered, _| button::Style {
        background: Some(Background::Color(if hovered { p.selected } else { p.folder })),
        border_radius: 8.0.into(),
        border_width: 1.0,
        border_color: p.border,
        text_color: Some(p.text),
        icon_color: Some(p.text),
        ..button::Style::new()
    })
}

/// Symbols drawn here, not taken from the theme: Breeze's power symbol is a
/// 1 px ring cut square, drawn for 16 px, that blurs at other sizes, and its
/// gear has a "play" triangle inside (a service, in KDE). Ours are thin as
/// Windows', rounded.
const OWN_SYMBOLS: &[(&str, &[u8])] = &[
    ("system-shutdown-symbolic", include_bytes!("../../data/icons/power-symbolic.svg")),
    ("emblem-system-symbolic", include_bytes!("../../data/icons/settings-symbolic.svg")),
];

/// A symbolic icon by name, from the theme, but for the ones drawn here.
pub fn symbolic(name: &str) -> icon::Handle {
    if let Some((_, svg)) = OWN_SYMBOLS.iter().find(|(own, _)| *own == name) {
        return icon::from_svg_bytes(*svg).symbolic(true);
    }
    icon::from_name(name).symbolic(true).handle()
}

pub(super) fn named_icon(names: &[&str]) -> icon::Handle {
    icon::from_name(names[0])
        .size(64)
        .fallback(Some(icon::IconFallback::Names(
            names[1..].iter().map(|n| (*n).to_owned().into()).collect(),
        )))
        .handle()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_memory(handle: &icon::Handle) -> bool {
        matches!(&handle.data, icon::Data::Svg(svg)
            if matches!(svg.data(), cosmic::iced::core::svg::Data::Bytes(_)))
    }

    #[test]
    fn icons_grow_by_fifteen_percent_rounded() {
        assert_eq!(px(36.0), 41);
        assert_eq!(px(32.0), 37);
        assert_eq!(px(20.0), 23);
    }

    #[test]
    fn the_power_symbol_is_ours_not_the_themes() {
        let handle = symbolic("system-shutdown-symbolic");
        assert!(handle.symbolic);
        assert!(from_memory(&handle));
    }

    #[test]
    fn the_settings_gear_is_ours_not_the_themes() {
        let handle = symbolic("emblem-system-symbolic");
        assert!(handle.symbolic);
        assert!(from_memory(&handle));
    }

    #[test]
    fn other_symbols_still_come_from_the_theme() {
        assert!(!from_memory(&symbolic("go-next-symbolic")));
    }
}
