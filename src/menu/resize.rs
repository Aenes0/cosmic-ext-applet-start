
//! Pure geometry and state machine for resizing the menu by its edges.

use cosmic::iced::mouse::Interaction;
use cosmic::iced::{Point, Rectangle, Size};

use super::{MARGIN, MIN_HEIGHT};

pub const MIN_WIDTH: f32 = 400.0;
pub const MAX_WIDTH: f32 = 874.0;
pub const MAX_HEIGHT: f32 = 920.0;
pub const MIN_RESIZE_HEIGHT: f32 = 480.0;

const OUT: f32 = 6.0;
const THICK: f32 = 8.0;
const CORNER: f32 = 16.0;

/// `Far` is the edge away from the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Far,
    FarLeft,
    FarRight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resize {
    Idle,
    Armed { edge: Edge, from: Size },
    Active { edge: Edge, from: Size, start: Point },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub min: Size,
    pub max: Size,
}

fn usable(v: f32) -> bool {
    v.is_finite() && v > 0.0
}

pub fn limits(room_width: f32, room: f32) -> Limits {
    let max_w = if usable(room_width) {
        MAX_WIDTH.min(room_width - 2.0 * MARGIN)
    } else {
        MAX_WIDTH
    }
    .max(1.0);
    let max_h = if usable(room) {
        MAX_HEIGHT.min(room - 2.0 * MARGIN).max(MIN_HEIGHT)
    } else {
        MAX_HEIGHT
    };
    Limits {
        min: Size::new(MIN_WIDTH.min(max_w), MIN_RESIZE_HEIGHT.min(max_h)),
        max: Size::new(max_w, max_h),
    }
}

fn fit(v: f32, min: f32, max: f32) -> f32 {
    if v.is_finite() {
        v.clamp(min, max).round()
    } else {
        max
    }
}

/// Width: nearest even value (half rounds up), never above the largest even
/// value <= max nor below the smallest even value >= min, so the centred menu
/// has no half pixel.
fn fit_even(v: f32, min: f32, max: f32) -> f32 {
    let (lo, hi) = ((min / 2.0).ceil() * 2.0, (max / 2.0).floor() * 2.0);
    if !v.is_finite() {
        return hi.max(lo);
    }
    let even = (v.clamp(min, max) / 2.0).round() * 2.0;
    if hi < lo { max.round() } else { even.clamp(lo, hi) }
}

fn fit_size(s: Size, l: Limits) -> Size {
    Size::new(
        fit_even(s.width, l.min.width, l.max.width),
        fit(s.height, l.min.height, l.max.height),
    )
}

pub fn effective(pref: Option<Size>, l: Limits) -> Size {
    fit_size(pref.unwrap_or(l.max), l)
}

/// The preference after a drag: the dragged axes take the dragged value, the
/// other keeps the earlier preference (or the default maximum).
pub fn requested(edge: Edge, pref: Option<Size>, dragged: Size) -> Size {
    let base = pref.unwrap_or(Size::new(MAX_WIDTH, MAX_HEIGHT));
    let width = if edge == Edge::Far { base.width } else { dragged.width };
    let height = if matches!(edge, Edge::Left | Edge::Right) { base.height } else { dragged.height };
    Size::new(width, height)
}

pub fn should_save(before: Option<Size>, after: Option<Size>, persist: bool) -> bool {
    persist && before != after
}

/// What goes to the config; ConfigSet cannot delete, so no preference stores
/// the default maximum.
pub fn stored_pair(pref: Option<Size>) -> (u32, u32) {
    let s = pref.unwrap_or(Size::new(MAX_WIDTH, MAX_HEIGHT));
    (s.width as u32, s.height as u32)
}

pub fn clamp_stored(w: u32, h: u32) -> Size {
    Size::new(
        (w as f32).clamp(MIN_WIDTH, MAX_WIDTH),
        (h as f32).clamp(MIN_RESIZE_HEIGHT, MAX_HEIGHT),
    )
}

