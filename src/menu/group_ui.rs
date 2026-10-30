//! The group's popup, folders and pinned tiles.

use std::time::Duration;

use cosmic::{
    Element, theme,
    iced::{self, Alignment, Background, Border, Color, Length, widget::text::Wrapping},
    widget::{self, button, icon},
};

use super::drag::{Slot, Source};
use super::groups::GroupRef;
use super::metrics::Metrics;
use super::{
    AppEntry, Message, Palette, Target, draggable, ellipsize, flat_class, folder_class, px, scoped,
    tile_overlay,
};

pub const POPUP_RADIUS: f32 = 16.0;
/// Id of the name field in the popup's title.
pub const GROUP_NAME: &str = "group-name";
/// Id of the popup's card (hit-test of the drag).
pub const POPUP_CARD: &str = "group-popup";
pub const PIN_BOX: f32 = 56.0;
pub const PIN_RADIUS: f32 = 12.0;
pub const FOLDER_BOX: f32 = 120.0;
/// Height of the "Unpin" bar.
pub const UNPIN_BAR_HEIGHT: f32 = 30.0;
const FOLDER_CARD_HEIGHT: f32 = 144.0;

/// (columns, shown) of a mini folder with `n` apps: 2x2 up to 4, 3x3 from 5.
pub fn mini_grid(n: usize) -> (usize, usize) {
    if n <= 4 { (2, n) } else { (3, n.min(9)) }
}

/// Geometry of a mini folder: columns, icon side, gap and padding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MiniLayout {
    pub cols: usize,
    pub icon: u16,
    pub gap: f32,
    pub pad: f32,
}

/// Largest icon in a folder: the interface's default icon size.
pub fn icon_max() -> u16 {
    px(36.0)
}

/// (gap, pad) of a mini folder in a square of `side` px.
fn gap_pad(side: f32) -> (f32, f32) {
    (if side < 80.0 { 2.0 } else { 3.0 }, (side * 0.06).clamp(3.0, 5.0))
}

/// Layout of `count` mini icons in a square of `side` px, icons capped at `icon_max`.
pub fn mini_layout(side: f32, count: usize) -> MiniLayout {
    let cols = mini_grid(count).0;
    let (gap, pad) = gap_pad(side);
    let c = cols as f32;
    let icon = ((side - 2.0 * pad - (c - 1.0) * gap) / c).floor().max(0.0) as u16;
    MiniLayout { cols, icon: icon.min(icon_max()), gap, pad }
}

/// `side`, or the smaller square where a 2x2 folder gets exactly `icon_max` icons.
pub fn capped_side(side: f32) -> f32 {
    let mut s = side;
    for _ in 0..3 {
        let (gap, pad) = gap_pad(s);
        s = s.min(2.0 * f32::from(icon_max()) + gap + 2.0 * pad);
    }
    s
}

/// Rows needed for `n` cells in `cols` columns (at least one).
pub fn grid_rows(n: usize, cols: usize) -> usize {
    n.div_ceil(cols.max(1)).max(1)
}

/// Empty cells that complete the last row (a whole row when `n` is 0).
pub fn pad_cells(n: usize, cols: usize) -> usize {
    if n == 0 { cols } else { (cols - n % cols) % cols }
}

pub fn member_key(g: GroupRef, i: usize) -> String {
    format!("grp:{}:{i}", g.key())
}

pub fn member_id(g: GroupRef, i: usize) -> widget::Id {
    widget::Id::new(member_key(g, i))
}

pub fn folder_key(g: GroupRef) -> String {
    format!("fold:{}", g.key())
}

pub fn folder_id(g: GroupRef) -> widget::Id {
    widget::Id::new(folder_key(g))
}

pub fn popup_card_id() -> widget::Id {
    widget::Id::new(POPUP_CARD)
}

/// The first icons of a group in a `size` box: 2x2 up to 4, 3x3 from 5 (the first 9). No background.
pub fn mini_folder<'a>(icons: &[icon::Handle], size: f32) -> Element<'a, Message> {
    let MiniLayout { cols, icon: side, gap, pad } = mini_layout(size, icons.len());
    let shown = mini_grid(icons.len()).1;
    let cell = |handle: Option<&icon::Handle>| -> Element<'a, Message> {
        match handle {
            Some(h) => widget::icon(h.clone()).size(side).into(),
            None => widget::space::horizontal().width(Length::Fixed(f32::from(side))).into(),
        }
    };
    let rows = shown.div_ceil(cols).max(1);
    let mut grid = widget::column::with_capacity(rows).spacing(gap);
    for r in 0..rows {
        let mut row = widget::row::with_capacity(cols).spacing(gap);
        for c in 0..cols {
            let i = r * cols + c;
            row = row.push(cell((i < shown).then(|| icons.get(i)).flatten()));
        }
        grid = grid.push(row);
    }
    widget::container(grid)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .padding(pad)
        .into()
}

