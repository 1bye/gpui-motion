use super::{heading, spring, surface};
use gpui_ce_motion::{Motion, Target};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::StyledExt as _;

use gpui_kit::{
    AppContext, Context, Entity, Render, SharedString, Window,
    component::{
        ActiveTheme,
        button::{Button, ButtonVariants},
        command::{Command, CommandItem, CommandState},
    },
    div,
    prelude::*,
};

use gpui_motion_kit::MotionState;

const DOCUMENTS: [(&str, &str); 6] = [
    (
        "Product principles",
        "Make the next action clear. Keep the details close to the thing they describe. Let motion explain the relationship.",
    ),
    (
        "Release checklist",
        "Check the portable core, native interactions, reduced motion, release build, and package contents before shipping.",
    ),
    (
        "Design notes",
        "A quiet canvas. A useful hierarchy. Enough room for the work. Small, deliberate moments of feedback.",
    ),
    (
        "Research sessions",
        "Start with the person’s task. Listen for the words they use. Watch what they try before explaining the interface.",
    ),
    (
        "Motion guidelines",
        "Keep identity stable, preserve momentum, and let every interaction interrupt the one before it. Respect reduced motion.",
    ),
    (
        "Team handbook",
        "Write down decisions and make the important work easy to find. Leave each workflow a little easier to understand.",
    ),
];

pub(super) struct Palette {
    command: Entity<CommandState>,
    selected: usize,
    opened: usize,
    previews: [MotionState; 6],
    height: MotionState,
    matches: usize,
}

impl Palette {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            command: cx.new(|cx| CommandState::new(window, cx)),
            selected: 0,
            opened: 0,
            previews: std::array::from_fn(|_| MotionState::default()),
            height: MotionState::default(),
            matches: DOCUMENTS.len(),
        }
    }
}

impl Render for Palette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let confirm_owner = owner.clone();
        let query_owner = owner.clone();
        let state = self.command.clone();

        let height = self.height.frame(
            Motion::new(
                Target::default()
                    .height(f32::from(window.rem_size()) * (4. + self.matches.min(6) as f32 * 2.5)),
            )
            .transition(spring()),
            window,
            cx,
        );

        let mut preview = div().relative().flex_1().min_h_0();

        for (index, (title, body)) in DOCUMENTS.iter().enumerate() {
            let frame = self.previews[index].frame(
                Motion::new(Target::default())
                    .initial(Target::default().y(10.).opacity(0.))
                    .exit(Target::default().y(-10.).opacity(0.))
                    .present(index == self.selected)
                    .transition(spring()),
                window,
                cx,
            );

            if frame.is_mounted() {
                preview = preview.child(
                    frame
                        .apply(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .flex_col()
                                .gap_5()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("PREVIEW"),
                                )
                                .child(div().text_2xl().font_semibold().child(*title))
                                .child(div().text_color(cx.theme().muted_foreground).child(*body)),
                        )
                        .absolute()
                        .size_full(),
                );
            }
        }

        let command = Command::new(&self.command)
            .items(
                DOCUMENTS
                    .iter()
                    .map(|(title, _)| CommandItem::new().label(*title)),
            )
            .placeholder("Find a document…")
            .on_select(move |index, _, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.selected = index.row;
                    cx.notify();
                });
            })
            .on_confirm(move |index, _, cx| {
                let _ = confirm_owner.update(cx, |this, cx| {
                    this.opened = index.row;
                    cx.notify();
                });
            })
            .on_query(move |_, _, cx| {
                let matches = state.read(cx).matched_count();

                let _ = query_owner.update(cx, |this, cx| {
                    this.matches = matches;
                    cx.notify();
                });
            })
            .w_full()
            .h_full();

        let opened: SharedString = format!("Open document: {}", DOCUMENTS[self.opened].0).into();

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(heading(
                "Find it before you finish typing.",
                "Type a few letters. Use ↑ / ↓ to preview. Press Enter to open a document.",
                cx,
            ))
            .child(
                div()
                    .flex()
                    .gap_8()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .w_96()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(height.apply(div().overflow_hidden()).child(command))
                            .child(
                                Button::new("focus-command")
                                    .ghost()
                                    .label("Search documents")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.command.update(cx, |state, cx| state.focus(window, cx))
                                    })),
                            ),
                    )
                    .child(
                        surface(cx)
                            .flex_1()
                            .min_w_0()
                            .p_8()
                            .flex()
                            .flex_col()
                            .gap_5()
                            .child(preview)
                            .child(
                                div()
                                    .id("opened-document")
                                    .test_support()
                                    .aria_label(opened.clone())
                                    .pt_4()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(opened),
                            ),
                    ),
            )
    }
}
