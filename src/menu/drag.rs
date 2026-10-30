//! Pure state of dragging an app to pin it, reorder the pinned or move it among the groups.

use std::time::{Duration, Instant};

use cosmic::iced::{Point, Rectangle};

use super::groups::GroupRef;

/// Distance (px) the pointer must travel before a press becomes a drag.
pub const THRESHOLD: f32 = 6.0;
/// Pause on a tile center before "into the group" holds.
pub const DWELL: Duration = Duration::from_millis(450);
/// Half the gap between tiles: the halves cover the gap.
pub const EDGE: f32 = 6.0;
/// Leaving the popup only counts beyond this margin.
pub const POPUP_MARGIN: f32 = 28.0;
/// Gap between the pinned area and the "Unpin" bar.
pub const UNPIN_GAP: f32 = 4.0;
/// Height of the "Unpin" bar.
pub const UNPIN_HEIGHT: f32 = 30.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Index in `self.pinned`.
    Pinned(usize),
    /// App id (tile, card, row, window).
    App(String),
    /// An app id taken out of a group.
    Member(GroupRef, String),
    /// A whole group.
    Group(GroupRef),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Slot {
    /// Before the pinned item at this index.
    Before(usize),
    End,
    /// Into a group.
    Into(GroupRef),
    /// Before the member at this index of a group.
    Member(GroupRef, usize),
    /// After the last member of a group.
    MemberEnd(GroupRef),
    /// The body of the menu, out of any group.
    Body,
    /// The center of an app tile (index in `self.apps`).
    OnApp(usize),
    /// The "Unpin" bar below the pinned area.
    Unpin,
}

/// Which target wins where they overlap: the smaller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    PopupCenter,
    PopupTile,
    PopupArea,
    Unpin,
    Center,
    Tile,
    Area,
    Body,
}

/// Where a drop may land, and with which priority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub slot: Slot,
    pub rect: Rectangle,
    pub layer: Layer,
}

/// What is placed in a group.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    App(String),
    Pinned(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Reorder { from: usize, to: Slot },
    Pin { app: String, to: Slot },
    Place { item: Item, into: GroupRef, before: Option<usize> },
    Ungroup { app: String },
    /// Create a user group from `item` and the app at `onto` (index in `self.apps`).
    Merge { item: Item, onto: usize },
    /// Remove the pinned entry at `index`.
    Unpin { index: usize },
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Drag {
    #[default]
    Idle,
    Armed {
        source: Source,
        from: Point,
    },
    Dragging {
        source: Source,
        at: Point,
        over: Option<Slot>,
        hover: Option<(Slot, Instant)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Zone {
    Whole,
    Center,
    Left,
    Right,
    /// A strip just under the rectangle (the "Unpin" bar).
    Below,
}

/// A target before its rectangle is known.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    pub slot: Slot,
    pub layer: Layer,
    pub zone: Zone,
}

/// What a drag source may land on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wants {
    pub centers: bool,
    pub folders: bool,
    pub popup: bool,
    /// Only pinned items (app or group) can be unpinned.
    pub unpin: bool,
}

/// The middle 50% of a rectangle.
pub fn center(rect: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(rect.x + rect.width * 0.25, rect.y + rect.height * 0.25),
        cosmic::iced::Size::new(rect.width * 0.5, rect.height * 0.5),
    )
}

/// The part of `rect` a zone covers.
pub fn zone(rect: Rectangle, zone: Zone) -> Rectangle {
    let half = rect.width / 2.0;
    let (y, h) = (rect.y - EDGE, rect.height + 2.0 * EDGE);
    match zone {
        Zone::Whole => rect,
        Zone::Center => center(rect),
        Zone::Left => Rectangle::new(Point::new(rect.x - EDGE, y), cosmic::iced::Size::new(half + EDGE, h)),
        Zone::Right => Rectangle::new(Point::new(rect.x + half, y), cosmic::iced::Size::new(half + EDGE, h)),
        Zone::Below => Rectangle::new(
            Point::new(rect.x, rect.y + rect.height + UNPIN_GAP),
            cosmic::iced::Size::new(rect.width, UNPIN_HEIGHT),
        ),
    }
}

