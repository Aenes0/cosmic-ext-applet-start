// SPDX-License-Identifier: GPL-3.0-only

//! The Start button's right-click menu, Windows' Win+X: the system's tools
//! in a list, as Windows has them, each to what does that here. What has
//! nothing to do it on this system (Device Manager, Mobility Center…)
//! stays out; Disk Management shows with GNOME Disks there.

use cosmic::{
    Element,
    iced::{self, Alignment, Background, Color, Length},
    theme,
    widget::{self, button},
};

use crate::{
    menu::{Palette, flat_class, spawn_clean},
    texts::texts,
};

#[derive(Clone, Debug)]
pub enum Tool {
    /// A program and its arguments.
    Run(&'static str, Vec<String>),
    /// A command in a terminal that takes one (COSMIC's doesn't).
    InTerminal(Vec<String>),
    Search,
    ShowDesktop,
    /// Opens or closes "Shut down or sign out".
    PowerMenu,
}

impl Tool {
    /// Does it; the menu closes after, unless it opened the power one.
    pub fn run(&self) {
        match self {
            Tool::Run(program, args) => {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                spawn_clean(program, &args);
            }
            Tool::InTerminal(command) => {
                let mut args = vec!["-e"];
                args.extend(command.iter().map(String::as_str));
                match terminal_with_command() {
                    Some(terminal) => spawn_clean(terminal, &args),
                    None => spawn_clean("cosmic-term", &[]),
                }
            }
            Tool::Search => spawn_clean(&crate::menu::self_exe().to_string_lossy(), &["--menu"]),
            Tool::ShowDesktop => {
                spawn_clean(&crate::menu::self_exe().to_string_lossy(), &["--show-desktop"]);
            }
            Tool::PowerMenu => {}
        }
    }
}

/// A terminal that runs what it's given with `-e`.
fn terminal_with_command() -> Option<&'static str> {
    ["alacritty", "kitty", "foot", "wezterm", "konsole", "gnome-terminal", "xterm"]
        .into_iter()
        .find(|t| which(t))
}

fn which(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
    })
}

fn settings(page: &str) -> Tool {
    Tool::Run("cosmic-settings", vec![page.into()])
}

/// The list, as Windows orders it; `None` is a line between groups.
fn items() -> Vec<Option<(&'static str, &'static str, Tool)>> {
    let t = texts();
    let mut items = vec![
        Some(("view-app-grid-symbolic", t.installed_apps, settings("applications"))),
        Some(("battery-good-symbolic", t.power_options, settings("power"))),
        Some((
            "text-x-generic-symbolic",
            t.event_viewer,
            Tool::InTerminal(vec!["journalctl".into(), "-b".into(), "-e".into()]),
        )),
        Some(("computer-symbolic", t.system, settings("about"))),
        Some(("network-wired-symbolic", t.network_connections, settings("network"))),
    ];
    if which("gnome-disks") {
        items.push(Some(("drive-harddisk-symbolic", t.disk_management, Tool::Run("gnome-disks", Vec::new()))));
    }
    items.extend([
        Some(("utilities-terminal-symbolic", t.terminal, Tool::Run("cosmic-term", Vec::new()))),
        Some((
            "security-high-symbolic",
            t.terminal_admin,
            Tool::InTerminal(vec!["sudo".into(), "-i".into()]),
        )),
        None,
        Some((
            "utilities-system-monitor-symbolic",
            t.task_manager,
            if which("cosmic-monitor") {
                Tool::Run("cosmic-monitor", Vec::new())
            } else {
                Tool::InTerminal(vec!["btop".into()])
            },
        )),
        Some(("emblem-system-symbolic", t.settings, Tool::Run("cosmic-settings", Vec::new()))),
        Some(("system-file-manager-symbolic", t.file_explorer, Tool::Run("cosmic-files", Vec::new()))),
        Some(("system-search-symbolic", t.search, Tool::Search)),
        Some(("system-run-symbolic", t.run, Tool::Run("cosmic-launcher", Vec::new()))),
        None,
        Some(("system-shutdown-symbolic", t.shut_down_or_sign_out, Tool::PowerMenu)),
        Some(("user-desktop-symbolic", t.desktop, Tool::ShowDesktop)),
    ]);
    items
}

fn power_items() -> Vec<(&'static str, &'static str, Tool)> {
    let t = texts();
    vec![
        ("system-log-out-symbolic", t.log_out, Tool::Run("cosmic-osd", vec!["log-out".into()])),
        ("system-suspend-symbolic", t.suspend, Tool::Run("systemctl", vec!["suspend".into()])),
        ("system-shutdown-symbolic", t.power_off, Tool::Run("cosmic-osd", vec!["shutdown".into()])),
        ("system-reboot-symbolic", t.restart, Tool::Run("cosmic-osd", vec!["restart".into()])),
    ]
}

fn card<'a, M: 'static>(content: impl Into<Element<'a, M>>, width: f32, p: Palette) -> Element<'a, M> {
    widget::container(content)
        .width(Length::Fixed(width))
        .padding(4)
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: Some(Background::Color(p.popup)),
            text_color: Some(p.text),
            icon_color: Some(p.text),
            border: iced::Border {
                radius: 8.0.into(),
                width: 1.0,
                color: p.border,
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        }))
        .into()
}

fn row<'a, M: Clone + 'static>(icon: &'static str, label: &'static str, arrow: bool, message: M, p: Palette) -> Element<'a, M> {
    let mut row = widget::row::with_capacity(3)
        .spacing(12)
        .align_y(Alignment::Center)
        .push(widget::icon::icon(crate::menu::symbolic(icon)).size(16))
        .push(widget::text::body(label).width(Length::Fill));
    if arrow {
        row = row.push(widget::icon::from_name("go-next-symbolic").size(12));
    }
    button::custom(row)
        .padding([7, 12])
        .width(Length::Fill)
        .class(flat_class(p, false))
        .on_press(message)
        .into()
}

/// The menu; with "Shut down or sign out" open, its own beside it, level
/// with that line, as Windows has it.
pub fn view<'a, M: Clone + 'static>(power_open: bool, on: impl Fn(Tool) -> M, p: Palette) -> Element<'a, M> {
    let mut list = widget::column::with_capacity(20).spacing(0);
    for item in items() {
        list = list.push(match item {
            Some((icon, label, tool)) => {
                let arrow = matches!(tool, Tool::PowerMenu);
                row(icon, label, arrow, on(tool), p)
            }
            None => widget::container(
                widget::container(widget::space::horizontal())
                    .width(Length::Fill)
                    .height(Length::Fixed(1.0))
                    .class(theme::Container::custom(move |_| widget::container::Style {
                        background: Some(Background::Color(p.border)),
                        ..Default::default()
                    })),
            )
            .padding([4, 8])
            .into(),
        });
    }
    let main = card(list, 250.0, p);
    if !power_open {
        return main;
    }
    let mut power = widget::column::with_capacity(4);
    for (icon, label, tool) in power_items() {
        power = power.push(row(icon, label, false, on(tool), p));
    }
    // Its foot a line above the menu's: by "Shut down or sign out".
    widget::row::with_children(vec![
        main,
        widget::container(card(power, 190.0, p))
            .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 42.0, left: 0.0 })
            .into(),
    ])
    .spacing(2)
    .align_y(Alignment::End)
    .into()
}
