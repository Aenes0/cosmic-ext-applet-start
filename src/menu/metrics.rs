//! Responsive layout metrics: pure function of the menu size.

use super::MIN_HEIGHT;
use super::open_strip::{self, fit};
use super::resize::{MAX_HEIGHT, MAX_WIDTH, MIN_WIDTH};

const SIDE_MIN: f32 = 24.0;
const SIDE_MAX: f32 = 46.0;
const TILE_MIN: f32 = 72.0;
const TILE_GAP: f32 = 12.0;
const MAX_PINNED_COLS: usize = 8;
const PINNED_ROWS: usize = 2;
const TILE_CHAR_W: f32 = 6.5;
const TILE_CHARS_MIN: f32 = 8.0;
const TILE_CHARS_MAX: f32 = 13.0;
const GRID_TILE: f32 = 84.0;
const MAX_GRID_COLS: usize = 8;
const CARD_MIN: f32 = 200.0;
const CARD_GAP: f32 = 16.0;
const MAX_CARD_COLS: usize = 3;
const CARD_COUNT: usize = 6;
const MAX_CARD_ROWS: usize = 4;
const CARD_TEXT_PAD: f32 = 75.0;
const TITLE_CHAR_W: f32 = 7.0;
const SUB_CHAR_W: f32 = 6.25;
const CATEGORY_MIN: f32 = 140.0;
const MAX_CATEGORY_COLS: usize = 4;
const OPEN_PAD: f32 = 24.0;
const DETAIL_MIN_INNER: f32 = 520.0;
const COMPACT_MAX_INNER: f32 = 440.0;
const POPUP_MAX: f32 = 640.0;
const POPUP_MARGIN: f32 = 48.0;
const POPUP_CELL: f32 = 84.0;
const POPUP_GAP: f32 = 4.0;
const POPUP_PAD: f32 = 8.0;
const MAX_POPUP_COLS: usize = 6;
const CONTEXT_MAX: f32 = 260.0;
const POWER_WIDTH: f32 = 200.0;
const UNINSTALL_MAX: f32 = 420.0;
const SMALL_MARGIN: f32 = 32.0;
const UNINSTALL_V_MARGIN: f32 = 64.0;
const POPUP_SIDE_INSET: f32 = 16.0;
const FOOTER_INSET: f32 = 12.0;
const USER_RESERVED: f32 = 150.0;
const USER_CHAR_W: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub width: f32,
    pub height: f32,
    pub side: f32,
    pub inner: f32,
    pub pinned_cols: usize,
    pub pinned_rows: usize,
    pub pinned_shown: usize,
    pub grid_cols: usize,
    pub card_cols: usize,
    pub card_rows: usize,
    pub card_title_chars: usize,
    pub card_sub_chars: usize,
    pub category_cols: usize,
    pub open_inner: f32,
    pub detail_panel: bool,
    pub compact_headers: bool,
    pub group_popup_width: f32,
    pub group_cols: usize,
    pub group_max_height: f32,
    pub context_width: f32,
    pub power_width: f32,
    pub uninstall_width: f32,
    pub uninstall_max_height: f32,
    pub popup_side: f32,
    pub footer_pad: f32,
    pub user_chars: usize,
    pub tools_width: f32,
    pub tile_chars: usize,
}

impl Metrics {
    pub fn open_capacity(&self, total: usize) -> usize {
        open_strip::capacity_in(total, self.open_inner)
    }
}

fn clamp_input(value: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() { value.clamp(min, max) } else { max }
}

fn chars(card_w: f32, char_w: f32, min: f32, max: f32) -> usize {
    ((card_w - CARD_TEXT_PAD) / char_w).floor().clamp(min, max) as usize
}

/// (cols, rows, title chars, sub chars)
fn cards(inner: f32) -> (usize, usize, usize, usize) {
    let cols = fit(inner, CARD_MIN, CARD_GAP).min(MAX_CARD_COLS);
    let rows = CARD_COUNT.div_ceil(cols).min(MAX_CARD_ROWS);
    let card_w = (inner - CARD_GAP * (cols - 1) as f32) / cols as f32;
    (cols, rows, chars(card_w, TITLE_CHAR_W, 10.0, 25.0), chars(card_w, SUB_CHAR_W, 10.0, 28.0))
}

fn tile_chars(inner: f32, cols: usize) -> usize {
    let tile = (inner - TILE_GAP * (cols - 1) as f32) / cols as f32;
    (tile / TILE_CHAR_W).floor().clamp(TILE_CHARS_MIN, TILE_CHARS_MAX) as usize
}

fn side_for(width: f32) -> f32 {
    let span = MAX_WIDTH - MIN_WIDTH;
    (SIDE_MIN + (width - MIN_WIDTH) * (SIDE_MAX - SIDE_MIN) / span)
        .round()
        .clamp(SIDE_MIN, SIDE_MAX)
}

pub fn metrics(width: f32, height: f32) -> Metrics {
    let width = clamp_input(width, MIN_WIDTH, MAX_WIDTH);
    let height = clamp_input(height, MIN_HEIGHT, MAX_HEIGHT);
    let side = side_for(width);
    let inner = width - 2.0 * side;
    let pinned_cols = fit(inner, TILE_MIN, TILE_GAP).min(MAX_PINNED_COLS);
    let (card_cols, card_rows, card_title_chars, card_sub_chars) = cards(inner);
    let group_popup_width = POPUP_MAX.min(width - POPUP_MARGIN);
    let footer_pad = side - FOOTER_INSET;
    let user_room = (width - 2.0 * footer_pad - USER_RESERVED) / USER_CHAR_W;
    Metrics {
        width,
        height,
        side,
        inner,
        pinned_cols,
        pinned_rows: PINNED_ROWS,
        pinned_shown: pinned_cols * PINNED_ROWS,
        grid_cols: fit(inner, GRID_TILE, TILE_GAP).min(MAX_GRID_COLS),
        card_cols,
        card_rows,
        card_title_chars,
        card_sub_chars,
        category_cols: fit(inner, CATEGORY_MIN, CARD_GAP).min(MAX_CATEGORY_COLS),
        open_inner: inner - OPEN_PAD,
        detail_panel: inner >= DETAIL_MIN_INNER,
        compact_headers: inner < COMPACT_MAX_INNER,
        group_popup_width,
        group_cols: fit(group_popup_width - POPUP_PAD, POPUP_CELL, POPUP_GAP).min(MAX_POPUP_COLS),
        group_max_height: height * 0.6,
        context_width: CONTEXT_MAX.min(width - SMALL_MARGIN),
        power_width: POWER_WIDTH,
        uninstall_width: UNINSTALL_MAX.min(width - SMALL_MARGIN),
        uninstall_max_height: height - UNINSTALL_V_MARGIN,
        popup_side: side - POPUP_SIDE_INSET,
        footer_pad,
        user_chars: user_room.floor().clamp(8.0, 40.0) as usize,
        tools_width: width.max(crate::tools::WIDTH),
        tile_chars: tile_chars(inner, pinned_cols),
    }
}

#[cfg(test)]
mod tests;