/// Accent ring over a drop target; always there, transparent when off.
pub fn drop_ring<'a>(on: bool, radius: f32, p: Palette) -> Element<'a, Message> {
    widget::container(widget::space::horizontal())
        .width(Length::Fill)
        .height(Length::Fill)
        .class(theme::Container::custom(move |_| widget::container::Style {
            border: Border {
                color: if on { p.accent } else { Color::TRANSPARENT },
                width: 2.0,
                radius: radius.into(),
            },
            ..Default::default()
        }))
        .into()
}

/// A box with the folder color and a radius.
fn folder_box<'a>(content: Element<'a, Message>, radius: f32, p: Palette) -> Element<'a, Message> {
    widget::container(content)
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(Background::Color(p.folder)),
            border: Border { radius: radius.into(), ..Default::default() },
            ..Default::default()
        }))
        .into()
}

/// The dragged folder under the cursor: `mini_folder` in a PIN_BOX with the folder color.
pub fn ghost_folder<'a>(icons: &[icon::Handle], p: Palette) -> Element<'a, Message> {
    folder_box(mini_folder(icons, PIN_BOX), PIN_RADIUS, p)
}

/// A member in the popup, draggable out; its overlay is always there.
pub fn member_tile<'a>(
    app: &'a AppEntry,
    g: GroupRef,
    i: usize,
    bar: bool,
    dim: bool,
    ring: bool,
    active: bool,
    p: Palette,
) -> Element<'a, Message> {
    let id = member_id(g, i);
    let content = widget::column::with_capacity(2)
        .push(widget::icon(app.icon_at(icon_max())).size(icon_max()))
        .push(
            widget::text::caption(ellipsize(&app.name, 11))
                .wrapping(Wrapping::None)
                .align_x(Alignment::Center),
        )
        .align_x(Alignment::Center)
        .spacing(4)
        .width(Length::Fill);
    let tile = widget::mouse_area(draggable(
        button::custom(content).padding([6, 2, 4, 2]).width(Length::Fill).class(flat_class(p, false)),
        Some(Source::Member(g, app.id.clone())),
        Message::Launch(app.id.clone()),
        active,
    ))
    .on_right_press(Message::MenuFor(Target::App(app.id.clone()), id.clone()));
    let stack = iced::widget::stack(vec![tile.into(), tile_overlay(dim, bar, p), drop_ring(ring, 8.0, p)]);
    widget::container(stack).width(Length::Fill).id(id).into()
}

/// The id of the group whose title is an editable field (user groups only).
pub fn editable(g: GroupRef) -> Option<u32> {
    match g {
        GroupRef::User(id) => Some(id),
        _ => None,
    }
}

/// The popup's title: a borderless name field for user groups, a heading otherwise.
fn title_view<'a>(title: String, g: GroupRef) -> Element<'a, Message> {
    match editable(g) {
        Some(id) => widget::text_input(super::texts().group_name, title)
            .id(widget::Id::new(GROUP_NAME))
            .on_input(move |s| Message::GroupName(id, s))
            .on_submit(|_| Message::GroupNameDone)
            .style(theme::TextInput::Inline)
            .size(18)
            .width(Length::Fill)
            .into(),
        None => widget::container(widget::text::heading(title)).width(Length::Fill).center_x(Length::Fill).into(),
    }
}

/// The popup's content: title and a scrollable grid of member tiles; `ring` marks the app under the drag.
pub fn popup_body<'a>(
    title: String,
    g: GroupRef,
    members: &[&'a AppEntry],
    over: Option<Slot>,
    dragged: Option<&str>,
    ring: Option<&str>,
    active: bool,
    p: Palette,
    m: &Metrics,
) -> Element<'a, Message> {
    let mut grid = widget::column::with_capacity(grid_rows(members.len(), m.group_cols)).spacing(6);
    for (n, chunk) in members.chunks(m.group_cols).enumerate() {
        let mut row = widget::row::with_capacity(m.group_cols).spacing(4);
        for (k, app) in chunk.iter().enumerate() {
            let i = n * m.group_cols + k;
            let bar = over == Some(Slot::Member(g, i));
            let dim = dragged == Some(app.id.as_str());
            row = row.push(member_tile(app, g, i, bar, dim, ring == Some(app.id.as_str()), active, p));
        }
        for _ in 0..pad_cells(chunk.len(), m.group_cols) {
            row = row.push(widget::space::horizontal().width(Length::FillPortion(1)));
        }
        grid = grid.push(row);
    }
    let scroll = widget::container(widget::scrollable(grid)).max_height(m.group_max_height);
    widget::column::with_capacity(2)
        .push(title_view(title, g))
        .push(scroll)
        .spacing(8)
        .into()
}

