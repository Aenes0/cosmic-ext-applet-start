use super::*;

#[test]
fn mini_grid_counts() {
    assert_eq!(mini_grid(0), (2, 0));
    assert_eq!(mini_grid(1), (2, 1));
    assert_eq!(mini_grid(4), (2, 4));
    assert_eq!(mini_grid(5), (3, 5));
    assert_eq!(mini_grid(9), (3, 9));
    assert_eq!(mini_grid(12), (3, 9));
}

#[test]
fn mini_layout_fits_and_grows() {
    // (side, cols, old icon): Pinned keep growing past the old; "All" is capped now.
    let old = [(56.0, 2, 19u16), (56.0, 3, 12), (120.0, 2, 39), (120.0, 3, 21)];
    for (side, cols, before) in old {
        let n = if cols == 2 { 4 } else { 9 };
        let l = mini_layout(side, n);
        assert_eq!(l.cols, cols);
        if side < 100.0 {
            assert!(l.icon > before, "{side} {cols}: {} <= {before}", l.icon);
        } else {
            assert!(l.icon <= icon_max());
        }
        let used = cols as f32 * f32::from(l.icon) + (cols as f32 - 1.0) * l.gap + 2.0 * l.pad;
        assert!(used <= side, "{used} > {side}");
        assert!((3.0..=5.0).contains(&l.pad));
        assert!(l.gap == 2.0 || l.gap == 3.0);
    }
}

#[test]
fn mini_icon_never_exceeds_interface_icon() {
    for side in (40..=140).step_by(4) {
        for count in 1..=9 {
            let l = mini_layout(side as f32, count);
            assert!(l.icon <= icon_max(), "{side} {count}: {}", l.icon);
        }
    }
}

#[test]
fn capped_side_hugs_the_max_icon() {
    let s = capped_side(FOLDER_BOX);
    assert!(s < FOLDER_BOX);
    let l = mini_layout(s, 4);
    assert_eq!(l.icon, icon_max());
    assert!(2.0 * f32::from(l.icon) + l.gap + 2.0 * l.pad <= s);
    assert_eq!(capped_side(PIN_BOX), PIN_BOX);
}

#[test]
fn grid_pads_to_full_rows() {
    let pads = [(1, 5), (5, 1), (6, 0), (7, 5), (12, 0), (13, 5), (0, 6)];
    for (n, want) in pads {
        assert_eq!(pad_cells(n, 6), want, "pad {n}");
    }
    let rows = [(0, 1), (1, 1), (6, 1), (7, 2), (12, 2), (13, 3)];
    for (n, want) in rows {
        assert_eq!(grid_rows(n, 6), want, "rows {n}");
    }
}

#[test]
fn unpin_background_kinds() {
    assert!(matches!(unpin_background(true), Background::Gradient(_)));
    assert!(matches!(unpin_background(false), Background::Color(_)));
}

#[test]
fn keys_and_ids() {
    let games = GroupRef::Category(0);
    assert_eq!(member_key(GroupRef::Loose, 0), "grp:none:0");
    assert_ne!(member_key(games, 1), member_key(games, 2));
    assert_ne!(member_key(games, 1), member_key(GroupRef::Loose, 1));
    assert_eq!(folder_key(GroupRef::Loose), "fold:none");
    assert_ne!(folder_key(games), folder_key(GroupRef::Loose));
    assert_eq!(member_id(games, 3), widget::Id::new(member_key(games, 3)));
    assert_eq!(folder_id(games), widget::Id::new(folder_key(games)));
}

#[test]
fn editable_only_for_user_groups() {
    assert_eq!(editable(GroupRef::User(7)), Some(7));
    assert_eq!(editable(GroupRef::Category(0)), None);
    assert_eq!(editable(GroupRef::Loose), None);
}
