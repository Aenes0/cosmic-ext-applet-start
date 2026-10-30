// SPDX-License-Identifier: GPL-3.0-only

//! The uninstall dialog: its phases (pure, tested) and its card.

use cosmic::{
    Element,
    iced::{Alignment, Background, Border, Color, Length, widget::text::Wrapping},
    theme,
    widget::{self, button},
};

use super::{AppEntry, Message, Palette, symbolic};
use crate::{
    texts::texts,
    uninstall::{self, Origin, Plan},
};

/// Where the dialog is.
#[derive(Clone, Debug)]
pub enum Phase<P = Plan> {
    Planning,
    Ready(P),
    Running,
    Failed(String),
}

/// What happens to the dialog.
#[derive(Clone, Debug)]
pub enum Event<P = Plan> {
    Planned(Result<P, String>),
    Confirm,
    Done(Result<(), String>),
}

/// The dialog of one app.
#[derive(Clone, Debug)]
pub struct Dialog {
    pub id: String,
    pub phase: Phase,
    pub delete_data: bool,
}

impl Dialog {
    pub fn new(id: String) -> Self {
        Self {
            id,
            phase: Phase::Planning,
            delete_data: false,
        }
    }

    pub fn running(&self) -> bool {
        matches!(self.phase, Phase::Running)
    }

    /// Whether a new window of the menu must keep the dialog, so the
    /// outcome is not lost.
    pub fn keep_on_show(&self) -> bool {
        matches!(self.phase, Phase::Running | Phase::Failed(_))
    }
}

/// Whether the app's menu offers to uninstall it.
pub fn uninstall_visible(app: &AppEntry) -> bool {
    origin_offers(&app.origin)
}

fn origin_offers(origin: &Origin) -> bool {
    origin.removable()
}

/// The pinned without `gone`, if it no longer `exists`; `None` if nothing
/// changes.
pub fn prune(pinned: &[String], gone: &str, exists: impl Fn(&str) -> bool) -> Option<Vec<String>> {
    if exists(gone) || !pinned.iter().any(|p| p == gone) {
        return None;
    }
    Some(pinned.iter().filter(|p| *p != gone).cloned().collect())
}

/// The next phase; `None` is the dialog gone. Events out of order change
/// nothing.
pub fn advance<P: Clone>(phase: &Phase<P>, event: Event<P>) -> Option<Phase<P>> {
    match (phase, event) {
        (Phase::Planning, Event::Planned(Ok(plan))) => Some(Phase::Ready(plan)),
        (Phase::Planning, Event::Planned(Err(err))) => Some(Phase::Failed(err)),
        (Phase::Ready(_), Event::Confirm) => Some(Phase::Running),
        (Phase::Running, Event::Done(Ok(()))) => None,
        (Phase::Running, Event::Done(Err(err))) => Some(Phase::Failed(err)),
        (other, _) => Some(other.clone()),
    }
}

/// The source's name, before the plan has its own label.
fn origin_name(origin: &Origin) -> String {
    match origin {
        Origin::Flatpak { .. } => "Flatpak".into(),
        Origin::Steam { .. } => "Steam".into(),
        Origin::Waydroid { .. } => "Waydroid".into(),
        Origin::Wine { .. } => "Wine".into(),
        Origin::AppImage { .. } => "AppImage".into(),
        Origin::Pacman => "pacman".into(),
        Origin::LooseDesktop => "Atalho".into(),
        Origin::Protected | Origin::Unknown => String::new(),
    }
}

/// Most lines of the preview shown before "and N more".
const MAX_LINES: usize = 6;

/// How a button looks.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Neutral,
    Accent,
    Danger,
}

/// The confirm button's look: red only when this menu deletes; the hand-over
/// to another program's uninstaller is a normal action.
fn confirm_kind(hands_over: bool) -> Kind {
    if hands_over {
        Kind::Accent
    } else {
        Kind::Danger
    }
}

/// The lines to show and how many were left out.
fn visible_lines(lines: &[String], max: usize) -> (&[String], usize) {
    if lines.len() <= max {
        (lines, 0)
    } else {
        (&lines[..max], lines.len() - max)
    }
}

