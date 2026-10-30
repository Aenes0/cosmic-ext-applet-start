use super::*;

/// Arms a drag on press, and launches on click; while a drag is active a
/// release over the source must not launch, so `on_press` is dropped then.
pub(super) fn draggable<'a>(
    b: button::Button<'a, Message>,
    source: Option<drag::Source>,
    launch: Message,
    active: bool,
) -> button::Button<'a, Message> {
    let Some(source) = source else {
        return b.on_press(launch);
    };
    b.on_press_maybe((!active).then_some(launch))
        .on_press_down_with_rectange(move |_, r| Message::DragArm(source.clone(), r.center()))
}

/// Over a tile: dimmed if it is what is dragged, with an accent bar at its
/// left if a drop would go before it. Always there, so the tree is the same.
pub(super) fn tile_overlay<'a>(dim: bool, bar: bool, p: Palette) -> Element<'a, Message> {
    let bar = widget::container(widget::space::horizontal())
        .width(Length::Fixed(3.0))
        .height(Length::Fill)
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: bar.then(|| Background::Color(p.accent)),
            ..Default::default()
        }));
    widget::container(bar)
        .width(Length::Fill)
        .height(Length::Fill)
        .class(theme::Container::custom(move |_| widget::container::Style {
            background: dim.then(|| Background::Color(Color { a: 0.6, ..p.bg })),
            border: iced::Border { radius: 4.0.into(), ..Default::default() },
            ..Default::default()
        }))
        .into()
}

/// Where, and whether hot, the unpin bar is drawn: while a pinned is dragged and
/// its target is measured.
pub(super) fn unpin_bar_at(drag: &drag::Drag, targets: &[drag::Target]) -> Option<(iced::Rectangle, bool)> {
    let drag::Drag::Dragging { source: drag::Source::Pinned(_), over, .. } = drag else {
        return None;
    };
    let bar = targets.iter().find(|t| t.slot == drag::Slot::Unpin)?;
    Some((bar.rect, *over == Some(drag::Slot::Unpin)))
}

/// A spec that takes the whole rectangle.
fn whole(slot: drag::Slot, layer: drag::Layer) -> drag::Spec {
    drag::Spec { slot, layer, zone: drag::Zone::Whole }
}

/// The specs of the visible pinned tiles: the left half goes before the tile, the
/// right one before the next visible (the last one to `end`), the center into a group.
fn pinned_specs(
    visible: &[usize],
    end: drag::Slot,
    center_at: &dyn Fn(usize) -> Option<drag::Slot>,
    centers: bool,
) -> Vec<Vec<drag::Spec>> {
    visible
        .iter()
        .enumerate()
        .map(|(k, &i)| {
            let after = visible.get(k + 1).map_or(end, |&next| drag::Slot::Before(next));
            let center = center_at(i).filter(|_| centers);
            drag::tile_specs(drag::Slot::Before(i), after, drag::Layer::Tile, center)
        })
        .collect()
}

/// The specs of the `n` members of a group's popup; `apps` (their indices in the apps)
/// gives them a center to make a group, except in a user group: no folders inside folders.
fn member_specs(g: groups::GroupRef, n: usize, apps: Option<&[usize]>) -> Vec<Vec<drag::Spec>> {
    let apps = apps.filter(|_| group_ui::editable(g).is_none());
    (0..n)
        .map(|i| {
            let after = if i + 1 < n { drag::Slot::Member(g, i + 1) } else { drag::Slot::MemberEnd(g) };
            let mut specs = drag::tile_specs(drag::Slot::Member(g, i), after, drag::Layer::PopupTile, None);
            if let Some(&app) = apps.and_then(|a| a.get(i)) {
                specs.push(drag::Spec {
                    slot: drag::Slot::OnApp(app),
                    layer: drag::Layer::PopupCenter,
                    zone: drag::Zone::Center,
                });
            }
            specs
        })
        .collect()
}

/// The indices in `self.pinned` of the pinned shown: two rows, or all.
fn visible_pinned(found: &[usize], expanded: bool, shown: usize) -> Vec<usize> {
    let shown = if expanded { found.len() } else { shown };
    found.iter().copied().take(shown).collect()
}

/// Whether the pinned tile at `index` is where the drop would go before.
pub(super) fn drop_before(over: Option<drag::Slot>, index: usize) -> bool {
    over == Some(drag::Slot::Before(index))
}

/// The drag's source, if it is being dragged: its pinned index or app id.
pub(super) fn dragged(drag: &drag::Drag) -> Option<&drag::Source> {
    match drag {
        drag::Drag::Dragging { source, .. } => Some(source),
        _ => None,
    }
}

