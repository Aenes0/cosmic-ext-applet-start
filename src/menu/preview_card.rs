//! The window preview's card: a grid of window cards over an open-app icon.

use super::*;
use super::preview::{self, CARD_H, CARD_W, GAP, PAD};

/// Characters of a title that fit the card's header.
const TITLE_CHARS: usize = 22;

/// The title to show: an empty or blank one reads "untitled".
fn card_title(title: &str) -> &str {
    if title.trim().is_empty() { texts().untitled } else { title }
}

/// `items` in rows of `cols` (0 counts as 1).
fn rows_of<T: Clone>(items: &[T], cols: usize) -> Vec<Vec<T>> {
    items.chunks(cols.max(1)).map(<[T]>::to_vec).collect()
}

/// True when the grid is taller than the room the card has.
fn needs_scroll(natural_h: f32, max_h: f32) -> bool {
    natural_h > max_h
}

impl StartMenu {
    /// The preview of the open app `key`, already placed over `anchor` inside
    /// `bounds`; `None` when the app has no windows.
    pub(super) fn preview_card(
        &self,
        key: &str,
        anchor: iced::Rectangle,
        bounds: iced::Rectangle,
        p: Palette,
    ) -> Option<Element<'_, Message>> {
        let group = self.group_of(key).filter(|g| !g.windows.is_empty())?;
        let windows: Vec<&Window> = group.windows.iter().filter_map(|&i| self.windows.get(i)).collect();
        if windows.is_empty() {
            return None;
        }
        let n = windows.len();
        let rect = preview::card_rect(anchor, n, bounds);
        let (cols, rows, _) = preview::grid(n, bounds.size());
        let mut grid = widget::column::with_capacity(rows).spacing(GAP);
        for line in rows_of(&windows, cols) {
            let mut row = widget::row::with_capacity(line.len()).spacing(GAP);
            for window in line {
                row = row.push(self.window_card(window, p));
            }
            grid = grid.push(row);
        }
        let natural = 2.0 * PAD + rows as f32 * CARD_H + rows.saturating_sub(1) as f32 * GAP;
        let grid = widget::container(grid).padding(PAD);
        let body: Element<Message> = if needs_scroll(natural, rect.height) {
            widget::scrollable(grid).height(Length::Fixed(rect.height)).into()
        } else {
            grid.into()
        };
        let card = widget::container(body)
            .width(Length::Fixed(rect.width))
            .height(Length::Fixed(rect.height))
            .class(theme::Container::custom(move |_| widget::container::Style {
                background: Some(Background::Color(p.popup)),
                text_color: Some(p.text),
                icon_color: Some(p.text),
                border: iced::Border { radius: 8.0.into(), width: 1.0, color: p.border },
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                ..Default::default()
            }));
        // Presses on the card must not reach the backdrop, which would hide the menu.
        let card = widget::mouse_area(card)
            .on_press(Message::Nothing)
            .on_right_press(Message::Nothing)
            .interaction(iced::mouse::Interaction::Idle);
        Some(iced::widget::pin(card).x(rect.x).y(rect.y).width(Length::Fill).height(Length::Fill).into())
    }

    /// One window: its app's icon, title and (under the pointer) close
    /// button on top, its picture under them.
    fn window_card(&self, window: &Window, p: Palette) -> Element<'_, Message> {
        let id = window.id.clone();
        let hot = self.preview_hot.as_deref() == Some(id.as_str());
        let app_icon = self.window_icon(window);
        let close: Element<Message> = if hot {
            button::custom(icon::from_name("window-close-symbolic").size(16))
                .class(theme::Button::Destructive)
                .on_press(Message::CloseWindow(id.clone()))
                .padding(4)
                .into()
        } else {
            // The same room, so the title doesn't move.
            widget::space::horizontal().width(Length::Fixed(24.0)).into()
        };
        let header = widget::row::with_capacity(3)
            .push(widget::icon::icon(app_icon.clone()).size(16))
            .push(
                widget::text::body(ellipsize(card_title(&window.title), TITLE_CHARS))
                    .wrapping(Wrapping::None)
                    .width(Length::Fill),
            )
            .push(close)
            .spacing(8)
            .height(Length::Fixed(24.0))
            .align_y(Alignment::Center);
        // No picture (minimized, or not taken yet): the app's icon, big.
        let picture: Element<Message> = match self.thumbs.get(&id) {
            Some(handle) => iced::widget::image(handle.clone()).content_fit(iced::ContentFit::Contain).into(),
            None => widget::icon::icon(app_icon).size(48).into(),
        };
        let picture = widget::container(picture).center(Length::Fill).class(theme::Container::custom(|t| {
            widget::container::Style {
                border: iced::Border { radius: t.cosmic().radius_s().into(), ..Default::default() },
                ..Default::default()
            }
        }));
        let content = widget::column::with_capacity(2).push(header).push(picture).spacing(6);
        let card = button::custom(content)
            .on_press(Message::Activate(id.clone()))
            .class(flat_class(p, window.active))
            .width(Length::Fixed(CARD_W))
            .height(Length::Fixed(CARD_H))
            .padding([4, 4, 8, 8]);
        widget::mouse_area(card)
            .on_enter(Message::PreviewHot(Some(id.clone())))
            .on_exit(Message::PreviewHot(None))
            .on_middle_press(Message::CloseWindow(id))
            .into()
    }
}

#[cfg(test)]
mod tests;