/// A folder in "All": a card with its mini folder, the name under it.
pub fn folder_card<'a>(
    g: GroupRef,
    name: String,
    icons: &[icon::Handle],
    ring: bool,
    active: bool,
    p: Palette,
) -> Element<'a, Message> {
    let inner = widget::container(mini_folder(icons, capped_side(FOLDER_BOX)))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill);
    let click = Message::Menu(Some(Target::Group(g)));
    let source = g.pinnable().then_some(Source::Group(g));
    let card = widget::mouse_area(draggable(
        button::custom(inner)
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fixed(FOLDER_CARD_HEIGHT))
            .class(folder_class(p)),
        source,
        click,
        active,
    ))
    .on_right_press(Message::MenuFor(Target::GroupMenu(g), folder_id(g)));
    let stack = iced::widget::stack(vec![card.into(), drop_ring(ring, 8.0, p)]);
    let framed = widget::container(stack).width(Length::Fill).id(folder_id(g));
    widget::column::with_capacity(2)
        .push(framed)
        .push(
            widget::container(widget::text::caption(name).wrapping(Wrapping::None))
                .width(Length::Fill)
                .center_x(Length::Fill),
        )
        .spacing(4)
        .into()
}

/// A pinned group: a folder-colored square with its mini folder, as a pinned app's row.
pub fn pinned_group_tile<'a>(
    index: usize,
    g: GroupRef,
    name: String,
    icons: &[icon::Handle],
    bar: bool,
    ring: bool,
    dim: bool,
    active: bool,
    p: Palette,
) -> Element<'a, Message> {
    let id = scoped("pin", &index.to_string());
    let square = widget::container(folder_box(mini_folder(icons, PIN_BOX), PIN_RADIUS, p))
        .width(Length::Fill)
        .center_x(Length::Fill);
    let tile = widget::mouse_area(draggable(
        button::custom(square).padding([6, 0]).width(Length::Fill).class(flat_class(p, false)),
        Some(Source::Pinned(index)),
        Message::Menu(Some(Target::Group(g))),
        active,
    ))
    .on_right_press(Message::MenuFor(Target::GroupMenu(g), id.clone()));
    let stack = iced::widget::stack(vec![
        tile.into(),
        tile_overlay(dim, bar, p),
        drop_ring(ring, PIN_RADIUS, p),
    ]);
    let tip = widget::tooltip(stack, widget::text::caption(name), widget::tooltip::Position::Bottom)
        .delay(Duration::from_millis(500));
    widget::container(tip).width(Length::Fill).id(id).into()
}

/// Background of the "Unpin" bar: a red gradient when hot, a discreet gray otherwise.
pub fn unpin_background(hot: bool) -> Background {
    if !hot {
        return Background::Color(Color::from_rgba(0.5, 0.5, 0.5, 0.18));
    }
    let red = |a: f32| Color::from_rgba(0.80, 0.18, 0.18, a);
    Background::Gradient(iced::Gradient::Linear(
        iced::gradient::Linear::new(iced::Radians(std::f32::consts::FRAC_PI_2))
            .add_stop(0.0, red(0.55))
            .add_stop(1.0, red(0.15)),
    ))
}

/// The "Unpin" bar overlay: centered label over `unpin_background`; captures no events.
pub fn unpin_bar<'a>(label: &'a str, width: f32, hot: bool, p: Palette) -> Element<'a, Message> {
    widget::container(widget::text::caption(label).class(theme::Text::Color(p.text)))
        .width(Length::Fixed(width))
        .height(Length::Fixed(UNPIN_BAR_HEIGHT))
        .center_x(Length::Fixed(width))
        .center_y(Length::Fixed(UNPIN_BAR_HEIGHT))
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(unpin_background(hot)),
            border: Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        }))
        .into()
}

#[cfg(test)]
mod tests;