/// Specs of a tile: left half = before, right half = after, optional center.
pub fn tile_specs(before: Slot, after: Slot, layer: Layer, center: Option<Slot>) -> Vec<Spec> {
    let mut specs = vec![
        Spec { slot: before, layer, zone: Zone::Left },
        Spec { slot: after, layer, zone: Zone::Right },
    ];
    if let Some(slot) = center {
        // Limitation: the center is always `Layer::Center`; popup tiles would need `PopupCenter`.
        specs.push(Spec { slot, layer: Layer::Center, zone: Zone::Center });
    }
    specs
}

/// Pairs specs with measured rectangles, drops unknown ones, sorts by layer (stable).
pub fn targets(specs: &[Vec<Spec>], rects: &[Option<Rectangle>]) -> Vec<Target> {
    let mut out: Vec<Target> = specs
        .iter()
        .zip(rects)
        .filter_map(|(list, rect)| rect.map(|r| (list, r)))
        .flat_map(|(list, r)| {
            list.iter().map(move |s| Target { slot: s.slot, rect: zone(r, s.zone), layer: s.layer })
        })
        .collect();
    out.sort_by_key(|t| t.layer);
    out
}

/// True when a popup exists and the point is beyond its margin.
pub fn left_popup(point: Point, targets: &[Target]) -> bool {
    let mut areas = targets.iter().filter(|t| t.layer == Layer::PopupArea).peekable();
    if areas.peek().is_none() {
        return false;
    }
    !areas.any(|t| {
        let r = t.rect;
        let grown = Rectangle::new(
            Point::new(r.x - POPUP_MARGIN, r.y - POPUP_MARGIN),
            cosmic::iced::Size::new(r.width + 2.0 * POPUP_MARGIN, r.height + 2.0 * POPUP_MARGIN),
        );
        grown.contains(point)
    })
}

/// True for the layers that belong to an open popup.
fn is_popup(layer: Layer) -> bool {
    matches!(layer, Layer::PopupCenter | Layer::PopupTile | Layer::PopupArea)
}

/// Targets without the popup layers.
pub fn without_popup(targets: &[Target]) -> Vec<Target> {
    targets
        .iter()
        .filter(|t| !is_popup(t.layer))
        .copied()
        .collect()
}

/// Which target kinds a source may use.
pub fn wants(source: &Source, pinned_is_group: bool) -> Wants {
    let (centers, popup) = match source {
        Source::Group(_) => (false, false),
        Source::Pinned(_) if pinned_is_group => (false, false),
        Source::Member(..) => (true, true),
        Source::Pinned(_) | Source::App(_) => (true, false),
    };
    Wants { centers, folders: centers, popup, unpin: matches!(source, Source::Pinned(_)) }
}

impl Drag {
    pub fn arm(source: Source, point: Point) -> Drag {
        Drag::Armed {
            source,
            from: point,
        }
    }

    pub fn moved(self, point: Point, targets: &[Target], now: Instant) -> Drag {
        match self {
            Drag::Armed { source, from } => {
                let dx = point.x - from.x;
                let dy = point.y - from.y;
                if (dx * dx + dy * dy).sqrt() > THRESHOLD {
                    Drag::dragging(source, point, targets, None, now)
                } else {
                    Drag::Armed { source, from }
                }
            }
            Drag::Dragging { source, hover, .. } => Drag::dragging(source, point, targets, hover, now),
            Drag::Idle => Drag::Idle,
        }
    }

