use super::{heading, spring, surface};
use gpui_ce_motion::{Motion, Target};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::StyledExt as _;

use gpui_kit::{
    AppContext, Bounds, Context, DragMoveEvent, Pixels, Render, SharedString, Task, Window, canvas,
    component::{
        ActiveTheme, Disableable,
        button::{Button, ButtonVariants},
    },
    div,
    prelude::*,
    px, rems,
};

use gpui_motion_kit::MotionState;
use std::{cell::Cell, rc::Rc, time::Duration};

const COLUMNS: [&str; 3] = ["To do", "In progress", "Done"];

#[derive(Clone)]
struct CardDrag {
    id: usize,
    title: SharedString,
}

impl Render for CardDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        surface(cx)
            .w_64()
            .p_5()
            .shadow_lg()
            .opacity(0.94)
            .font_medium()
            .child(self.title.clone())
    }
}

struct Card {
    title: SharedString,
    category: &'static str,
    motion: MotionState,
}

pub(super) struct Planner {
    cards: Vec<Card>,
    columns: [Vec<usize>; 3],
    dragging: Option<usize>,
    hover: Option<(usize, usize)>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    drag_watch: Option<Task<()>>,
}

fn place(columns: &mut [Vec<usize>; 3], id: usize, column: usize, mut slot: usize) {
    for (source, items) in columns.iter_mut().enumerate() {
        if let Some(index) = items.iter().position(|item| *item == id) {
            items.remove(index);

            if source == column && index < slot {
                slot -= 1;
            }

            break;
        }
    }

    let slot = slot.min(columns[column].len());

    columns[column].insert(slot, id);
}

impl Planner {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            cards: [
                ("Refine the empty state", "Design"),
                ("Keyboard shortcuts", "Interaction"),
                ("Write the getting started guide", "Documentation"),
                ("Review the new inspector", "Design"),
                ("Ship the motion package", "Release"),
                ("Keep the details in view", "Interaction"),
            ]
            .into_iter()
            .map(|(title, category)| Card {
                title: title.into(),
                category,
                motion: MotionState::default(),
            })
            .collect(),
            columns: [vec![0, 1, 2], vec![3, 5], vec![4]],
            dragging: None,
            hover: None,
            bounds: Rc::new(Cell::new(Bounds::default())),
            drag_watch: None,
        }
    }

    fn begin_drag(&mut self, id: usize, cx: &mut Context<Self>) {
        self.dragging = Some(id);
        self.hover = None;
        self.drag_watch = Some(cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(30))
                    .await;
                let active = view
                    .update(cx, |this, cx| {
                        if !cx.has_active_drag() {
                            this.dragging = None;
                            this.hover = None;
                            cx.notify();

                            false
                        } else {
                            true
                        }
                    })
                    .unwrap_or(false);

                if !active {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn target(&self, position: gpui_kit::Point<Pixels>, rem: f32) -> (usize, usize) {
        let bounds = self.bounds.get();
        let relative = position - bounds.origin;
        let width = f32::from(bounds.size.width).max(1.);
        let column = ((f32::from(relative.x) / (width / 3.)).floor() as usize).min(2);
        let slot = ((f32::from(relative.y) / rem - 4.) / 7.).round().max(0.) as usize;

        (column, slot)
    }
}

impl Render for Planner {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rem = f32::from(window.rem_size());
        let measured = f32::from(self.bounds.get().size.width);

        let width = if measured > 1. {
            measured
        } else {
            f32::from(window.viewport_size().width) - rem * 4.
        };

        let column_width = width / 3.;
        let mut displayed = self.columns.clone();

        if let (Some(id), Some((column, slot))) = (self.dragging, self.hover) {
            place(&mut displayed, id, column, slot);
        }

        let cache = self.bounds.clone();

        let mut board = div()
            .id("planner-board")
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(
                canvas(
                    move |bounds, window, _| {
                        if cache.replace(bounds) != bounds {
                            window.request_animation_frame();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<CardDrag>, window, cx| {
                    let next = if event.bounds.contains(&event.event.position) {
                        Some(this.target(event.event.position, f32::from(window.rem_size())))
                    } else {
                        None
                    };

                    if this.hover != next {
                        this.hover = next;
                        cx.notify();
                    }
                }),
            )
            .on_drop(cx.listener(|this, drag: &CardDrag, window, cx| {
                let (column, slot) =
                    this.target(window.mouse_position(), f32::from(window.rem_size()));

                place(&mut this.columns, drag.id, column, slot);
                this.dragging = None;
                this.hover = None;
                cx.notify();
            }));

        for (column, label) in COLUMNS.iter().enumerate() {
            board = board.child(
                div()
                    .id(format!("planner-column-{column}"))
                    .test_support()
                    .absolute()
                    .left(px(column as f32 * column_width))
                    .top_0()
                    .bottom_0()
                    .w(px(column_width - rem))
                    .rounded_lg()
                    .bg(cx.theme().background)
                    .child(
                        div()
                            .p_4()
                            .font_semibold()
                            .child(format!("{label} · {}", self.columns[column].len())),
                    ),
            );

            for (slot, &id) in displayed[column].iter().enumerate() {
                let card = &mut self.cards[id];
                let x = column as f32 * column_width + rem * 0.75;
                let y = (4. + slot as f32 * 7.) * rem;

                let frame = card.motion.frame(
                    Motion::new(
                        Target::default()
                            .x(x)
                            .y(y)
                            .opacity(if self.dragging == Some(id) { 0.2 } else { 1. }),
                    )
                    .initial(Target::default().x(x).y(y + 12.).opacity(0.))
                    .transition(spring()),
                    window,
                    cx,
                );

                let payload = CardDrag {
                    id,
                    title: card.title.clone(),
                };

                let owner = cx.entity().downgrade();
                let next_column = (column + 1) % 3;
                let button_label = format!("Move {} to {}", card.title, COLUMNS[next_column]);

                let element = surface(cx)
                    .id(format!("planner-card-{id}"))
                    .test_support()
                    .w(px(column_width - rem * 2.5))
                    .h(rems(6.))
                    .p_4()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .cursor_grab()
                    .child(div().font_medium().child(card.title.clone()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(card.category),
                            )
                            .child(
                                Button::new(format!("move-task-{id}"))
                                    .ghost()
                                    .label("→")
                                    .accessibility_label(button_label)
                                    .disabled(self.dragging.is_some())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let end = this.columns[next_column].len();

                                        place(&mut this.columns, id, next_column, end);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .on_drag(payload, move |drag, _, _, cx| {
                        let _ = owner.update(cx, |this, cx| this.begin_drag(drag.id, cx));

                        cx.new(|_| drag.clone())
                    });

                board = board.child(frame.apply(element).absolute());

                if self.dragging == Some(id) && self.hover.is_some() {
                    board = board.child(
                        div()
                            .absolute()
                            .left(px(x))
                            .top(px(y))
                            .w(px(column_width - rem * 2.5))
                            .h(rems(6.))
                            .rounded_lg()
                            .border_2()
                            .border_color(cx.theme().primary),
                    );
                }
            }
        }

        div().size_full().flex().flex_col()
            .child(heading("Make room for the next thing.", "Drag cards between columns or reorder them. The arrow buttons provide a keyboard-accessible alternative.", cx))
            .child(board)
    }
}