/// Parses "WxH" (case-insensitive x, surrounding spaces allowed).
pub fn parse_size(s: &str) -> Option<Size> {
    let (w, h) = s.trim().split_once(['x', 'X'])?;
    Some(clamp_stored(w.trim().parse().ok()?, h.trim().parse().ok()?))
}

/// Requested size and whether it should be persisted (an env override is not).
pub fn initial_pref(env: Option<&str>, stored: Option<(u32, u32)>) -> (Option<Size>, bool) {
    if let Some(size) = env.and_then(parse_size) {
        return (Some(size), false);
    }
    (stored.map(|(w, h)| clamp_stored(w, h)), true)
}

pub fn resized(edge: Edge, from: Size, start: Point, now: Point, on_top: bool, l: Limits) -> Size {
    let (dx, dy) = (now.x - start.x, now.y - start.y);
    let mut s = from;
    match edge {
        Edge::Left | Edge::FarLeft => s.width = from.width - 2.0 * dx,
        Edge::Right | Edge::FarRight => s.width = from.width + 2.0 * dx,
        Edge::Far => {}
    }
    if !matches!(edge, Edge::Left | Edge::Right) {
        s.height = if on_top { from.height + dy } else { from.height - dy };
    }
    fit_size(s, l)
}

pub fn handles(menu: Rectangle, on_top: bool) -> [(Edge, Rectangle); 5] {
    let m = menu;
    let (far_y, corner_y, side_y) = if on_top {
        (m.y + m.height - (THICK - OUT), m.y + m.height - (CORNER - OUT), m.y)
    } else {
        (m.y - OUT, m.y - OUT, m.y + (CORNER - OUT))
    };
    let side_h = m.height - (CORNER - OUT);
    let r = |x, y, w, h| Rectangle::new(Point::new(x, y), Size::new(w, h));
    [
        (Edge::Left, r(m.x - OUT, side_y, THICK, side_h)),
        (Edge::Right, r(m.x + m.width - (THICK - OUT), side_y, THICK, side_h)),
        (Edge::Far, r(m.x + (CORNER - OUT), far_y, m.width - 2.0 * (CORNER - OUT), THICK)),
        (Edge::FarLeft, r(m.x - OUT, corner_y, CORNER, CORNER)),
        (Edge::FarRight, r(m.x + m.width - (CORNER - OUT), corner_y, CORNER, CORNER)),
    ]
}

pub fn interaction(edge: Edge, on_top: bool) -> Interaction {
    match (edge, on_top) {
        (Edge::Left | Edge::Right, _) => Interaction::ResizingHorizontally,
        (Edge::Far, _) => Interaction::ResizingVertically,
        (Edge::FarLeft, false) | (Edge::FarRight, true) => Interaction::ResizingDiagonallyDown,
        (Edge::FarRight, false) | (Edge::FarLeft, true) => Interaction::ResizingDiagonallyUp,
    }
}

impl Resize {
    pub fn arm(edge: Edge, from: Size) -> Self {
        Resize::Armed { edge, from }
    }

    pub fn moved(self, point: Point, on_top: bool, l: Limits) -> (Self, Option<Size>) {
        match self {
            Resize::Idle => (self, None),
            Resize::Armed { edge, from } => (Resize::Active { edge, from, start: point }, None),
            Resize::Active { edge, from, start } => {
                (self, Some(resized(edge, from, start, point, on_top, l)))
            }
        }
    }

    /// Ends the gesture; the flag says whether one was running.
    pub fn finish(self) -> (Self, bool) {
        (Resize::Idle, !self.is_idle())
    }

    pub fn edge(&self) -> Option<Edge> {
        match self {
            Resize::Idle => None,
            Resize::Armed { edge, .. } | Resize::Active { edge, .. } => Some(*edge),
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, Resize::Idle)
    }

    /// Size at the start of the gesture; `None` when idle (only tests read it).
    #[cfg(test)]
    pub fn from(&self) -> Option<Size> {
        match self {
            Resize::Idle => None,
            Resize::Armed { from, .. } | Resize::Active { from, .. } => Some(*from),
        }
    }
}

#[cfg(test)]
mod tests;
