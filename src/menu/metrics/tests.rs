use super::*;

const WIDTHS: [f32; 5] = [400.0, 480.0, 600.0, 720.0, 874.0];

fn at(w: f32) -> Metrics {
    metrics(w, 920.0)
}

#[test]
fn golden_874x920_matches_today() {
    let m = metrics(874.0, 920.0);
    assert_eq!(m.width, 874.0);
    assert_eq!(m.height, 920.0);
    assert_eq!(m.side, 46.0);
    assert_eq!(m.inner, 782.0);
    assert_eq!((m.pinned_cols, m.pinned_rows, m.pinned_shown), (8, 2, 16));
    assert_eq!(m.grid_cols, 8);
    assert_eq!((m.card_cols, m.card_rows), (3, 2));
    assert_eq!((m.card_title_chars, m.card_sub_chars), (25, 28));
    assert_eq!(m.category_cols, 4);
    assert_eq!(m.open_inner, 758.0);
    assert!(m.detail_panel && !m.compact_headers);
    assert_eq!(m.group_popup_width, 640.0);
    assert_eq!(m.group_cols, 6);
    assert_eq!(m.group_max_height, 552.0);
    assert_eq!(m.context_width, 260.0);
    assert_eq!(m.power_width, 200.0);
    assert_eq!(m.uninstall_width, 420.0);
    assert_eq!(m.uninstall_max_height, 856.0);
    assert_eq!((m.popup_side, m.footer_pad), (30.0, 34.0));
    assert_eq!(m.user_chars, 40);
    assert_eq!(m.tools_width, 874.0);
    assert_eq!(m.tile_chars, 13);

    for t in 0..=20 {
        assert_eq!(m.open_capacity(t), if t <= 10 { 10 } else { 9 });
    }
}

#[test]
fn table_by_width() {
    let side = [24.0, 28.0, 33.0, 39.0, 46.0];
    let inner = [352.0, 424.0, 534.0, 642.0, 782.0];
    let pinned = [4, 5, 6, 7, 8];
    let grid = [3, 4, 5, 6, 8];
    let cc = [1, 2, 2, 3, 3];
    let cr = [4, 3, 3, 2, 2];
    let title = [25, 18, 25, 18, 25];
    let sub = [28, 20, 28, 20, 28];
    let cat = [2, 2, 3, 4, 4];
    let open_inner = [328.0, 400.0, 510.0, 618.0, 758.0];
    let cap_small = [4, 5, 7, 8, 10];
    let cap_big = [3, 4, 6, 7, 9];
    let detail = [false, false, true, true, true];
    let gw = [352.0, 432.0, 552.0, 640.0, 640.0];
    let gc = [3, 4, 6, 6, 6];
    let uw = [368.0, 420.0, 420.0, 420.0, 420.0];
    let ps = [8.0, 12.0, 17.0, 23.0, 30.0];
    let fp = [12.0, 16.0, 21.0, 27.0, 34.0];
    let uc = [28, 37, 40, 40, 40];
    let tile = [12, 11, 12, 12, 13];
    let compact = [true, true, false, false, false];
    let tools = [442.0, 480.0, 600.0, 720.0, 874.0];
    for (i, w) in WIDTHS.iter().enumerate() {
        let m = at(*w);
        assert_eq!(m.side, side[i], "side {w}");
        assert_eq!(m.inner, inner[i], "inner {w}");
        assert_eq!(m.pinned_cols, pinned[i], "pinned {w}");
        assert_eq!(m.pinned_rows, 2);
        assert_eq!(m.pinned_shown, pinned[i] * 2, "shown {w}");
        assert_eq!(m.grid_cols, grid[i], "grid {w}");
        assert_eq!(m.card_cols, cc[i], "card_cols {w}");
        assert_eq!(m.card_rows, cr[i], "card_rows {w}");
        assert_eq!(m.card_title_chars, title[i], "title {w}");
        assert_eq!(m.card_sub_chars, sub[i], "sub {w}");
        assert_eq!(m.category_cols, cat[i], "cat {w}");
        assert_eq!(m.open_inner, open_inner[i], "open_inner {w}");
        assert_eq!(m.open_capacity(0), cap_small[i], "cap0 {w}");
        assert_eq!(m.open_capacity(99), cap_big[i], "cap99 {w}");
        assert_eq!(m.detail_panel, detail[i], "detail {w}");
        assert_eq!(m.compact_headers, compact[i], "compact {w}");
        assert_eq!(m.group_popup_width, gw[i], "gw {w}");
        assert_eq!(m.group_cols, gc[i], "gc {w}");
        assert_eq!(m.context_width, 260.0);
        assert_eq!(m.power_width, 200.0);
        assert_eq!(m.uninstall_width, uw[i], "uw {w}");
        assert_eq!(m.popup_side, ps[i], "ps {w}");
        assert_eq!(m.footer_pad, fp[i], "fp {w}");
        assert_eq!(m.user_chars, uc[i], "uc {w}");
        assert_eq!(m.tools_width, tools[i], "tools {w}");
        assert_eq!(m.tile_chars, tile[i], "tile {w}");
    }
}

#[test]
fn monotonic_and_never_zero() {
    let mut prev = at(400.0);
    for w in 400..=874 {
        let m = at(w as f32);
        let cur = [m.pinned_cols, m.grid_cols, m.card_cols, m.category_cols, m.group_cols, m.open_capacity(0)];
        let old = [prev.pinned_cols, prev.grid_cols, prev.card_cols, prev.category_cols, prev.group_cols, prev.open_capacity(0)];
        for (c, o) in cur.iter().zip(old.iter()) {
            assert!(c >= o, "decresce em {w}");
            assert!(*c >= 1, "zero em {w}");
        }
        let c = m.pinned_cols as f32;
        assert!((m.inner - 12.0 * (c - 1.0)) / c >= 72.0, "tile {w}");
        let c = m.card_cols as f32;
        let card_w = (m.inner - 16.0 * (c - 1.0)) / c;
        assert!(m.card_cols == 1 || card_w >= 200.0, "card {w}");
        let c = m.category_cols as f32;
        assert!((m.inner - 16.0 * (c - 1.0)) / c >= 140.0, "cat {w}");
        let wf = w as f32;
        assert!(m.context_width <= wf && m.power_width <= wf);
        assert!(m.uninstall_width <= wf && m.group_popup_width <= wf);
        assert!(m.tools_width >= wf);
        prev = m;
    }
}

#[test]
fn input_is_clamped() {
    assert_eq!(metrics(100.0, 100.0), metrics(400.0, 300.0));
    assert_eq!(metrics(5000.0, 5000.0), metrics(874.0, 920.0));
    assert_eq!(metrics(f32::NAN, f32::NAN), metrics(874.0, 920.0));
    let m = metrics(874.0, 300.0);
    assert_eq!(m.uninstall_max_height, 236.0);
    assert_eq!(m.group_max_height, 300.0 * 0.6);
}

#[test]
fn tile_chars_fit_the_tile() {
    for w in 400..=874 {
        let m = at(w as f32);
        let c = m.pinned_cols as f32;
        let pinned_tile = (m.inner - 12.0 * (c - 1.0)) / c;
        assert!((8..=13).contains(&m.tile_chars), "range {w}");
        assert!(m.tile_chars as f32 * 6.5 <= pinned_tile, "fit {w}");
    }
}
