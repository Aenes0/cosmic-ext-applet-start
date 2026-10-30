//! Window preview: pure hover state machine plus placement geometry.

use std::time::Duration;

use cosmic::iced::{Point, Rectangle, Size};

use super::open_strip::fit;

pub const OPEN: Duration = Duration::from_millis(400);
pub const CLOSE: Duration = Duration::from_millis(300);
pub const REFRESH: Duration = Duration::from_millis(1000);
pub const CARD_W: f32 = 224.0;
pub const CARD_H: f32 = 164.0;
pub const PAD: f32 = 8.0;
pub const GAP: f32 = 8.0;
pub const MAX_COLS: usize = 7;

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Waiting { key: String },
    Shown { key: String },
    Leaving { key: String },
}

#[derive(Clone, Debug, Default)]
pub struct Hover {
    phase: Phase,
    generation: u64,
    measure: u64,
    anchor: Option<Rectangle>,
    pointer: Option<Point>,
}

impl Hover {
    pub fn phase(&self) -> &Phase {
        &self.phase
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Generation of the measure: changes only with the target (key), never Shown <-> Leaving.
    pub fn measure(&self) -> u64 {
        self.measure
    }
    /// The group changed key without changing windows: keeps phase, generation and anchor,
    /// drops the answers of the old key (the caller sends a `Relayout`).
    pub fn rekey(&mut self, key: String) {
        match &mut self.phase {
            Phase::Idle => return,
            Phase::Waiting { key: k } | Phase::Shown { key: k } | Phase::Leaving { key: k } => *k = key,
        }
        self.measure += 1;
    }
    pub fn key(&self) -> Option<&str> {
        match &self.phase {
            Phase::Idle => None,
            Phase::Waiting { key } | Phase::Shown { key } | Phase::Leaving { key } => Some(key),
        }
    }
    pub fn is_shown(&self) -> bool {
        matches!(self.phase, Phase::Shown { .. } | Phase::Leaving { .. })
    }
    pub fn anchor(&self) -> Option<Rectangle> {
        self.anchor
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    EnterIcon(String),
    LeaveIcon(String),
    ClickGroup(String),
    Pointer(Point),
    PointerGone,
    Anchor(u64, Option<Rectangle>),
    Timer(u64),
    /// Window list or menu size changed: the icon may have moved.
    Relayout,
    /// Menu scrolled while a preview is open.
    Wheel,
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Wait(Duration, u64),
    Measure(String, u64),
    StartCapture(String),
    StopCapture,
}

/// Screen context for one step: `count` is the window count of a group (0 = gone).
pub struct Ctx<F: Fn(&str) -> usize> {
    pub blocked: bool,
    pub bounds: Rectangle,
    pub count: F,
}

impl<F: Fn(&str) -> usize> Ctx<F> {
    fn exists(&self, key: &str) -> bool {
        (self.count)(key) > 0
    }
    fn inside(&self, h: &Hover, key: &str) -> bool {
        inside(h, (self.count)(key), self.bounds)
    }
}

/// Applies one event; returns the effects the caller must run.
pub fn step<F: Fn(&str) -> usize>(h: &mut Hover, ev: Event, ctx: &Ctx<F>) -> Vec<Effect> {
    let mut fx = Vec::new();
    match &ev {
        Event::Pointer(p) => h.pointer = Some(*p),
        Event::PointerGone => h.pointer = None,
        _ => {}
    }
    if ev == Event::Close || (ctx.blocked && h.phase != Phase::Idle) {
        go_idle(h, &mut fx);
        return fx;
    }
    let stale = match &ev {
        Event::Timer(g) => *g != h.generation,
        Event::Anchor(m, _) => *m != h.measure,
        _ => false,
    };
    if stale {
        return fx;
    }
    if h.is_shown() && h.key().is_some_and(|k| !ctx.exists(k)) {
        go_idle(h, &mut fx);
        return fx;
    }
    let Some(key) = h.key().map(str::to_string) else {
        on_idle(h, ev, ctx, &mut fx);
        return fx;
    };
    match ev {
        Event::Relayout => fx.push(Effect::Measure(key, h.measure)),
        Event::Wheel => on_wheel(h, &key, ctx, &mut fx),
        ev => match h.phase {
            Phase::Waiting { .. } => on_waiting(h, &key, ev, ctx, &mut fx),
            Phase::Shown { .. } => on_shown(h, &key, ev, ctx, &mut fx),
            _ => on_leaving(h, &key, ev, ctx, &mut fx),
        },
    }
    fx
}

fn go_idle(h: &mut Hover, fx: &mut Vec<Effect>) {
    if h.phase == Phase::Idle {
        return;
    }
    if h.is_shown() {
        fx.push(Effect::StopCapture);
    }
    set_phase(h, Phase::Idle, true);
}

/// Changes phase (always a new generation); a new target drops the anchor
/// and starts a new measure generation.
fn set_phase(h: &mut Hover, phase: Phase, reset: bool) {
    h.generation += 1;
    h.phase = phase;
    if reset {
        h.anchor = None;
        h.measure += 1;
    }
}

fn start_wait(h: &mut Hover, key: &str, fx: &mut Vec<Effect>) {
    set_phase(h, Phase::Waiting { key: key.to_string() }, true);
    fx.push(Effect::Measure(key.to_string(), h.measure));
    fx.push(Effect::Wait(OPEN, h.generation));
}

/// Shows `key` now (click or icon switch); keeps the geometry if the target is unchanged.
fn show(h: &mut Hover, key: &str, fx: &mut Vec<Effect>) {
    let reset = h.key() != Some(key);
    set_phase(h, Phase::Shown { key: key.to_string() }, reset);
    fx.push(Effect::Measure(key.to_string(), h.measure));
    fx.push(Effect::StartCapture(key.to_string()));
}

fn start_leaving(h: &mut Hover, key: &str, fx: &mut Vec<Effect>) {
    set_phase(h, Phase::Leaving { key: key.to_string() }, false);
    fx.push(Effect::Wait(CLOSE, h.generation));
}

fn on_idle<F: Fn(&str) -> usize>(h: &mut Hover, ev: Event, ctx: &Ctx<F>, fx: &mut Vec<Effect>) {
    if ctx.blocked {
        return;
    }
    match ev {
        Event::EnterIcon(k) => start_wait(h, &k, fx),
        Event::ClickGroup(k) if ctx.exists(&k) => show(h, &k, fx),
        _ => {}
    }
}

/// Wheel: keep the preview only when the pointer is over the card itself.
fn on_wheel<F: Fn(&str) -> usize>(h: &mut Hover, key: &str, ctx: &Ctx<F>, fx: &mut Vec<Effect>) {
    let on_card = match (h.pointer, h.anchor) {
        (Some(p), Some(a)) => {
            h.is_shown() && card_rect(a, (ctx.count)(key), ctx.bounds).contains(p)
        }
        _ => false,
    };
    if !on_card {
        go_idle(h, fx);
    }
}

/// While waiting only the icon counts: the card is not drawn yet.
fn over_icon(h: &Hover) -> bool {
    matches!((h.pointer, h.anchor), (Some(p), Some(a)) if a.contains(p))
}

fn on_waiting<F: Fn(&str) -> usize>(h: &mut Hover, key: &str, ev: Event, ctx: &Ctx<F>, fx: &mut Vec<Effect>) {
    match ev {
        Event::EnterIcon(k) if k != key => start_wait(h, &k, fx),
        Event::LeaveIcon(k) if k == key => go_idle(h, fx),
        Event::PointerGone => go_idle(h, fx),
        Event::Pointer(_) if h.anchor.is_some() && !over_icon(h) => go_idle(h, fx),
        Event::ClickGroup(k) if ctx.exists(&k) => show(h, &k, fx),
        Event::Anchor(_, Some(r)) => h.anchor = Some(r),
        Event::Anchor(_, None) => go_idle(h, fx),
        Event::Timer(_) if h.anchor.is_none() || !ctx.exists(key) => go_idle(h, fx),
        Event::Timer(_) => {
            set_phase(h, Phase::Shown { key: key.to_string() }, false);
            fx.push(Effect::StartCapture(key.to_string()));
        }
        _ => {}
    }
}

fn on_shown<F: Fn(&str) -> usize>(h: &mut Hover, key: &str, ev: Event, ctx: &Ctx<F>, fx: &mut Vec<Effect>) {
    match ev {
        Event::ClickGroup(k) | Event::EnterIcon(k) if k != key && ctx.exists(&k) => show(h, &k, fx),
        Event::Pointer(_) | Event::PointerGone if !ctx.inside(h, key) => start_leaving(h, key, fx),
        Event::LeaveIcon(k) if k == key && !ctx.inside(h, key) => start_leaving(h, key, fx),
        Event::Anchor(_, Some(r)) => h.anchor = Some(r),
        Event::Anchor(_, None) => go_idle(h, fx),
        _ => {}
    }
}

fn on_leaving<F: Fn(&str) -> usize>(h: &mut Hover, key: &str, ev: Event, ctx: &Ctx<F>, fx: &mut Vec<Effect>) {
    let back = || set_back(key);
    match ev {
        Event::ClickGroup(k) if ctx.exists(&k) => show(h, &k, fx),
        Event::EnterIcon(k) if k != key && ctx.exists(&k) => show(h, &k, fx),
        Event::EnterIcon(_) => set_phase(h, back(), false),
        Event::Pointer(_) if ctx.inside(h, key) => set_phase(h, back(), false),
        Event::Timer(_) if ctx.inside(h, key) => set_phase(h, back(), false),
        Event::Timer(_) => go_idle(h, fx),
        Event::Anchor(_, Some(r)) => {
            h.anchor = Some(r);
            if ctx.inside(h, key) {
                set_phase(h, back(), false);
            }
        }
        Event::Anchor(_, None) => go_idle(h, fx),
        _ => {}
    }
}

fn set_back(key: &str) -> Phase {
    Phase::Shown { key: key.to_string() }
}

/// Card rectangle for `n` windows over `anchor`; the view draws with the same function.
pub fn card_rect(anchor: Rectangle, n: usize, bounds: Rectangle) -> Rectangle {
    let size = grid(n, bounds.size()).2;
    Rectangle::new(place(anchor, size, bounds), size)
}

/// True when the pointer is over the icon, the card or the gap between them.
pub fn inside(h: &Hover, n: usize, bounds: Rectangle) -> bool {
    let (Some(p), Some(a)) = (h.pointer, h.anchor) else {
        return false;
    };
    let c = card_rect(a, n, bounds);
    a.contains(p) || c.contains(p) || bridge(a, c).is_some_and(|b| b.contains(p))
}

/// Strip over the gap between icon and card, with the icon's x and width.
pub fn bridge(anchor: Rectangle, card: Rectangle) -> Option<Rectangle> {
    let card_bottom = card.y + card.height;
    let anchor_bottom = anchor.y + anchor.height;
    let (y, height) = if card_bottom <= anchor.y {
        (card_bottom, anchor.y - card_bottom)
    } else if card.y >= anchor_bottom {
        (anchor_bottom, card.y - anchor_bottom)
    } else {
        return None;
    };
    (height > 0.0).then_some(Rectangle { x: anchor.x, y, width: anchor.width, height })
}

/// Top-left of a card of `size`: above the icon if it fits, else below, centred and clamped.
pub fn place(anchor: Rectangle, size: Size, bounds: Rectangle) -> Point {
    let above = anchor.y - GAP - size.height;
    let below = anchor.y + anchor.height + GAP;
    let y = if above >= bounds.y {
        above
    } else if below + size.height <= bounds.y + bounds.height {
        below
    } else {
        (bounds.y + bounds.height - size.height).max(bounds.y)
    };
    let x = if size.width > bounds.width {
        bounds.x
    } else {
        (anchor.center_x() - size.width / 2.0)
            .clamp(bounds.x, bounds.x + bounds.width - size.width)
    };
    Point::new(x, y)
}

/// Columns, rows and size of the card grid for `n` cards inside `bounds`.
pub fn grid(n: usize, bounds: Size) -> (usize, usize, Size) {
    let cols = n
        .min(MAX_COLS)
        .min(fit(bounds.width - 2.0 * PAD, CARD_W, GAP))
        .max(1);
    let rows = n.div_ceil(cols);
    let width = 2.0 * PAD + cols as f32 * CARD_W + (cols - 1) as f32 * GAP;
    let height = 2.0 * PAD + rows as f32 * CARD_H + rows.saturating_sub(1) as f32 * GAP;
    (cols, rows, Size::new(width, height.min(bounds.height)))
}

#[cfg(test)]
mod tests;