fn kind_class(p: Palette, kind: Kind) -> theme::Button {
    let style = move |hovered: bool, pressed: bool| {
        let shade = |base: Color, hover: Color, down: Color| {
            if pressed {
                down
            } else if hovered {
                hover
            } else {
                base
            }
        };
        let (background, text_color, border_width) = match kind {
            Kind::Danger => (
                shade(
                    Color::from_rgb(0.80, 0.18, 0.18),
                    Color::from_rgb(0.70, 0.14, 0.14),
                    Color::from_rgb(0.60, 0.11, 0.11),
                ),
                Color::WHITE,
                0.0,
            ),
            Kind::Accent => (
                shade(
                    p.accent,
                    Color {
                        a: 0.85,
                        ..p.accent
                    },
                    Color { a: 0.7, ..p.accent },
                ),
                Color::WHITE,
                0.0,
            ),
            Kind::Neutral => (shade(p.folder, p.selected, p.hover), p.text, 1.0),
        };
        button::Style {
            background: Some(Background::Color(background)),
            border_radius: 6.0.into(),
            border_width,
            border_color: p.border,
            text_color: Some(text_color),
            icon_color: Some(text_color),
            ..button::Style::new()
        }
    };
    theme::Button::Custom {
        active: Box::new(move |_, _| style(false, false)),
        disabled: Box::new(move |_| style(false, false)),
        hovered: Box::new(move |_, _| style(true, false)),
        pressed: Box::new(move |_, _| style(true, true)),
    }
}

/// Buttons on the right: the neutral one, then the confirm one.
fn button_row<'a>(
    p: Palette,
    cancel: Option<(&'static str, Message)>,
    confirm: Option<(&'static str, Message, Kind)>,
) -> Element<'a, Message> {
    let make = |label: &'static str, message: Message, kind: Kind| {
        button::custom(widget::text::body(label))
            .padding([8, 20])
            .class(kind_class(p, kind))
            .on_press(message)
    };
    let mut row = widget::row::with_capacity(3)
        .spacing(8)
        .align_y(Alignment::Center);
    row = row.push(widget::space::horizontal());
    if let Some((label, message)) = cancel {
        row = row.push(make(label, message, Kind::Neutral));
    }
    if let Some((label, message, kind)) = confirm {
        row = row.push(make(label, message, kind));
    }
    row.into()
}

/// A box with a subtle background and rounded corners.
fn panel<'a>(content: impl Into<Element<'a, Message>>, p: Palette) -> Element<'a, Message> {
    widget::container(content)
        .padding(12)
        .width(Length::Fill)
        .class(theme::Container::custom(move |_| {
            widget::container::Style {
                background: Some(Background::Color(p.field)),
                border: Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: p.border,
                },
                ..Default::default()
            }
        }))
        .into()
}

