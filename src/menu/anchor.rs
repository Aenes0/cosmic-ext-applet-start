//! Where an item is on the window, and where its context menu goes.

use cosmic::iced::advanced::widget::{self, Operation, operation::Outcome};
use cosmic::iced::runtime::task;
use cosmic::iced::{Point, Rectangle, Size, Task, Vector};

/// Vertical gap between the anchor and the popup, in pixels.
const GAP: f32 = 4.0;
/// Height of one popup entry, in pixels.
const ITEM_HEIGHT: f32 = 36.0;
/// Height of the popup title row, in pixels.
const TITLE_HEIGHT: f32 = 40.0;
/// Total vertical padding of the popup, in pixels.
const PADDING: f32 = 8.0;
/// Spacing between the rows of the popup column (`spacing(2)` in `menu.rs`), in pixels.
const ITEM_SPACING: f32 = 2.0;

/// Estimated popup height: `items * 36`, plus 40 for a title, plus 8 of padding,
/// plus 2 between each pair of rows (the title counts as a row).
pub fn popup_height(items: usize, has_title: bool) -> f32 {
    let title = if has_title { TITLE_HEIGHT } else { 0.0 };
    let rows = items + usize::from(has_title);
    let gaps = rows.saturating_sub(1) as f32 * ITEM_SPACING;
    items as f32 * ITEM_HEIGHT + title + PADDING + gaps
}

/// Top-left corner of the popup: below the anchor's bottom-left corner (4 px gap),
/// flipped above if it does not fit, and always clamped inside `menu`.
pub fn popup_origin(anchor: Rectangle, popup: Size, menu: Rectangle) -> Point {
    let max_x = menu.x + menu.width - popup.width;
    let x = anchor.x.min(max_x).max(menu.x);

    let below = anchor.y + anchor.height + GAP;
    let menu_bottom = menu.y + menu.height;
    let y = if below + popup.height <= menu_bottom {
        below
    } else {
        let above = anchor.y - GAP - popup.height;
        if above >= menu.y {
            above
        } else {
            (menu_bottom - popup.height).max(menu.y)
        }
    };
    Point::new(x, y)
}

/// Visible bounds of each container in `ids`, in one pass; `None` where not found or fully clipped.
///
/// Mirrors iced's `container::visible_bounds`, whose `traverse` recurses forever.
pub fn find_many(ids: Vec<widget::Id>) -> Task<Vec<Option<Rectangle>>> {
    task::widget(VisibleBounds::new(ids))
}

/// Visible bounds (window coordinates, discounting scrolling) of the container
/// with `id`, or `None` if it is not found or fully clipped.
pub fn find(id: widget::Id) -> Task<Option<Rectangle>> {
    find_many(vec![id]).map(|found| found.into_iter().next().flatten())
}

struct VisibleBounds {
    targets: Vec<widget::Id>,
    depth: usize,
    scrollables: Vec<(Vector, Rectangle, usize)>,
    found: Vec<bool>,
    bounds: Vec<Option<Rectangle>>,
}

impl VisibleBounds {
    fn new(targets: Vec<widget::Id>) -> Self {
        let n = targets.len();
        Self {
            targets,
            depth: 0,
            scrollables: Vec::new(),
            found: vec![false; n],
            bounds: vec![None; n],
        }
    }

    /// Bounds in window coordinates, discounting the innermost scrollable.
    fn visible(&self, bounds: Rectangle) -> Option<Rectangle> {
        match self.scrollables.last() {
            Some((translation, viewport, _)) => viewport.intersection(&(bounds - *translation)),
            None => Some(bounds),
        }
    }
}

impl Operation<Vec<Option<Rectangle>>> for VisibleBounds {
    fn scrollable(
        &mut self,
        _id: Option<&widget::Id>,
        bounds: Rectangle,
        _content_bounds: Rectangle,
        translation: Vector,
        _state: &mut dyn widget::operation::Scrollable,
    ) {
        match self.scrollables.last() {
            Some((last_translation, last_viewport, _)) => {
                let viewport = last_viewport
                    .intersection(&(bounds - *last_translation))
                    .unwrap_or(Rectangle::new(Point::ORIGIN, Size::ZERO));
                self.scrollables
                    .push((translation + *last_translation, viewport, self.depth));
            }
            None => self.scrollables.push((translation, bounds, self.depth)),
        }
    }

    fn container(&mut self, id: Option<&widget::Id>, bounds: Rectangle) {
        if let Some(id) = id {
            let visible = self.visible(bounds);
            for (i, target) in self.targets.iter().enumerate() {
                if target == id && !self.found[i] {
                    self.found[i] = true;
                    self.bounds[i] = visible;
                }
            }
        }
    }

    fn traverse(
        &mut self,
        operate: &mut dyn FnMut(&mut dyn Operation<Vec<Option<Rectangle>>>),
    ) {
        self.depth += 1;
        operate(self);
        self.depth -= 1;
        // A scrollable registered at depth D goes out of scope with its children.
        let depth = self.depth;
        self.scrollables.retain(|(_, _, d)| *d < depth);
    }

    fn finish(&self) -> Outcome<Vec<Option<Rectangle>>> {
        Outcome::Some(self.bounds.clone())
    }
}

#[cfg(test)]
mod tests;