    /// Builds the dragging state, applying the dwell rule on center targets.
    fn dragging(
        source: Source,
        point: Point,
        targets: &[Target],
        prev: Option<(Slot, Instant)>,
        now: Instant,
    ) -> Drag {
        let popup_open = targets.iter().any(|t| t.layer == Layer::PopupArea);
        let usable = |t: &&Target| {
            !popup_open || is_popup(t.layer)
        };
        let raw = targets.iter().filter(usable).find(|t| t.rect.contains(point));
        let (over, hover) = match raw {
            Some(c) if matches!(c.layer, Layer::Center | Layer::PopupCenter) => {
                let since = match prev {
                    Some((s, at)) if s == c.slot => at,
                    _ => now,
                };
                let over = if now.saturating_duration_since(since) >= DWELL {
                    Some(c.slot)
                } else {
                    // Body is excluded: a member dropped early on a tile center must
                    // cancel, not ungroup (the intent was to merge).
                    targets
                        .iter()
                        .filter(usable)
                        .find(|t| {
                            !matches!(t.layer, Layer::Center | Layer::PopupCenter | Layer::Body)
                                && t.rect.contains(point)
                        })
                        .map(|t| t.slot)
                };
                (over, Some((c.slot, since)))
            }
            other => (other.map(|t| t.slot), None),
        };
        Drag::Dragging { source, at: point, over, hover }
    }

    /// True while a center pause is running and still needs a tick to win.
    pub fn dwelling(&self) -> bool {
        matches!(self, Drag::Dragging { over, hover: Some((s, _)), .. } if *over != Some(*s))
    }

    pub fn released(self) -> Action {
        let Drag::Dragging { source, over: Some(to), .. } = self else {
            return Action::Cancel;
        };
        match (source, to) {
            (Source::Pinned(from), Slot::Before(_) | Slot::End) => Action::Reorder { from, to },
            (Source::Pinned(i), Slot::OnApp(onto)) => Action::Merge { item: Item::Pinned(i), onto },
            (Source::Pinned(i), Slot::Into(g)) => {
                Action::Place { item: Item::Pinned(i), into: g, before: None }
            }
            (Source::Pinned(index), Slot::Unpin) => Action::Unpin { index },
            (Source::Member(_, app), Slot::Body) => Action::Ungroup { app },
            (Source::App(app) | Source::Member(_, app), Slot::OnApp(onto)) => {
                Action::Merge { item: Item::App(app), onto }
            }
            (Source::App(app) | Source::Member(_, app), to) => released_app(app, to),
            (Source::Group(g), Slot::Before(_) | Slot::End) => match g.pin() {
                Some(app) => Action::Pin { app, to },
                None => Action::Cancel,
            },
            _ => Action::Cancel,
        }
    }

    pub fn cancel(self) -> Drag {
        Drag::Idle
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Drag::Dragging { .. })
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, Drag::Idle)
    }
}

/// Drop of an app id (from anywhere) over `to`.
fn released_app(app: String, to: Slot) -> Action {
    let place = |into, before| Action::Place { item: Item::App(app.clone()), into, before };
    match to {
        Slot::Before(_) | Slot::End => Action::Pin { app, to },
        Slot::Into(g) | Slot::MemberEnd(g) => place(g, None),
        Slot::Member(g, i) => place(g, Some(i)),
        Slot::Body | Slot::OnApp(_) | Slot::Unpin => Action::Cancel,
    }
}

fn reorder(pinned: &[String], from: usize, to: Slot) -> Vec<String> {
    if from >= pinned.len() {
        return pinned.to_vec();
    }
    let mut list = pinned.to_vec();
    let item = list.remove(from);
    let idx = match to {
        Slot::End => list.len(),
        Slot::Before(i) if i > from => (i - 1).min(list.len()),
        Slot::Before(i) => i.min(list.len()),
        _ => return pinned.to_vec(),
    };
    list.insert(idx, item);
    list
}

pub fn apply(
    pinned: &[String],
    action: &Action,
    same_app: &dyn Fn(&str, &str) -> bool,
) -> Vec<String> {
    match action {
        Action::Cancel | Action::Place { .. } | Action::Ungroup { .. } | Action::Merge { .. } | Action::Unpin { .. } => {
            pinned.to_vec()
        }
        Action::Reorder { from, to } => reorder(pinned, *from, *to),
        Action::Pin { app, to } => match pinned.iter().position(|p| same_app(p, app)) {
            Some(from) => reorder(pinned, from, *to),
            None => {
                let mut list = pinned.to_vec();
                let idx = match to {
                    Slot::End => list.len(),
                    Slot::Before(i) => (*i).min(list.len()),
                    _ => return list,
                };
                list.insert(idx, app.clone());
                list
            }
        },
    }
}

#[cfg(test)]
mod tests;