/// The source's chip.
fn chip<'a>(name: String, p: Palette) -> Element<'a, Message> {
    widget::container(widget::text::caption(name).class(theme::Text::Color(p.accent)))
        .padding([2, 10])
        .class(theme::Container::custom(move |_| {
            widget::container::Style {
                background: Some(Background::Color(Color {
                    a: 0.16,
                    ..p.accent
                })),
                border: Border {
                    radius: 10.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into()
}

/// The app's big icon, its name and its source.
fn header<'a>(app: &AppEntry, origin: String, p: Palette) -> Element<'a, Message> {
    let mut names = widget::column::with_capacity(2)
        .spacing(6)
        .push(widget::text::title3(app.name.clone()).wrapping(Wrapping::WordOrGlyph));
    if !origin.is_empty() {
        names = names.push(chip(origin, p));
    }
    widget::row::with_children(vec![
        widget::icon(app.icon.clone()).size(48).into(),
        widget::container(names).width(Length::Fill).into(),
    ])
    .spacing(14)
    .align_y(Alignment::Center)
    .into()
}

fn preview_list<'a>(lines: &[String], p: Palette) -> Element<'a, Message> {
    let (shown, more) = visible_lines(lines, MAX_LINES);
    let mut list = widget::column::with_capacity(shown.len() + 1).spacing(8);
    for line in shown {
        list = list.push(
            widget::row::with_children(vec![
                widget::icon::icon(symbolic("package-x-generic-symbolic"))
                    .size(16)
                    .into(),
                widget::text::caption(line.clone())
                    .wrapping(Wrapping::WordOrGlyph)
                    .width(Length::Fill)
                    .into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    if more > 0 {
        list = list.push(
            widget::text::caption(texts().and_more.replace("{}", &more.to_string()))
                .class(theme::Text::Color(p.dim)),
        );
    }
    panel(list, p)
}

/// Room for the card's header, buttons and spacing (48 icon + ~38 buttons
/// + 3 x 16 gaps + 8 padding + the card's own padding).
const CARD_CHROME: f32 = 160.0;
/// The body never shrinks below this.
const BODY_MIN: f32 = 96.0;

/// The tallest the scrolling body may be inside a card of `card_max`.
pub fn body_max_height(card_max: f32) -> f32 {
    (card_max - CARD_CHROME).max(BODY_MIN)
}

fn ready_body<'a>(
    app: &AppEntry,
    plan: &Plan,
    delete_data: bool,
    max_height: f32,
    p: Palette,
) -> Vec<Element<'a, Message>> {
    let t = texts();
    let hands_over = plan.opens_other_program();
    let mut middle = vec![
        widget::text::heading(if hands_over {
            t.will_open
        } else {
            t.will_remove
        })
        .into(),
        preview_list(&plan.preview, p),
    ];
    if plan.supports_data_delete {
        middle.push(
            widget::container(
                widget::checkbox(delete_data)
                    .label(t.delete_data)
                    .on_toggle(Message::ToggleDeleteData),
            )
            .padding([4, 4])
            .into(),
        );
    }
    // Header and buttons stay put; only the middle scrolls.
    let scroll = widget::scrollable(widget::column::with_children(middle).spacing(16).width(Length::Fill));
    vec![
        header(app, plan.label.clone(), p),
        widget::container(scroll).max_height(body_max_height(max_height)).into(),
        button_row(
            p,
            Some((t.cancel, Message::Menu(None))),
            Some((
                if hands_over {
                    t.open_uninstaller
                } else {
                    t.uninstall_confirm
                },
                Message::ConfirmUninstall,
                confirm_kind(hands_over),
            )),
        ),
    ]
}

fn centered<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    widget::container(content)
        .width(Length::Fill)
        .padding([16, 0])
        .align_x(Alignment::Center)
        .into()
}

fn failed_body<'a>(err: &str, p: Palette) -> Vec<Element<'a, Message>> {
    let t = texts();
    let title = widget::row::with_children(vec![
        widget::icon::icon(symbolic("dialog-warning-symbolic"))
            .size(24)
            .into(),
        widget::text::title4(t.uninstall_failed).into(),
    ])
    .spacing(10)
    .align_y(Alignment::Center);
    vec![
        title.into(),
        panel(
            widget::text::caption(uninstall::cut(err)).wrapping(Wrapping::WordOrGlyph),
            p,
        ),
        button_row(p, Some((t.close, Message::Menu(None))), None),
    ]
}

/// The card's content for the dialog of `app`.
pub fn card_body<'a>(
    app: &AppEntry,
    dialog: &Dialog,
    max_height: f32,
    p: Palette,
) -> Vec<Element<'a, Message>> {
    let t = texts();
    let content = match &dialog.phase {
        Phase::Planning => vec![
            header(app, origin_name(&app.origin), p),
            centered(widget::text::body(t.checking).into()),
            button_row(p, Some((t.cancel, Message::Menu(None))), None),
        ],
        Phase::Ready(plan) => ready_body(app, plan, dialog.delete_data, max_height, p),
        Phase::Running => vec![
            header(app, origin_name(&app.origin), p),
            centered(
                widget::text::heading(t.uninstalling)
                    .class(theme::Text::Color(p.dim))
                    .into(),
            ),
        ],
        Phase::Failed(err) => failed_body(err, p),
    };
    vec![
        widget::column::with_children(content)
            .spacing(16)
            .padding(4)
            .width(Length::Fill)
            .into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> u8 {
        1
    }

    #[test]
    fn planning_goes_to_ready_or_failed() {
        assert!(matches!(
            advance(&Phase::Planning, Event::Planned(Ok(plan()))),
            Some(Phase::Ready(_))
        ));
        assert!(
            matches!(advance(&Phase::<u8>::Planning, Event::Planned(Err("e".into()))), Some(Phase::Failed(e)) if e == "e")
        );
    }

    #[test]
    fn only_ready_can_be_confirmed() {
        assert!(matches!(
            advance(&Phase::Ready(plan()), Event::Confirm),
            Some(Phase::Running)
        ));
        assert!(matches!(
            advance(&Phase::<u8>::Planning, Event::Confirm),
            Some(Phase::Planning)
        ));
        assert!(matches!(
            advance(&Phase::<u8>::Running, Event::Confirm),
            Some(Phase::Running)
        ));
    }

    #[test]
    fn done_closes_on_success_and_fails_on_error() {
        assert!(advance(&Phase::<u8>::Running, Event::Done(Ok(()))).is_none());
        assert!(matches!(
            advance(&Phase::<u8>::Running, Event::Done(Err("boom".into()))),
            Some(Phase::Failed(_))
        ));
    }

    #[test]
    fn a_late_result_changes_nothing() {
        assert!(matches!(
            advance(&Phase::<u8>::Failed("a".into()), Event::Done(Ok(()))),
            Some(Phase::Failed(_))
        ));
        assert!(matches!(
            advance(&Phase::Ready(plan()), Event::Planned(Err("x".into()))),
            Some(Phase::Ready(_))
        ));
    }

    #[test]
    fn running_and_failed_survive_a_show() {
        let mut d = Dialog::new("a".into());
        assert!(!d.keep_on_show() && !d.running());
        d.phase = Phase::Running;
        assert!(d.keep_on_show() && d.running());
        d.phase = Phase::Failed("e".into());
        assert!(d.keep_on_show() && !d.running());
    }

    #[test]
    fn errors_are_cut() {
        assert_eq!(uninstall::cut("  ok "), "ok");
        assert_eq!(uninstall::cut(&"a".repeat(500)).chars().count(), 400);
    }

    #[test]
    fn only_removable_origins_show_the_item() {
        let removable = [
            Origin::Flatpak {
                reference: "a.B".into(),
                system: false,
            },
            Origin::Steam { app_id: "1".into() },
            Origin::Waydroid {
                package: "a.b".into(),
            },
            Origin::Wine {
                prefix: "/p".into(),
            },
            Origin::AppImage {
                file: "/a.AppImage".into(),
            },
            Origin::Pacman,
            Origin::LooseDesktop,
        ];
        assert!(removable.iter().all(origin_offers));
        assert!(!origin_offers(&Origin::Unknown));
        assert!(!origin_offers(&Origin::Protected));
    }

    struct NoRunner;

    impl uninstall::Runner for NoRunner {
        fn run(&self, _: &str, _: &[String]) -> Result<uninstall::Out, String> {
            Ok(uninstall::Out {
                ok: true,
                ..Default::default()
            })
        }
    }

    #[test]
    fn the_chip_does_not_change_between_planning_and_ready() {
        let facts = uninstall::DesktopFacts {
            path: "/home/u/.local/share/applications/a.desktop".into(),
            ..Default::default()
        };
        let origins = [
            Origin::Flatpak {
                reference: "a.B".into(),
                system: false,
            },
            Origin::Steam { app_id: "1".into() },
            Origin::Wine {
                prefix: "/p".into(),
            },
            Origin::AppImage {
                file: "/a.AppImage".into(),
            },
            Origin::LooseDesktop,
        ];
        for origin in origins {
            let plan = uninstall::plan(&origin, &facts, &NoRunner).unwrap();
            assert_eq!(origin_name(&origin), plan.label, "{origin:?}");
        }
    }

    #[test]
    fn long_previews_are_cut_with_a_count() {
        let lines: Vec<String> = (0..9).map(|n| n.to_string()).collect();
        assert_eq!(visible_lines(&lines, 6), (&lines[..6], 3));
        assert_eq!(visible_lines(&lines[..6], 6), (&lines[..6], 0));
        assert_eq!(visible_lines(&[], 6).1, 0);
    }

    #[test]
    fn the_body_gets_what_the_card_leaves() {
        assert_eq!(body_max_height(856.0), 696.0);
        assert_eq!(body_max_height(300.0), 140.0);
        assert_eq!(body_max_height(100.0), 96.0);
    }

    #[test]
    fn only_deleting_is_red() {
        assert_eq!(confirm_kind(false), Kind::Danger);
        assert_eq!(confirm_kind(true), Kind::Accent);
    }

    #[test]
    fn prune_only_the_one_gone() {
        let pinned = vec!["a".to_owned(), "b".to_owned()];
        assert_eq!(prune(&pinned, "a", |_| false), Some(vec!["b".to_owned()]));
        assert_eq!(prune(&pinned, "a", |_| true), None);
        assert_eq!(prune(&pinned, "z", |_| false), None);
    }
}
