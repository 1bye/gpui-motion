//! Covers the production Kit showcase through real controls and input dispatch.
//! Real: Kit components, domain models, motion, layout, and pointer/keyboard input.
//! Doubles: GPUI TestPlatform display/GPU. Requires: demo,native-tests.

use super::*;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    Bounds, Point, TestAppContext, WindowBounds, WindowHandle, WindowOptions, base::Root, px, size,
};

fn open(cx: &mut TestAppContext, scene: &str) -> WindowHandle<Root> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        bind_keys(cx);
        cx.set_reduce_motion(true);
        let (window, _) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Point::default(),
                    size(px(1180.), px(780.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Showcase::new(scene, window, cx)),
        )
        .unwrap();
        window.downcast::<Root>().unwrap()
    })
}

fn draw(cx: &mut TestAppContext, handle: WindowHandle<Root>) {
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn advance(cx: &mut TestAppContext, handle: WindowHandle<Root>, milliseconds: u64) {
    cx.executor()
        .advance_clock(Duration::from_millis(milliseconds));
    cx.run_until_parked();
    draw(cx, handle);
}

#[gpui_kit::test]
fn inbox_archives_and_undoes_through_real_controls(cx: &mut TestAppContext) {
    let handle = open(cx, "inbox");
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("archive-selected", cx);
        assert_eq!(window.find("inbox-count").label(), Some("6 messages"));
        window.click("message-select-0", cx);
        window.click("message-select-1", cx);
        window.click("archive-selected", cx);
        assert_eq!(window.find("inbox-count").label(), Some("4 messages"));
        assert!(window.try_find("message-select-0").is_none());
        window.click("undo-archive", cx);
        assert_eq!(window.find("inbox-count").label(), Some("6 messages"));
        assert!(window.find("message-select-0").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn asset_filter_and_inspector_respond_to_real_input(cx: &mut TestAppContext) {
    let handle = open(cx, "assets");
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("asset-search", cx);
        window.input("orbit", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("asset-search").value(), Some("orbit"));
        assert!(window.find("asset-3").visible());
        assert!(window.try_find("asset-0").is_none());
        window.click("asset-3", cx);
        assert!(window.find("asset-inspector").bounds().size.width > px(100.));
        window.click("close-inspector", cx);
        assert_eq!(window.find("asset-inspector").bounds().size.width, px(0.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn downloads_pause_cancel_and_finish(cx: &mut TestAppContext) {
    let handle = open(cx, "downloads");
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("start-downloads", cx);
    })
    .unwrap();
    advance(cx, handle, 1000);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("transfer-toggle-0", cx);
        assert!(
            window
                .find("transfer-status-0")
                .label()
                .unwrap()
                .starts_with("Paused")
        );
        window.click("transfer-cancel-1", cx);
        assert!(window.try_find("transfer-toggle-1").is_none());
    })
    .unwrap();
    advance(cx, handle, 10000);
    cx.update_window(handle.into(), |_, window, _| {
        assert!(
            window
                .find("transfer-status-0")
                .label()
                .unwrap()
                .starts_with("Paused")
        );
        assert_eq!(
            window.find("transfer-status-2").label(),
            Some("Complete · 100%")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn command_palette_search_and_enter_open_the_document(cx: &mut TestAppContext) {
    let handle = open(cx, "palette");
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("focus-command", cx);
        window.input("release", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("opened-document").label(),
            Some("Open document: Release checklist")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn planner_moves_cards_with_native_drag_and_keyboard_controls(cx: &mut TestAppContext) {
    let handle = open(cx, "kanban");
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        let from = window.find("planner-card-0").bounds().center();
        let column = window.find("planner-column-2").bounds();
        let to = column.origin + gpui_kit::point(column.size.width / 2., px(230.));
        window.drag(from, to, cx);
        assert!(window.find("planner-card-0").bounds().origin.x > column.origin.x);
        window.click("move-task-0", cx);
        assert!(window.find("planner-card-0").bounds().origin.x < column.origin.x);
    })
    .unwrap();
}

#[gpui_kit::test]
fn automation_keeps_completed_steps_when_retrying(cx: &mut TestAppContext) {
    let handle = open(cx, "automation");
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("run-pipeline", cx);
    })
    .unwrap();
    advance(cx, handle, 4000);
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("step-status-0").label(), Some("Complete"));
        assert_eq!(window.find("step-status-2").label(), Some("Failed"));
        window.click("retry-publish", cx);
        assert_eq!(window.find("step-status-0").label(), Some("Complete"));
    })
    .unwrap();
    advance(cx, handle, 4000);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.find("pipeline-status").label(),
            Some("Release complete")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn inbox_undo_interrupts_an_exit_without_jumping(cx: &mut TestAppContext) {
    let handle = open(cx, "inbox");
    cx.update(|cx| cx.set_reduce_motion(false));

    let start = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let start = window.find("message-0").bounds().origin;

            window.click("message-select-0", cx);
            window.press("cmd-backspace", cx);
            assert_eq!(window.find("inbox-count").label(), Some("5 messages"));

            start
        })
        .unwrap();

    advance(cx, handle, 80);
    cx.update_window(handle.into(), |_, window, cx| {
        let departing = window.find("message-0").bounds().origin;
        let neighbor = window.find("message-1").bounds().origin;

        assert!(departing.x > start.x + px(1.));
        window.press("cmd-z", cx);
        assert_eq!(window.find("inbox-count").label(), Some("6 messages"));
        assert!((window.find("message-0").bounds().origin.x - departing.x).abs() < px(1.));
        assert!((window.find("message-1").bounds().origin.y - neighbor.y).abs() < px(1.));
    })
    .unwrap();

    advance(cx, handle, 1000);
    cx.update_window(handle.into(), |_, window, _| {
        assert!((window.find("message-0").bounds().origin.x - start.x).abs() < px(1.));
        assert!(window.find("message-select-0").visible());
    })
    .unwrap();
}
