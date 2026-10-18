use super::{heading, progress, spring, surface};
use gpui_ce_motion::{Color, Motion, PathCommand, PathShape, StrokeReveal, Target};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::StyledExt as _;

use gpui_kit::{
    Context, Render, Task, Window,
    component::{
        ActiveTheme, Disableable,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
    },
    div,
    prelude::*,
    rems,
};

use gpui_motion_kit::{MotionState, ValueState, VectorState};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Status {
    Waiting,
    Running,
    Done,
    Failed,
    Paused,
}

struct Step {
    title: &'static str,
    status: Status,
    progress: f64,
    value: ValueState,
    check: VectorState,
}

pub(super) struct Runner {
    steps: Vec<Step>,
    connectors: Vec<VectorState>,
    fail_publish: bool,
    task: Option<Task<()>>,
    ticking: bool,
    error_motion: MotionState,
}

fn line() -> PathShape {
    PathShape::new([PathCommand::Move([0., 12.]), PathCommand::Line([56., 12.])])
        .expect("valid line")
}

fn check() -> PathShape {
    PathShape::new([
        PathCommand::Move([3., 12.]),
        PathCommand::Line([9., 18.]),
        PathCommand::Line([21., 5.]),
    ])
    .expect("valid check")
}

impl Runner {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            steps: ["Fetch source", "Build", "Publish", "Notify"]
                .into_iter()
                .map(|title| Step {
                    title,
                    status: Status::Waiting,
                    progress: 0.,
                    value: ValueState::new(0.),
                    check: VectorState::new(check()),
                })
                .collect(),
            connectors: (0..3).map(|_| VectorState::new(line())).collect(),
            fail_publish: true,
            task: None,
            ticking: false,
            error_motion: MotionState::default(),
        }
    }

    fn advance(&mut self, seconds: f64) {
        let mut remaining = seconds * 100. / 1.1;

        while let Some(index) = self
            .steps
            .iter()
            .position(|step| step.status == Status::Running)
        {
            let needed = 100. - self.steps[index].progress;
            let amount = remaining.min(needed);

            self.steps[index].progress += amount;
            remaining -= amount;

            if self.steps[index].progress < 100. {
                break;
            }

            if index == 2 && self.fail_publish {
                self.steps[index].status = Status::Failed;
                break;
            }

            self.steps[index].status = Status::Done;

            if let Some(next) = self.steps.get_mut(index + 1) {
                next.status = Status::Running;
            }

            if remaining <= 0. {
                break;
            }
        }
    }

    fn run(&mut self, retry: bool, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }

        if retry {
            self.fail_publish = false;

            if let Some(step) = self
                .steps
                .iter_mut()
                .find(|step| step.status == Status::Failed)
            {
                step.progress = 0.;
                step.status = Status::Running;
            }
        } else if let Some(step) = self
            .steps
            .iter_mut()
            .find(|step| step.status == Status::Paused)
        {
            step.status = Status::Running;
        } else {
            for step in &mut self.steps {
                step.status = Status::Waiting;
                step.progress = 0.;
            }

            self.steps[0].status = Status::Running;
        }

        self.ticking = true;
        self.task = Some(cx.spawn(async move |view, cx| {
            let mut previous = cx.background_executor().now();

            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(70))
                    .await;
                let now = cx.background_executor().now();
                let delta = now.saturating_duration_since(previous).as_secs_f64();

                previous = now;
                let keep_going = view
                    .update(cx, |this, cx| {
                        this.advance(delta);
                        this.ticking = this.steps.iter().any(|step| step.status == Status::Running);
                        cx.notify();

                        this.ticking
                    })
                    .unwrap_or(false);

                if !keep_going {
                    break;
                }
            }
        }));
        cx.notify();
    }
}

impl Render for Runner {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let failed = self.steps.iter().any(|step| step.status == Status::Failed);
        let completed = self.steps.iter().all(|step| step.status == Status::Done);
        let paused = self.steps.iter().any(|step| step.status == Status::Paused);
        let rgba = cx.theme().primary.to_rgb();
        let ink = Color::rgba(rgba.r.into(), rgba.g.into(), rgba.b.into(), rgba.a.into());
        let mut pipeline = div().flex().items_center().gap_3();

