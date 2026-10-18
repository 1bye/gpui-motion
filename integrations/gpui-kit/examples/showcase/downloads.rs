use super::{heading, progress, spring, surface};
use gpui_ce_motion::{Easing, Motion, Target, Transition};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::StyledExt as _;

use gpui_kit::{
    Context, Render, SharedString, Task, Window,
    component::{
        ActiveTheme, Disableable,
        button::{Button, ButtonVariants},
    },
    div,
    prelude::*,
};

use gpui_motion_kit::{LayoutGroup, MotionState, ValueState};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Status {
    Ready,
    Running,
    Paused,
    Complete,
    Cancelled,
}

struct Transfer {
    name: SharedString,
    percent: f64,
    rate: f64,
    status: Status,
    presence: MotionState,
    progress: ValueState,
}

pub(super) struct Downloads {
    transfers: Vec<Transfer>,
    positions: LayoutGroup,
    task: Option<Task<()>>,
    ticking: bool,
}

impl Downloads {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        let mut view = Self {
            transfers: Vec::new(),
            positions: LayoutGroup::default(),
            task: None,
            ticking: false,
        };

        for name in [
            "Brand assets.zip",
            "Research recordings.mov",
            "Design system.fig",
        ] {
            view.add(name);
        }

        view
    }

    fn add(&mut self, name: impl Into<SharedString>) {
        self.transfers.push(Transfer {
            name: name.into(),
            percent: 0.,
            rate: 13. + self.transfers.len() as f64 * 4.,
            status: Status::Ready,
            presence: MotionState::default(),
            progress: ValueState::new(0.),
        });
    }

    fn ensure_timer(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }

        self.ticking = true;
        self.task = Some(cx.spawn(async move |view, cx| {
            let mut previous = cx.background_executor().now();

            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                let now = cx.background_executor().now();
                let delta = now.saturating_duration_since(previous).as_secs_f64();

                previous = now;
                let keep_going = view
                    .update(cx, |this, cx| {
                        for transfer in &mut this.transfers {
                            if transfer.status == Status::Running {
                                transfer.percent =
                                    (transfer.percent + delta * transfer.rate).min(100.);

                                if transfer.percent >= 100. {
                                    transfer.status = Status::Complete;
                                }
                            }
                        }

                        this.ticking = this
                            .transfers
                            .iter()
                            .any(|transfer| transfer.status == Status::Running);
                        cx.notify();

                        this.ticking
                    })
                    .unwrap_or(false);

                if !keep_going {
                    break;
                }
            }
        }));
    }
}

impl Render for Downloads {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div()
            .id("transfer-list")
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();

        for (id, transfer) in self.transfers.iter_mut().enumerate() {
            let frame = transfer.presence.frame(
                Motion::new(Target::default())
                    .initial(Target::default().y(12.).opacity(0.))
                    .exit(Target::default().x(36.).opacity(0.))
                    .present(transfer.status != Status::Cancelled)
                    .transition(spring()),
                window,
                cx,
            );

            if !frame.is_mounted() {
                continue;
            }

            let displayed = transfer
                .progress
                .frame(
                    transfer.percent,
                    Transition::Tween(Duration::from_millis(120), Easing::Linear),
                    window,
                    cx,
                )
                .value() as f32;

            let (label, action) = match transfer.status {
                Status::Ready => ("Ready", "Start"),
                Status::Running => ("Downloading", "Pause"),
                Status::Paused => ("Paused", "Resume"),
                Status::Complete => ("Complete", "Done"),
                Status::Cancelled => ("Cancelled", "Cancelled"),
            };

            let status: SharedString = format!("{label} · {:.0}%", transfer.percent).into();

            let row = surface(cx)
                .p_5()
                .flex()
                .items_center()
                .gap_6()
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .child(div().font_medium().child(transfer.name.clone()))
                                .child(
                                    div()
                                        .id(format!("transfer-status-{id}"))
                                        .test_support()
                                        .aria_label(status.clone())
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(status),
                                ),
                        )
                        .child(progress(
                            format!("transfer-progress-{id}"),
                            transfer.name.clone(),
                            transfer.percent as f32,
                            displayed,
                            cx,
                        )),
                )
                .child(
                    Button::new(format!("transfer-toggle-{id}"))
                        .label(action)
                        .disabled(!frame.is_present() || transfer.status == Status::Complete)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let transfer = &mut this.transfers[id];

                            transfer.status = if transfer.status == Status::Running {
                                Status::Paused
                            } else {
                                Status::Running
                            };
                            this.ensure_timer(cx);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new(format!("transfer-cancel-{id}"))
                        .ghost()
                        .label("Cancel")
                        .disabled(!frame.is_present() || transfer.status == Status::Complete)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.transfers[id].status = Status::Cancelled;
                            cx.notify();
                        })),
                );

            list = list.child(
                self.positions
                    .position(format!("transfer-position-{id}"), frame.apply(row))
                    .transition(spring()),
            );
        }

        div().size_full().flex().flex_col()
            .child(heading("Room for what’s next.", "Simulated transfers. Start, pause, cancel, and add work while the queue rearranges.", cx))
            .child(div().flex().items_center().gap_3().mb_6()
                .child(Button::new("start-downloads").label("Start all").on_click(cx.listener(|this, _, _, cx| {
                    for transfer in &mut this.transfers {
                        if matches!(transfer.status, Status::Ready | Status::Paused) { transfer.status = Status::Running; }
                    }

                    this.ensure_timer(cx); cx.notify();
                })))
                .child(Button::new("add-download").ghost().label("Add download").on_click(cx.listener(|this, _, _, cx| {
                    let name = format!("Project export {}.zip", this.transfers.len() + 1);

                    this.add(name); cx.notify();
                }))))
            .child(list)
    }
}
