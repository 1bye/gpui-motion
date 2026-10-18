use super::{heading, spring, surface};
use gpui_ce_motion::{Motion, Target};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::StyledExt as _;

use gpui_kit::{
    App, Context, FocusHandle, KeyBinding, MouseButton, Render, SharedString, Window, actions,
    component::{
        ActiveTheme, Disableable,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
    },
    div,
    prelude::*,
    rems,
};

use gpui_motion_kit::MotionState;

actions!(inbox_demo, [ArchiveSelected, UndoArchive]);

pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-backspace", ArchiveSelected, Some("MotionInbox")),
        KeyBinding::new("ctrl-backspace", ArchiveSelected, Some("MotionInbox")),
        KeyBinding::new("cmd-z", UndoArchive, Some("MotionInbox")),
        KeyBinding::new("ctrl-z", UndoArchive, Some("MotionInbox")),
    ]);
}

struct Message {
    sender: SharedString,
    subject: SharedString,
    snippet: SharedString,
    selected: bool,
    archived: bool,
    slot: usize,
    motion: MotionState,
}

pub(super) struct Inbox {
    messages: Vec<Message>,
    history: Vec<Vec<usize>>,
    toast: MotionState,
    focus: FocusHandle,
}

impl Inbox {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        let seed = [
            (
                "Mira Chen",
                "A little more room to breathe",
                "The new navigation is ready. I left a few notes on spacing and rhythm.",
            ),
            (
                "Theo Park",
                "Your Friday reading list",
                "Five things worth slowing down for this weekend.",
            ),
            (
                "Studio North",
                "The first look",
                "A few early explorations for the identity. Which direction feels right?",
            ),
            (
                "Alex Rivera",
                "Small details, big difference",
                "The prototype is feeling much better. Let’s walk through the last transitions.",
            ),
            (
                "Nora Wilson",
                "Coffee next week?",
                "I’ll be around the neighborhood on Tuesday afternoon.",
            ),
            (
                "Linear",
                "A quiet week of progress",
                "Your team completed 12 issues. Here’s what moved forward.",
            ),
        ];

        Self {
            messages: seed
                .into_iter()
                .enumerate()
                .map(|(slot, (sender, subject, snippet))| Message {
                    sender: sender.into(),
                    subject: subject.into(),
                    snippet: snippet.into(),
                    selected: false,
                    archived: false,
                    slot,
                    motion: MotionState::default(),
                })
                .collect(),
            history: Vec::new(),
            toast: MotionState::default(),
            focus: cx.focus_handle(),
        }
    }

    fn archive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let batch: Vec<_> = self
            .messages
            .iter()
            .enumerate()
            .filter(|(_, message)| message.selected && !message.archived)
            .map(|(id, _)| id)
            .collect();

        if batch.is_empty() {
            return;
        }

        window.focus(&self.focus, cx);

        for &id in &batch {
            self.messages[id].archived = true;
            self.messages[id].selected = false;
        }

        self.history.push(batch);
        self.reflow();
        cx.notify();
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(batch) = self.history.pop() else {
            return;
        };

        window.focus(&self.focus, cx);

        for id in batch {
            self.messages[id].archived = false;
        }

        self.reflow();
        cx.notify();
    }

    fn reflow(&mut self) {
        let mut slot = 0;

        for message in &mut self.messages {
            if !message.archived {
                message.slot = slot;
                slot += 1;
            }
        }
    }
}

impl Render for Inbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self
            .messages
            .iter()
            .filter(|message| !message.archived)
            .count();

        let selected = self
            .messages
            .iter()
            .any(|message| message.selected && !message.archived);

        let row_height = f32::from(window.rem_size()) * 4.5;

        let mut list = div()
            .id("message-list")
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();

        for (id, message) in self.messages.iter_mut().enumerate() {
            let y = message.slot as f32 * row_height;

            let frame = message.motion.frame(
                Motion::new(Target::default().y(y))
                    .exit(Target::default().x(56.).y(y).opacity(0.))
                    .present(!message.archived)
                    .transition(spring()),
                window,
                cx,
            );

            if !frame.is_mounted() {
                continue;
            }

            let present = frame.is_present();

            let row = surface(cx)
                .id(format!("message-{id}"))
                .test_support()
                .h(rems(4.))
                .flex()
                .items_center()
                .gap_4()
                .px_5()
                .when(message.selected, |row| row.bg(cx.theme().accent))
                .child(
                    Checkbox::new(format!("message-select-{id}"))
                        .w(rems(11.))
                        .flex_shrink_0()
                        .label(message.sender.clone())
                        .checked(message.selected)
                        .disabled(!present)
                        .on_change(cx.listener(move |this, checked, _, cx| {
                            if !this.messages[id].archived {
                                this.messages[id].selected = *checked;
                                cx.notify();
                            }
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().font_medium().child(message.subject.clone()))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(message.snippet.clone()),
                        ),
                );

            list = list.child(frame.apply(row).absolute().w_full());
        }

        let toast = self.toast.frame(
            Motion::new(Target::default())
                .initial(Target::default().y(12.).opacity(0.))
                .exit(Target::default().y(12.).opacity(0.))
                .present(!self.history.is_empty())
                .transition(spring()),
            window,
            cx,
        );

        let status: SharedString = format!("{active} messages").into();

        div()
            .id("inbox-demo")
            .key_context("MotionInbox")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if !this.focus.contains_focused(window, cx) {
                        window.focus(&this.focus, cx);
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &ArchiveSelected, window, cx| this.archive(window, cx)),
            )
            .on_action(cx.listener(|this, _: &UndoArchive, window, cx| this.undo(window, cx)))
            .child(heading(
                "A little less inbox.",
                "Select a few messages. Archive them. Undo before they finish leaving.",
                cx,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mb_4()
                    .child(
                        div()
                            .id("inbox-count")
                            .test_support()
                            .aria_label(status.clone())
                            .text_sm()
                            .child(status),
                    )
                    .child(
                        Button::new("archive-selected")
                            .label("Archive selected")
                            .disabled(!selected)
                            .on_click(cx.listener(|this, _, window, cx| this.archive(window, cx))),
                    ),
            )
            .child(list)
            .child(div().h(rems(3.5)).pt_3().child(toast.render(|frame| {
                frame.apply(
                    surface(cx)
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_4()
                        .py_2()
                        .child("Moved to archive")
                        .child(
                            Button::new("undo-archive")
                                .ghost()
                                .label("Undo")
                                .disabled(!toast.is_present())
                                .on_click(cx.listener(|this, _, window, cx| this.undo(window, cx))),
                        ),
                )
            })))
    }
}
