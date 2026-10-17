//! Covers: real CE measurement, nested offsets, native hitboxes, size changes and scrolling.
//! Real: CE TestPlatform, Taffy, prepaint/paint dispatch and deterministic executor clock.
//! Doubles: window/display/GPU supplied by CE TestPlatform. Requires: native-tests feature.
use gpui::{
    AppContext, Context, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext, Window,
    div, point, prelude::*, px, size,
};
use gpui_ce_motion::{Easing, LayoutGroup, Transition};
use std::{cell::Cell, rc::Rc, time::Duration};
struct Fixture {
    group: LayoutGroup,
    gap: f32,
    child_gap: f32,
    width: f32,
    nested: bool,
    enabled: bool,
    reduced: bool,
    scroll: gpui::ScrollHandle,
    clicks: Rc<Cell<usize>>,
}
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        cx.set_reduce_motion(self.reduced);
        let clicks = self.clicks.clone();
        let item = self
            .group
            .position(
                "item-position",
                div()
                    .id("item")
                    .debug_selector(|| "item".into())
                    .w(px(self.width))
                    .h(px(20.))
                    .flex_shrink_0()
                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                        clicks.set(clicks.get() + 1)
                    }),
            )
            .transition(Transition::Tween(Duration::from_secs(1), Easing::Linear))
            .enabled(self.enabled);
        let row = div()
            .id("row")
            .flex()
            .w(px(500.))
            .h(px(20.))
            .flex_shrink_0()
            .child(div().w(px(self.child_gap)).flex_shrink_0())
            .child(item);
        let parent = div()
            .id("parent")
            .debug_selector(|| "parent".into())
            .w(px(500.))
            .h(px(400.))
            .flex_shrink_0()
            .child(row);
        let content = div()
            .flex()
            .w(px(1000.))
            .h(px(400.))
            .child(div().w(px(self.gap)).flex_shrink_0())
            .child(if self.nested {
                self.group
                    .position("parent-position", parent)
                    .transition(Transition::Tween(Duration::from_secs(1), Easing::Linear))
                    .into_any_element()
            } else {
                parent.into_any_element()
            });
        div()
            .id("scroller")
            .w(px(300.))
            .h(px(200.))
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .child(content)
    }
}
fn setup(cx: &mut TestAppContext, nested: bool) -> (gpui::WindowHandle<Fixture>, Rc<Cell<usize>>) {
    let clicks = Rc::new(Cell::new(0));
    let copy = clicks.clone();
    let window = cx.open_window(size(px(600.), px(300.)), move |_, _| Fixture {
        group: LayoutGroup::default(),
        gap: 0.,
        child_gap: 0.,
        width: 20.,
        nested,
        enabled: true,
        reduced: false,
        scroll: gpui::ScrollHandle::new(),
        clicks: copy,
    });
    draw(cx, window);
    (window, clicks)
}
fn draw(cx: &mut TestAppContext, window: gpui::WindowHandle<Fixture>) {
    cx.update_window(window.into(), |_, window, cx| {
        window.simulate_next_frame(cx);
        window.draw(cx).clear(cx);
    })
    .unwrap();
}
fn bounds(
    cx: &TestAppContext,
    window: gpui::WindowHandle<Fixture>,
    selector: &'static str,
) -> gpui::Bounds<gpui::Pixels> {
    VisualTestContext::from_window(window.into(), cx)
        .debug_bounds(selector)
        .expect("painted bounds")
}
#[gpui::test]
fn measured_layout_changes_translate_hitboxes_and_use_final_size(cx: &mut TestAppContext) {
    let (window, clicks) = setup(cx, false);
    assert_eq!(bounds(cx, window, "item").origin, point(px(0.), px(0.)));
    window
        .update(cx, |view, _, cx| {
            view.gap = 100.;
            view.width = 40.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    let first = bounds(cx, window, "item");
    assert_eq!(first.origin, point(px(0.), px(0.)));
    assert_eq!(first.size, size(px(40.), px(20.)));
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_mouse_down(
        point(px(110.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(clicks.get(), 0);
    visual.simulate_mouse_down(
        point(px(10.), px(10.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    assert_eq!(clicks.get(), 1);
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx, window);
    assert_eq!(bounds(cx, window, "item").origin, point(px(50.), px(0.)));
    window
        .update(cx, |view, _, cx| {
            view.gap = 200.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "item").origin, point(px(50.), px(0.)));
    cx.executor().advance_clock(Duration::from_secs(2));
    draw(cx, window);
    assert_eq!(bounds(cx, window, "item").origin, point(px(200.), px(0.)));
    assert_eq!(
        cx.update_window(window.into(), |_, window, cx| window
            .simulate_next_frame(cx))
            .unwrap(),
        0
    );
}
#[gpui::test]
fn nested_wrappers_inherit_parent_translation_without_doubling_it(cx: &mut TestAppContext) {
    let (window, _) = setup(cx, true);
    window
        .update(cx, |view, _, cx| {
            view.gap = 100.;
            view.child_gap = 40.;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "parent").origin.x, px(0.));
    assert_eq!(bounds(cx, window, "item").origin.x, px(0.));
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx, window);
    assert_eq!(bounds(cx, window, "parent").origin.x, px(50.));
    assert_eq!(bounds(cx, window, "item").origin.x, px(70.));
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(cx, window);
    assert_eq!(bounds(cx, window, "parent").origin.x, px(100.));
    assert_eq!(bounds(cx, window, "item").origin.x, px(140.));
}
#[gpui::test]
fn scrolling_moves_presentation_directly_without_starting_a_layout_animation(
    cx: &mut TestAppContext,
) {
    let (window, _) = setup(cx, false);
    window
        .update(cx, |view, _, cx| {
            view.child_gap = 100.;
            view.reduced = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    window
        .update(cx, |view, _, cx| {
            view.reduced = false;
            view.scroll.set_offset(point(px(-40.), px(-20.)));
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "item").origin, point(px(60.), px(-20.)));
    assert_eq!(
        cx.update_window(window.into(), |_, window, cx| window
            .simulate_next_frame(cx))
            .unwrap(),
        0
    );
}
#[gpui::test]
fn disabled_or_reduced_motion_follows_layout_immediately(cx: &mut TestAppContext) {
    let (window, _) = setup(cx, false);
    window
        .update(cx, |view, _, cx| {
            view.gap = 100.;
            view.enabled = false;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "item").origin.x, px(100.));
    window
        .update(cx, |view, _, cx| {
            view.gap = 200.;
            view.enabled = true;
            view.reduced = true;
            cx.notify();
        })
        .unwrap();
    draw(cx, window);
    assert_eq!(bounds(cx, window, "item").origin.x, px(200.));
}

struct Flow {
    group: LayoutGroup,
    narrow: bool,
    reverse: bool,
    extra: bool,
}
impl Render for Flow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let count: usize = if self.extra { 4 } else { 3 };
        let mut row = div()
            .flex()
            .flex_wrap()
            .content_start()
            .gap(px(10.))
            .w(px(if self.narrow { 100. } else { 150. }));
        for index in 0..count {
            let id = if self.reverse {
                count - 1 - index
            } else {
                index
            };
            row = row.child(
                self.group
                    .position(
                        ("position", id),
                        div()
                            .id(("card", id))
                            .debug_selector(move || format!("card-{id}"))
                            .w(px(40.))
                            .h(px(20.))
                            .flex_shrink_0(),
                    )
                    .transition(Transition::Tween(Duration::from_secs(1), Easing::Linear)),
            );
        }
        row
    }
}
#[gpui::test]
fn wrapping_reorder_and_insertion_follow_measured_layout_with_stable_keys(cx: &mut TestAppContext) {
    let window = cx.open_window(size(px(300.), px(200.)), |_, _| Flow {
        group: LayoutGroup::default(),
        narrow: false,
        reverse: false,
        extra: false,
    });
    let draw = |cx: &mut TestAppContext| {
        cx.update_window(window.into(), |_, w, cx| {
            w.simulate_next_frame(cx);
            w.draw(cx).clear(cx);
        })
        .unwrap();
    };
    let origin = |cx: &TestAppContext, key| {
        VisualTestContext::from_window(window.into(), cx)
            .debug_bounds(key)
            .unwrap()
            .origin
    };
    draw(cx);
    assert_eq!(origin(cx, "card-2"), point(px(100.), px(0.)));
    window
        .update(cx, |view, _, cx| {
            view.narrow = true;
            cx.notify();
        })
        .unwrap();
    draw(cx);
    assert_eq!(origin(cx, "card-2"), point(px(100.), px(0.)));
    cx.executor().advance_clock(Duration::from_millis(500));
    draw(cx);
    assert_eq!(origin(cx, "card-2"), point(px(50.), px(15.)));
    window
        .update(cx, |view, _, cx| {
            view.reverse = true;
            view.extra = true;
            cx.notify();
        })
        .unwrap();
    draw(cx);
    assert_eq!(origin(cx, "card-2"), point(px(50.), px(15.)));
    // New keys mount at their destination while retained siblings interrupt.
    assert_eq!(origin(cx, "card-3"), point(px(0.), px(0.)));
    cx.executor().advance_clock(Duration::from_secs(2));
    draw(cx);
    assert_eq!(origin(cx, "card-2"), point(px(50.), px(0.)));
    assert_eq!(origin(cx, "card-0"), point(px(50.), px(30.)));
}