/// Where a drop on the pinned area, not on a tile, goes: at the end, or,
/// with some hidden, after the last shown.
fn area_slot(found: &[usize], expanded: bool, shown: usize) -> drag::Slot {
    match found.get(shown) {
        Some(next) if !expanded => drag::Slot::Before(*next),
        _ => drag::Slot::End,
    }
}

impl StartMenu {
    /// The icon of what is dragged, under the cursor; it takes no events.
    pub(super) fn ghost(&self) -> Option<Element<'_, Message>> {
        let drag::Drag::Dragging { source, at, .. } = &self.drag else {
            return None;
        };
        let p = palette(&cosmic::theme::active());
        let folder = |apps: &[&AppEntry]| (group_ui::ghost_folder(&icons_of(apps), p), group_ui::PIN_BOX);
        let app = |app: &AppEntry| (widget::icon(app.icon_at(px(36.0))).size(px(36.0)).into(), f32::from(px(36.0)));
        let (content, size): (Element<'_, Message>, f32) = match source {
            drag::Source::Pinned(i) => match self.pinned_items().into_iter().find(|(n, _)| n == i)?.1 {
                PinnedItem::App(a) => app(a),
                PinnedItem::Group(_, members) => folder(&members),
            },
            drag::Source::App(id) | drag::Source::Member(_, id) => app(find_app(&self.apps, id)?),
            drag::Source::Group(g) => folder(&self.group_apps(*g)),
        };
        Some(iced::widget::pin(content).x(at.x - size / 2.0).y(at.y - size / 2.0).into())
    }

    pub(super) fn cancel_drag(&mut self) {
        self.drag = std::mem::take(&mut self.drag).cancel();
        self.drag_targets.clear();
    }

    /// Arms the drag and asks where every target it may land on is.
    pub(super) fn arm_drag(&mut self, source: drag::Source, point: iced::Point) -> app::Task<Message> {
        self.drag = drag::Drag::arm(source.clone(), point);
        self.drag_fresh = true;
        self.measure_targets(source)
    }

    /// Measures the targets again, if the layout moved under a drag.
    pub(super) fn remeasure(&mut self) -> app::Task<Message> {
        match &self.drag {
            drag::Drag::Armed { source, .. } | drag::Drag::Dragging { source, .. } => {
                let source = source.clone();
                self.measure_targets(source)
            }
            drag::Drag::Idle => Task::none(),
        }
    }

    fn measure_targets(&mut self, source: drag::Source) -> app::Task<Message> {
        let pinned_is_group = matches!(&source, drag::Source::Pinned(i)
            if self.pinned.get(*i).is_some_and(|e| groups::GroupRef::from_pin(e).is_some()));
        let wants = drag::wants(&source, pinned_is_group);
        self.drag_targets.clear();
        self.drag_gen = self.drag_gen.wrapping_add(1);
        let (ids, specs) = self.drag_specs(wants);
        let generation = self.drag_gen;
        anchor::find_many(ids)
            .map(move |rects| cosmic::Action::App(Message::DragTargets(generation, drag::targets(&specs, &rects))))
    }

    /// The ids to measure and what each one is, at the same index.
    fn drag_specs(&self, wants: drag::Wants) -> (Vec<widget::Id>, Vec<Vec<drag::Spec>>) {
        let found: Vec<usize> = self.pinned_items().iter().map(|(i, _)| *i).collect();
        let end = area_slot(&found, self.pinned_expanded, self.metrics.pinned_shown);
        let visible = visible_pinned(&found, self.pinned_expanded, self.metrics.pinned_shown);
        let center_at = |i: usize| {
            let entry = self.pinned.get(i)?;
            match groups::GroupRef::from_pin(entry) {
                Some(g) => Some(drag::Slot::Into(g)),
                None => self.app_index(entry).map(drag::Slot::OnApp),
            }
        };
        let mut ids: Vec<widget::Id> = visible.iter().map(|i| scoped("pin", &i.to_string())).collect();
        let mut specs = pinned_specs(&visible, end, &center_at, wants.centers);
        if wants.centers {
            for (idx, app) in self.apps.iter().enumerate() {
                ids.push(scoped("all", &app.id));
                specs.push(vec![drag::Spec {
                    slot: drag::Slot::OnApp(idx),
                    layer: drag::Layer::Center,
                    zone: drag::Zone::Center,
                }]);
            }
        }
        ids.push(widget::Id::new(PINNED_AREA));
        let mut area = vec![whole(end, drag::Layer::Area)];
        if wants.unpin {
            area.push(drag::Spec { slot: drag::Slot::Unpin, layer: drag::Layer::Unpin, zone: drag::Zone::Below });
        }
        specs.push(area);
        if wants.folders {
            for (g, _) in self.groups.visible(&self.app_keys()) {
                ids.push(group_ui::folder_id(g));
                specs.push(vec![whole(drag::Slot::Into(g), drag::Layer::Tile)]);
            }
        }
        if let (true, Some(Target::Group(g))) = (wants.popup, &self.menu) {
            let indices = self.group_indices(*g);
            ids.extend((0..indices.len()).map(|i| group_ui::member_id(*g, i)));
            specs.extend(member_specs(*g, indices.len(), wants.centers.then_some(indices.as_slice())));
            ids.push(group_ui::popup_card_id());
            specs.push(vec![whole(drag::Slot::MemberEnd(*g), drag::Layer::PopupArea)]);
        }
        ids.push(widget::Id::new(MENU_BODY));
        specs.push(vec![whole(drag::Slot::Body, drag::Layer::Body)]);
        (ids, specs)
    }

    pub(super) fn drag_moved(&mut self, point: iced::Point) {
        // The press point is not known: the first move after it is.
        if std::mem::take(&mut self.drag_fresh)
            && let drag::Drag::Armed { source, .. } = &self.drag
        {
            self.drag = drag::Drag::arm(source.clone(), point);
            return;
        }
        if !self.drag.is_idle() && drag::left_popup(point, &self.drag_targets) {
            self.close_menu();
            self.drag_targets = drag::without_popup(&self.drag_targets);
        }
        let was_active = self.drag.is_active();
        self.drag = std::mem::take(&mut self.drag).moved(point, &self.drag_targets, Instant::now());
        // Only a real drag closes the preview: a click is armed alone.
        if !was_active && self.drag.is_active() {
            self.preview_close();
        }
    }

    /// Re-applies the last point, for a pause over a center to take effect.
    pub(super) fn drag_tick(&mut self) {
        if let drag::Drag::Dragging { at, .. } = &self.drag {
            let at = *at;
            self.drag = std::mem::take(&mut self.drag).moved(at, &self.drag_targets, Instant::now());
        }
    }

    pub(super) fn drag_released(&mut self) -> app::Task<Message> {
        let action = std::mem::take(&mut self.drag).released();
        self.drag_targets.clear();
        if matches!(action, drag::Action::Cancel) {
            return Task::none();
        }
        let dropped = {
            let apps = &self.apps;
            let same = |item: &str, app: &str| same_entry(apps, item, app);
            let find = |id: &str| find_app(apps, id).and_then(|a| apps.iter().position(|x| x.id == a.id));
            drop::resolve_drop(&action, &self.pinned, &self.groups, &self.app_keys(), &find, &same)
        };
        let Some(dropped) = dropped else {
            return Task::none();
        };
        if dropped.pinned != self.pinned {
            self.pinned = dropped.pinned;
            save_pinned(&self.pinned);
        }
        if dropped.groups != self.groups {
            self.groups = dropped.groups;
            save_groups(&self.groups);
        }
        match dropped.open {
            Some(groups::GroupRef::User(id)) => self.edit_name(id),
            _ => Task::none(),
        }
    }

    /// The slot the drag is over, if it is dragging.
    pub(super) fn drag_over(&self) -> Option<drag::Slot> {
        match &self.drag {
            drag::Drag::Dragging { over, .. } => *over,
            _ => None,
        }
    }

    /// The pinned area, tinted when a drop would go to its end.
    pub(super) fn area_class(&self, p: Palette) -> theme::Container<'static> {
        let end = matches!(&self.drag, drag::Drag::Dragging { over: Some(drag::Slot::End), .. });
        theme::Container::custom(move |_| widget::container::Style {
            background: end.then(|| Background::Color(Color { a: 0.12, ..p.accent })),
            border: iced::Border { radius: 8.0.into(), ..Default::default() },
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_pinned_and_the_area_slot() {
        let found: Vec<usize> = (0..20).collect();
        assert_eq!(visible_pinned(&found, false, 16).len(), 16);
        assert_eq!(visible_pinned(&found, true, 16).len(), 20);
        assert_eq!(area_slot(&found, false, 16), drag::Slot::Before(16));
        assert_eq!(area_slot(&found, true, 16), drag::Slot::End);
        assert_eq!(area_slot(&found[..5], false, 16), drag::Slot::End);
        assert_eq!(visible_pinned(&found, false, 8).len(), 8);
        assert_eq!(area_slot(&found, false, 8), drag::Slot::Before(8));
    }

    #[test]
    fn only_the_tile_before_which_the_drop_goes_is_marked() {
        assert!(drop_before(Some(drag::Slot::Before(3)), 3));
        assert!(!drop_before(Some(drag::Slot::Before(3)), 2));
        assert!(!drop_before(Some(drag::Slot::End), 3));
        assert!(!drop_before(None, 3));
    }

    #[test]
    fn pinned_halves_point_to_the_next_visible_and_the_last_to_the_end() {
        let g = groups::GroupRef::Category(5);
        let center_at = |i: usize| match i {
            4 => Some(drag::Slot::Into(g)),
            7 => Some(drag::Slot::OnApp(11)),
            _ => None,
        };
        let specs = pinned_specs(&[2, 4, 7], drag::Slot::End, &center_at, true);
        let right = |k: usize| specs[k].iter().find(|s| s.zone == drag::Zone::Right).map(|s| s.slot);
        let left = |k: usize| specs[k].iter().find(|s| s.zone == drag::Zone::Left).map(|s| s.slot);
        assert_eq!(left(1), Some(drag::Slot::Before(4)));
        assert_eq!(right(0), Some(drag::Slot::Before(4)));
        assert_eq!(right(1), Some(drag::Slot::Before(7)));
        assert_eq!(right(2), Some(drag::Slot::End));
        let centers: Vec<usize> = (0..3).filter(|k| specs[*k].iter().any(|s| s.zone == drag::Zone::Center)).collect();
        assert_eq!(centers, vec![1, 2]);
        assert_eq!(specs[1][2].slot, drag::Slot::Into(g));
        assert_eq!(specs[2][2].slot, drag::Slot::OnApp(11));
        let none = pinned_specs(&[2, 4, 7], drag::Slot::Before(9), &center_at, false);
        assert!(none.iter().all(|l| l.len() == 2));
        assert_eq!(none[2][1].slot, drag::Slot::Before(9));
    }

    #[test]
    fn the_last_member_points_to_the_member_end() {
        let g = groups::GroupRef::Loose;
        let specs = member_specs(g, 3, None);
        assert_eq!(specs.len(), 3);
        assert_eq!(specs[0][1].slot, drag::Slot::Member(g, 1));
        assert_eq!(specs[2][1].slot, drag::Slot::MemberEnd(g));
        assert!(specs.iter().flatten().all(|s| s.layer == drag::Layer::PopupTile));
    }

    #[test]
    fn only_category_and_loose_popups_have_centers() {
        let cat = groups::GroupRef::Category(2);
        let with = member_specs(cat, 2, Some(&[7, 9]));
        assert_eq!(with[1][2].slot, drag::Slot::OnApp(9));
        assert_eq!(with[1][2].layer, drag::Layer::PopupCenter);
        assert_eq!(with[0].len(), 3);
        let user = member_specs(groups::GroupRef::User(1), 2, Some(&[7, 9]));
        assert!(user.iter().all(|l| l.len() == 2));
        assert!(member_specs(cat, 2, None).iter().all(|l| l.len() == 2));
    }

    #[test]
    fn the_unpin_bar_shows_only_for_a_dragged_pinned_with_its_target() {
        let at = iced::Point::new(5.0, 5.0);
        let rect = iced::Rectangle::new(iced::Point::new(1.0, 2.0), iced::Size::new(300.0, 30.0));
        let bar = drag::Target { slot: drag::Slot::Unpin, rect, layer: drag::Layer::Unpin };
        let other = drag::Target { slot: drag::Slot::End, rect, layer: drag::Layer::Area };
        let dragging = |source, over| drag::Drag::Dragging { source, at, over, hover: None };
        let armed = drag::Drag::arm(drag::Source::Pinned(0), at);
        assert_eq!(unpin_bar_at(&armed, &[bar]), None);
        let app = dragging(drag::Source::App("a".into()), None);
        assert_eq!(unpin_bar_at(&app, &[bar]), None);
        let pinned = dragging(drag::Source::Pinned(0), None);
        assert_eq!(unpin_bar_at(&pinned, &[other]), None);
        assert_eq!(unpin_bar_at(&pinned, &[other, bar]), Some((rect, false)));
        let hot = dragging(drag::Source::Pinned(0), Some(drag::Slot::Unpin));
        assert_eq!(unpin_bar_at(&hot, &[bar]), Some((rect, true)));
    }

    #[test]
    fn only_a_drag_past_the_threshold_has_a_source() {
        let armed = drag::Drag::arm(drag::Source::Pinned(1), iced::Point::ORIGIN);
        assert!(dragged(&armed).is_none());
        let moving = armed.moved(iced::Point::new(50.0, 0.0), &[], Instant::now());
        assert_eq!(dragged(&moving), Some(&drag::Source::Pinned(1)));
    }
}
