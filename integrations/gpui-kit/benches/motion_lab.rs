//! Real gallery CPU preparation on TestPlatform; excludes GPU presentation.
#[allow(dead_code)]
#[path = "../examples/motion_lab.rs"]
mod demo;

use demo::{button, configure_drag, motion, native_color, native_svg, ui};
use std::time::{Duration, Instant};
use ui::AppContext;

fn main() {
    for scene in [0, 1, 2, 5, 6, 7, 8] {
        let mut batches = Vec::new();
        let mut frames = Vec::new();

        for _ in 0..5 {
            let mut cx = ui::TestAppContext::single();

            cx.update(demo::init_app);

            let window = cx.open_window(ui::size(ui::px(960.), ui::px(640.)), |_, _| {
                let mut lab = demo::view::Lab::new();

                lab.select_scene(scene);

                lab
            });

            cx.update_window(window.into(), |_, w, _| w.activate_window())
                .unwrap();
            cx.run_until_parked();
            let (group, viewport, scroll) = window
                .update(&mut cx, |lab, _, _| {
                    (
                        lab.grid.clone(),
                        lab.grid_viewport.clone(),
                        lab.grid_scroll.clone(),
                    )
                })
                .unwrap();

            let mut total = Duration::ZERO;

            for frame in 0..264usize {
                cx.executor().advance_clock(Duration::from_micros(8333));
                let started = Instant::now();

                if scene == 5 {
                    let phase = frame % 48;

                    if phase == 0 {
                        scroll.set_offset(ui::point(ui::px(0.), ui::px(0.)));
                    }

                    if let Some(bounds) = viewport.rect() {
                        let origin = bounds.origin();

                        let point = if phase <= 1 {
                            [origin[0] + 20., origin[1] + 20.]
                        } else {
                            [
                                origin[0] + 20. + (phase % 6) as f64 * 48.,
                                origin[1] + 20. + phase as f64 * 6.,
                            ]
                        };

                        let key = group.order()[0];

                        cx.update_window(window.into(), |_, w, cx| {
                            if phase == 1 {
                                assert!(group.start_drag(&key, point, w, cx));
                            } else if (2..47).contains(&phase) {
                                // Multiple events per frame exercise stable slot mapping.
                                for delta in [0., 0.5, 1.] {
                                    w.dispatch_event(
                                        ui::PlatformInput::MouseMove(ui::MouseMoveEvent {
                                            position: ui::point(
                                                ui::px((point[0] + delta) as f32),
                                                ui::px(point[1] as f32),
                                            ),
                                            pressed_button: Some(ui::MouseButton::Left),
                                            modifiers: Default::default(),
                                        }),
                                        cx,
                                    );
                                }
                            } else if phase == 47 {
                                w.dispatch_event(
                                    ui::PlatformInput::MouseUp(ui::MouseUpEvent {
                                        position: ui::point(
                                            ui::px(point[0] as f32),
                                            ui::px(point[1] as f32),
                                        ),
                                        button: ui::MouseButton::Left,
                                        modifiers: Default::default(),
                                        click_count: 1,
                                    }),
                                    cx,
                                );
                            }
                        })
                        .unwrap();
                    }
                }

                window
                    .update(&mut cx, |view, w, cx| {
                        if scene != 5 && frame.is_multiple_of(24) {
                            view.play(w, cx);
                        }

                        cx.notify();
                    })
                    .unwrap();
                cx.update_window(window.into(), |_, w, cx| {
                    w.simulate_next_frame(cx);
                    w.draw(cx).clear(cx);
                })
                .unwrap();
                cx.run_until_parked();

                if frame >= 24 {
                    let elapsed = started.elapsed();

                    total += elapsed;
                    frames.push(elapsed.as_secs_f64() * 1000.);
                }
            }

            batches.push(total.as_secs_f64() * 1000. / 240.);
        }

        batches.sort_by(f64::total_cmp);
        frames.sort_by(f64::total_cmp);
        println!(
            "lab scene {:02}: median batch mean {:.3} ms/frame; frame P95 {:.3} ms; 5 x 240 frames",
            scene + 1,
            batches[2],
            frames[frames.len() * 95 / 100]
        );
    }
}
