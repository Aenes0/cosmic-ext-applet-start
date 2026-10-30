//! What the window, keyboard and layer events mean to the menu (pure).

use super::*;

/// Whether a size is the menu surface's: the button's window has others.
pub(super) fn accepts_size(surface: Option<window::Id>, id: window::Id) -> bool {
    surface == Some(id)
}

/// What the window, keyboard and layer events mean to the menu.
pub(super) fn window_event(event: iced::Event, status: event::Status, window: window::Id) -> Option<Message> {
    match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed { key, text, .. }) => match key {
            Key::Named(Named::Escape) => Some(Message::Escape),
            Key::Named(Named::ArrowDown) => Some(Message::Move(1)),
            Key::Named(Named::ArrowUp) => Some(Message::Move(-1)),
            // Typing goes to the search, wherever the focus went.
            _ if status == event::Status::Ignored => text
                .filter(|t| !t.is_empty() && !t.chars().any(char::is_control))
                .map(|t| Message::TypeAhead(t.to_string())),
            _ => None,
        },
        // The first configure of a layer surface is `Opened`, the next ones `Resized`.
        iced::Event::Window(window::Event::Opened { size, .. } | window::Event::Resized(size)) => {
            Some(Message::Resized(window, size))
        }
        iced::Event::PlatformSpecific(event::PlatformSpecific::Wayland(wayland::Event::Layer(layer, ..))) => {
            match layer {
                wayland::LayerEvent::Focused => Some(Message::Focused),
                wayland::LayerEvent::Unfocused => Some(Message::Unfocused),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opened_and_resized_both_give_the_size() {
        let (id, size) = (window::Id::unique(), iced::Size::new(1920.0, 1080.0));
        let opened = iced::Event::Window(window::Event::Opened { position: None, size });
        let resized = iced::Event::Window(window::Event::Resized(size));
        for event in [opened, resized] {
            assert!(matches!(
                window_event(event, event::Status::Ignored, id),
                Some(Message::Resized(w, s)) if w == id && s == size
            ));
        }
        assert!(window_event(iced::Event::Window(window::Event::Closed), event::Status::Ignored, id).is_none());
    }

    #[test]
    fn only_the_menu_surface_sets_the_size() {
        let (a, b) = (window::Id::unique(), window::Id::unique());
        assert!(accepts_size(Some(a), a));
        assert!(!accepts_size(Some(a), b));
        assert!(!accepts_size(None, a));
    }
}
