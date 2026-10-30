use super::*;
use cosmic::widget::image::Handle;
use std::collections::HashSet;

#[test]
fn captures_run_only_while_menu_and_preview_show() {
    assert!(capture_gate(true, true));
    assert!(!capture_gate(true, false));
    assert!(!capture_gate(false, true));
    assert!(!capture_gate(false, false));
}

#[test]
fn thumb_message_none_is_thumb_none() {
    match thumb_message("a".to_string(), None) {
        Message::Thumb(id, None) => assert_eq!(id, "a"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn thumb_message_image_is_thumb_some() {
    let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]));
    match thumb_message("b".to_string(), Some(img)) {
        Message::Thumb(id, Some(_)) => assert_eq!(id, "b"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn window_from_minimized() {
    let states = HashSet::from([zcosmic_toplevel_handle_v1::State::Minimized]);
    let w = window_from("i", "t", "app", &states);
    assert!(w.minimized);
    assert!(!w.active);
    assert_eq!((w.id.as_str(), w.title.as_str(), w.app_id.as_str()), ("i", "t", "app"));
}

#[test]
fn window_from_activated() {
    let states = HashSet::from([zcosmic_toplevel_handle_v1::State::Activated]);
    let w = window_from("i", "t", "app", &states);
    assert!(w.active);
    assert!(!w.minimized);
}

#[test]
fn window_from_empty_is_neither() {
    let w = window_from("i", "t", "app", &HashSet::new());
    assert!(!w.active);
    assert!(!w.minimized);
}

fn sample_list() -> Vec<Window> {
    vec![window_from("i", "t", "app", &HashSet::new())]
}

#[test]
fn report_skips_an_identical_list() {
    let a = sample_list();
    assert!(!should_send(Some(&a), &a.clone(), false));
}

#[test]
fn a_new_title_is_sent() {
    let a = sample_list();
    let mut b = a.clone();
    b[0].title = "other".to_string();
    assert!(should_send(Some(&a), &b, false));
}

#[test]
fn the_first_list_is_sent() {
    assert!(should_send(None, &sample_list(), false));
}

#[test]
fn refresh_always_sends() {
    let a = sample_list();
    assert!(should_send(Some(&a), &a.clone(), true));
}

fn handle() -> Handle {
    Handle::from_rgba(1, 1, vec![0, 0, 0, 255])
}

#[test]
fn store_thumb_inserts_replaces_and_removes() {
    let mut thumbs = HashMap::new();
    store_thumb(&mut thumbs, "a".to_string(), Some(handle()));
    assert_eq!(thumbs.len(), 1);
    let first = thumbs["a"].id();
    store_thumb(&mut thumbs, "a".to_string(), Some(handle()));
    assert_eq!(thumbs.len(), 1);
    assert_ne!(thumbs["a"].id(), first);
    store_thumb(&mut thumbs, "a".to_string(), None);
    assert!(thumbs.is_empty());
}

#[test]
fn store_thumb_none_of_absent_id_is_harmless() {
    let mut thumbs: HashMap<String, Handle> = HashMap::new();
    store_thumb(&mut thumbs, "nope".to_string(), None);
    assert!(thumbs.is_empty());
}
