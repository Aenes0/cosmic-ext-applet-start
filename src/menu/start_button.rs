use super::*;

/// The panel's button: four squares, as Windows has it. A click opens the
/// menu, or closes it; a right click, the tools' menu (Win+X), by it.
pub struct StartButton {
    core: app::Core,
    popup: Option<window::Id>,
    power_open: bool,
}

#[derive(Clone, Debug)]
pub enum ButtonMessage {
    Click,
    RightClick,
    Tool(crate::tools::Tool),
    Closed(window::Id),
}

impl cosmic::Application for StartButton {
    type Message = ButtonMessage;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = crate::APP_ID;

    fn init(mut core: app::Core, _flags: ()) -> (Self, app::Task<ButtonMessage>) {
        core.set_auto_blur(Default::default());
        (Self { core, popup: None, power_open: false }, Task::none())
    }

    fn core(&self) -> &app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut app::Core {
        &mut self.core
    }

    fn style(&self) -> Option<iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn update(&mut self, message: ButtonMessage) -> app::Task<ButtonMessage> {
        match message {
            ButtonMessage::Click => {
                // The menu running answers over D-Bus; else it starts.
                std::thread::spawn(|| {
                    if !toggle_running() {
                        spawn_clean(&self_exe().to_string_lossy(), &["--menu"]);
                    }
                });
                return self.close();
            }
            ButtonMessage::RightClick => {
                if self.popup.is_some() {
                    return self.close();
                }
                let Some(parent) = self.core.main_window_id() else {
                    return Task::none();
                };
                let id = window::Id::unique();
                self.popup = Some(id);
                self.power_open = false;
                return cosmic::surface::surface_task(cosmic::surface::action::app_popup(
                    |_: &Self| Default::default(),
                    move |app: &mut Self| {
                        let mut settings = app.core.applet.get_popup_settings(parent, id, None, None, None);
                        settings.positioner.size_limits =
                            Limits::NONE.min_width(1.0).min_height(1.0).max_width(500.0).max_height(800.0);
                        settings
                    },
                    None,
                ));
            }
            ButtonMessage::Tool(tool) => {
                if matches!(tool, crate::tools::Tool::PowerMenu) {
                    self.power_open = !self.power_open;
                    return Task::none();
                }
                tool.run();
                return self.close();
            }
            ButtonMessage::Closed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, ButtonMessage> {
        let applet = &self.core.applet;
        let (major, minor) = applet.suggested_padding(true);
        let size = applet.suggested_size(true).0 as f32;
        let square = (size - 2.0) / 2.0;
        let tile = || {
            widget::container(widget::space::horizontal())
                .width(Length::Fixed(square))
                .height(Length::Fixed(square))
                .class(theme::Container::custom(|theme| widget::container::Style {
                    background: Some(Background::Color(theme.cosmic().accent_color().into())),
                    border: iced::Border {
                        radius: 2.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }))
        };
        let logo = widget::column::with_children(vec![
            widget::row::with_children(vec![tile().into(), tile().into()]).spacing(2).into(),
            widget::row::with_children(vec![tile().into(), tile().into()]).spacing(2).into(),
        ])
        .spacing(2);
        let horizontal = applet.is_horizontal();
        let button = button::custom(logo)
            .padding(if horizontal { [minor, major] } else { [major, minor] })
            .class(theme::Button::AppletIcon)
            .on_press(ButtonMessage::Click);
        applet
            .autosize_window(widget::mouse_area(button).on_right_press(ButtonMessage::RightClick))
            .into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, ButtonMessage> {
        if self.popup != Some(id) {
            return widget::text("").into();
        }
        let p = palette(&cosmic::theme::active());
        cosmic::widget::autosize::autosize(
            crate::tools::view(self.power_open, ButtonMessage::Tool, p),
            cosmic::widget::Id::new("start-tools-autosize"),
        )
        .into()
    }

    fn on_close_requested(&self, id: window::Id) -> Option<ButtonMessage> {
        Some(ButtonMessage::Closed(id))
    }
}

impl StartButton {
    fn close(&mut self) -> app::Task<ButtonMessage> {
        match self.popup.take() {
            Some(id) => cosmic::iced::platform_specific::shell::commands::popup::destroy_popup(id),
            None => Task::none(),
        }
    }
}
