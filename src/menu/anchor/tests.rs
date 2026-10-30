use super::*;
use cosmic::iced::advanced::widget::operation::scrollable::{AbsoluteOffset, RelativeOffset};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

const MENU: Rectangle = Rectangle { x: 100.0, y: 100.0, width: 600.0, height: 500.0 };
const POPUP: Size = Size { width: 200.0, height: 150.0 };

#[test]
fn middle_goes_below_left_aligned() {
    let p = popup_origin(rect(300.0, 200.0, 80.0, 40.0), POPUP, MENU);
    assert_eq!(p, Point::new(300.0, 244.0));
}

#[test]
fn clamps_right_edge() {
    let p = popup_origin(rect(650.0, 200.0, 40.0, 40.0), POPUP, MENU);
    assert_eq!(p, Point::new(500.0, 244.0));
}

#[test]
fn clamps_left_edge() {
    let p = popup_origin(rect(10.0, 200.0, 40.0, 40.0), POPUP, MENU);
    assert_eq!(p, Point::new(100.0, 244.0));
}

#[test]
fn flips_up_without_room_below() {
    let p = popup_origin(rect(300.0, 500.0, 80.0, 40.0), POPUP, MENU);
    assert_eq!(p, Point::new(300.0, 346.0));
}

#[test]
fn stays_inside_menu_without_room_anywhere() {
    let menu = rect(100.0, 100.0, 600.0, 200.0);
    let p = popup_origin(rect(300.0, 180.0, 80.0, 40.0), POPUP, menu);
    assert_eq!(p, Point::new(300.0, 150.0));
    assert!(p.y >= menu.y && p.y + POPUP.height <= menu.y + menu.height);
}

#[test]
fn popup_larger_than_menu() {
    let menu = rect(100.0, 100.0, 150.0, 100.0);
    let p = popup_origin(rect(120.0, 120.0, 40.0, 20.0), POPUP, menu);
    assert_eq!(p, Point::new(100.0, 100.0));
}

#[test]
fn height_estimate() {
    assert_eq!(popup_height(3, false), 120.0);
    assert_eq!(popup_height(3, true), 162.0);
    assert_eq!(popup_height(0, false), 8.0);
    assert_eq!(popup_height(1, false), 44.0);
}

#[test]
fn height_includes_item_spacing() {
    assert_eq!(popup_height(7, false), 7.0 * 36.0 + 8.0 + 12.0);
}

struct Stub;
impl widget::operation::Scrollable for Stub {
    fn snap_to(&mut self, _: RelativeOffset<Option<f32>>) {}
    fn scroll_to(&mut self, _: AbsoluteOffset<Option<f32>>) {}
    fn scroll_by(&mut self, _: AbsoluteOffset, _: Rectangle, _: Rectangle) {}
}

fn ids(n: usize) -> Vec<widget::Id> {
    (0..n).map(|i| widget::Id::new(format!("t{i}"))).collect()
}

fn scroll(op: &mut dyn Operation<Vec<Option<Rectangle>>>, bounds: Rectangle, ty: f32) {
    op.scrollable(None, bounds, bounds, Vector::new(0.0, ty), &mut Stub);
}

fn result(op: &VisibleBounds) -> Vec<Option<Rectangle>> {
    match op.finish() {
        Outcome::Some(v) => v,
        _ => panic!("expected Outcome::Some"),
    }
}

#[test]
fn many_ids_missing_one_keeps_order() {
    let t = ids(3);
    let mut op = VisibleBounds::new(t.clone());
    op.container(Some(&t[2]), rect(0.0, 30.0, 10.0, 10.0));
    op.container(Some(&t[0]), rect(0.0, 0.0, 10.0, 10.0));
    let r = result(&op);
    assert_eq!(r, vec![Some(rect(0.0, 0.0, 10.0, 10.0)), None, Some(rect(0.0, 30.0, 10.0, 10.0))]);
}

#[test]
fn scrolled_container_is_shifted_and_clipped() {
    let t = ids(2);
    let mut op = VisibleBounds::new(t.clone());
    op.traverse(&mut |op| {
        scroll(op, rect(0.0, 0.0, 100.0, 100.0), 100.0);
        op.traverse(&mut |op| {
            op.container(Some(&t[0]), rect(0.0, 150.0, 50.0, 100.0));
            op.container(Some(&t[1]), rect(0.0, 500.0, 50.0, 10.0));
        });
    });
    assert_eq!(result(&op), vec![Some(rect(0.0, 50.0, 50.0, 50.0)), None]);
}

#[test]
fn scroll_scope_ends_with_scrollable_children() {
    let t = ids(2);
    let mut op = VisibleBounds::new(t.clone());
    op.container(None, rect(0.0, 0.0, 100.0, 100.0));
    op.traverse(&mut |op| {
        scroll(op, rect(0.0, 0.0, 100.0, 100.0), 100.0);
        op.traverse(&mut |op| op.container(Some(&t[0]), rect(0.0, 120.0, 10.0, 10.0)));
    });
    op.container(Some(&t[1]), rect(0.0, 400.0, 10.0, 10.0));
    assert_eq!(
        result(&op),
        vec![Some(rect(0.0, 20.0, 10.0, 10.0)), Some(rect(0.0, 400.0, 10.0, 10.0))]
    );
}

#[test]
fn nested_scrollables_sum_translations() {
    let t = ids(1);
    let mut op = VisibleBounds::new(t.clone());
    op.traverse(&mut |op| {
        scroll(op, rect(0.0, 0.0, 200.0, 200.0), 10.0);
        op.traverse(&mut |op| {
            scroll(op, rect(0.0, 0.0, 200.0, 200.0), 20.0);
            op.traverse(&mut |op| op.container(Some(&t[0]), rect(0.0, 100.0, 10.0, 10.0)));
        });
    });
    assert_eq!(result(&op), vec![Some(rect(0.0, 70.0, 10.0, 10.0))]);
}

#[test]
fn repeated_id_keeps_first() {
    let id = widget::Id::new("dup");
    let mut op = VisibleBounds::new(vec![id.clone(), id.clone()]);
    op.container(Some(&id), rect(1.0, 1.0, 5.0, 5.0));
    op.container(Some(&id), rect(9.0, 9.0, 5.0, 5.0));
    let first = Some(rect(1.0, 1.0, 5.0, 5.0));
    assert_eq!(result(&op), vec![first, first]);
}


// documenta: a Onda B garante popup.width <= menu.width clampando as larguras
#[test]
fn popup_wider_than_menu_sticks_to_menu_left() {
    let p = popup_origin(rect(400.0, 200.0, 80.0, 40.0), Size::new(700.0, 150.0), MENU);
    assert_eq!(p.x, 100.0);
    assert_eq!(p.y, 244.0);
}
