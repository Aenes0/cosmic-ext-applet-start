use super::*;

#[test]
fn an_empty_or_blank_title_becomes_untitled() {
    assert_eq!(card_title(""), texts().untitled);
    assert_eq!(card_title("   \t"), texts().untitled);
}

#[test]
fn a_real_title_is_kept() {
    assert_eq!(card_title("Terminal"), "Terminal");
}

#[test]
fn eight_items_in_seven_columns_make_two_rows() {
    let items: Vec<u32> = (0..8).collect();
    let rows = rows_of(&items, 7);
    assert_eq!(rows.iter().map(Vec::len).collect::<Vec<_>>(), vec![7, 1]);
    assert_eq!(rows[1], vec![7]);
}

#[test]
fn zero_columns_count_as_one() {
    let rows = rows_of(&[1, 2, 3], 0);
    assert_eq!(rows, vec![vec![1], vec![2], vec![3]]);
}

#[test]
fn no_items_make_no_rows() {
    assert!(rows_of::<u8>(&[], 3).is_empty());
}

#[test]
fn it_scrolls_only_when_the_content_is_taller() {
    assert!(needs_scroll(500.0, 400.0));
    assert!(!needs_scroll(400.0, 400.0));
    assert!(!needs_scroll(100.0, 400.0));
}