        for (index, step) in self.steps.iter_mut().enumerate() {
            let value = step
                .value
                .frame(step.progress, spring(), window, cx)
                .value() as f32;

            let status = match step.status {
                Status::Waiting => "Waiting",
                Status::Running => "Running",
                Status::Done => "Complete",
                Status::Failed => "Failed",
                Status::Paused => "Paused",
            };

            let checkmark = step
                .check
                .element(format!("step-check-{index}"), check(), [24., 24.], ink)
                .expect("valid check")
                .reveal(
                    StrokeReveal::new(if step.status == Status::Done { 1. } else { 0. }, 0., 1.)
                        .expect("valid reveal"),
                )
                .transition(spring());

            pipeline = pipeline.child(
                surface(cx)
                    .flex_1()
                    .min_w_0()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("0{}", index + 1)),
                            )
                            .child(checkmark),
                    )
                    .child(div().font_semibold().child(step.title))
                    .child(
                        div()
                            .id(format!("step-status-{index}"))
                            .test_support()
                            .aria_label(status)
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    )
                    .child(progress(
                        format!("step-progress-{index}"),
                        step.title,
                        step.progress as f32,
                        value,
                        cx,
                    )),
            );

            if index < self.connectors.len() {
                let connector = self.connectors[index]
                    .element(format!("step-link-{index}"), line(), [56., 24.], ink)
                    .expect("valid connector")
                    .reveal(
                        StrokeReveal::new(
                            if step.status == Status::Done { 1. } else { 0. },
                            0.,
                            1.,
                        )
                        .expect("valid reveal"),
                    )
                    .transition(spring());

                pipeline = pipeline.child(connector);
            }
        }

        let error = self.error_motion.frame(
            Motion::new(
                Target::default()
                    .height(if failed {
                        f32::from(window.rem_size()) * 8.
                    } else {
                        0.
                    })
                    .opacity(if failed { 1. } else { 0. }),
            )
            .transition(spring()),
            window,
            cx,
        );

        let summary = if failed {
            "Publish needs attention"
        } else if completed {
            "Release complete"
        } else if paused {
            "Run paused"
        } else if self.ticking {
            "Release in progress"
        } else {
            "Ready to run"
        };

        div().size_full().flex().flex_col()
            .child(heading("Ship, one step at a time.", "A simulated release pipeline. Pause it, let publishing fail, then retry without repeating completed work.", cx))
            .child(div().flex().items_center().justify_between().mb_8()
                .child(div().id("pipeline-status").test_support().aria_label(summary).font_medium().child(summary))
                .child(div().flex().items_center().gap_3()
                    .child(Checkbox::new("fail-publish").label("Fail at publish").checked(self.fail_publish).disabled(self.ticking)
                        .on_change(cx.listener(|this, checked, _, cx| { this.fail_publish = *checked; cx.notify(); })))
                    .child(Button::new("pause-run").ghost().label("Pause").disabled(!self.ticking)
                        .on_click(cx.listener(|this, _, _, cx| { for step in &mut this.steps { if step.status == Status::Running { step.status = Status::Paused; } }

                            cx.notify(); })))
                    .child(Button::new("run-pipeline").label(if paused { "Resume" } else if completed { "Run again" } else { "Run" })
                        .disabled(self.ticking || failed).on_click(cx.listener(|this, _, _, cx| this.run(false, cx))))))
            .child(pipeline)
            .child(error.apply(div().mt_6().overflow_hidden()).child(surface(cx).w_full().h(rems(8.)).p_5().flex().items_center().justify_between()
                .child(div().flex().flex_col().gap_2().child(div().font_semibold().child("Publish timed out"))
                    .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Completed steps are kept. Retry picks up here.")))
                .child(Button::new("retry-publish").label("Retry publish").disabled(!failed)
                    .on_click(cx.listener(|this, _, _, cx| this.run(true, cx))))))
    }
}
