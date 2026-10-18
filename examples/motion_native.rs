//! Native Motion-like integration gallery. Run `cargo run --features demo --example motion_native`.
//! G toggles the current example, R removes/restores an exit item, T timing,
//! M reduced motion. Tab/Shift+Tab navigate; Alt+Arrow reorders focused items.
mod common;

use gpui::{
    App, Bounds, Context, Div, FocusHandle, KeyBinding, Menu, MenuItem, Render, Role, Stateful,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};

use gpui_ce_motion::*;
use std::{cell::Cell, rc::Rc, time::Duration};

actions!(
    native_gallery,
    [
        /// Toggle the current example's destination.
        Toggle,
        /// Remove or restore Card 3.
        Remove,
        /// Switch spring/tween timing.
        Timing,
        /// Toggle the reduced-motion preference.
        Reduced,
        /// Move focus forward.
        Next,
        /// Move focus backward.
        Previous,
        /// Close the gallery window.
        Close,
        /// Quit the gallery.
        Quit,
    ]
);

fn button(id: impl Into<gpui::ElementId>, label: impl Into<gpui::SharedString>) -> Stateful<Div> {
    let label = label.into();

    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .tab_index(0)
        .px_3()
        .py_2()
        .bg(rgb(0x2a3344))
        .rounded_md()
        .border_1()
        .border_color(rgb(0x566681))
        .focus_visible(|s| s.border_color(rgb(0xffd4a0)))
        .child(label)
}

fn shape(alternate: bool) -> PathShape {
    PathShape::new([
        PathCommand::Move([20., 80.]),
        PathCommand::Cubic(
            [60., if alternate { 80. } else { 0. }],
            [140., if alternate { 80. } else { 0. }],
            [200., 80.],
        ),
        PathCommand::Cubic(
            [140., if alternate { 80. } else { 160. }],
            [60., if alternate { 80. } else { 160. }],
            [20., 80.],
        ),
        PathCommand::Close,
    ])
    .expect("compatible prepared geometry")
}

struct Gallery {
    root: FocusHandle,
    page: usize,
    alternate: bool,
    tween: bool,
    spring: Spring,
    blur: bool,
    pivot: bool,
    filled: bool,
    gestures: GestureState,
    scroll: gpui::ScrollHandle,
    reorder: ReorderGroup<u8>,
    commits: Rc<Cell<usize>>,
    pop: PopLayout,
    presence: PresenceGroup<usize>,
    states: [MotionState; 6],
    removed: bool,
    shared: SharedLayout<u8>,
    vector: VectorState,
    shapes: [PathShape; 2],
    svg: SvgMotionState,
    styles: StyleMotionState,
    clicks: usize,
}

impl Gallery {
    fn transition(&self) -> Transition {
        if self.tween {
            Transition::Tween(Duration::from_millis(800), Easing::EaseInOut)
        } else {
            Transition::Spring(self.spring)
        }
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let root = cx.focus_handle();

        root.focus(window, cx);
        let commits = Rc::new(Cell::new(0));
        let count = commits.clone();

        let reorder = ReorderGroup::new(vec![1, 2, 3, 4, 5], Axis::Y)
            .unwrap()
            .on_change(move |_, _, _| count.set(count.get() + 1));

        let mut presence = PresenceGroup::new(PresenceMode::Sync);

        presence.update(0..6).unwrap();
        let shapes = [shape(false), shape(true)];

        Self {
            root,
            page: 0,
            alternate: false,
            tween: false,
            spring: Spring::visual(Duration::from_millis(450), 0.25).expect("valid spring"),
            blur: false,
            pivot: false,
            filled: false,
            gestures: GestureState::default()
                .with_drag(Drag::new([0., 0.], [0., 0.], [260., 80.]).snap_grid(20.))
                .in_view(1., false),
            scroll: gpui::ScrollHandle::new(),
            reorder,
            commits,
            pop: PopLayout::default(),
            presence,
            states: std::array::from_fn(|_| MotionState::default()),
            removed: false,
            shared: SharedLayout::default(),
            vector: VectorState::new(shapes[0].clone()),
            shapes,
            svg: SvgMotionState::default(),
            styles: StyleMotionState::default(),
            clicks: 0,
        }
    }

    fn layout(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let transition = self.transition();

        let content = div()
            .w(px(480.))
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(rgb(0x26374d))
            .rounded_lg()
            .child("Content-measured accordion")
            .when(self.alternate, |d| {
                d.children((0..5).map(|i| {
                    div()
                        .text_sm()
                        .child(format!("Content row {} — measured by CE", i + 1))
                }))
            });

        let accordion = AutoSize::new("accordion", content)
            .width(480.)
            .transition(transition);

        let mut row = div()
            .id("pop-row")
            .relative()
            .flex()
            .items_start()
            .gap_3()
            .p_2()
            .w(px(660.))
            .h(px(92.));

        let children: Vec<_> = self
            .presence
            .children()
            .iter()
            .map(|c| (*c.key(), c.is_present()))
            .collect();

        let mut finished = Vec::new();

        for (id, present) in children {
            let frame = self.states[id].frame(
                Motion::new(Target::default().opacity(1.))
                    .initial(Target::default().opacity(0.))
                    .exit(Target::default().opacity(0.))
                    .present(present)
                    .transition(transition),
                window,
                cx,
            );

            if !frame.is_mounted() {
                finished.push(id);
                continue;
            }

            let card = frame
                .apply(
                    button(("pop-card", id), format!("Card {}", id + 1))
                        .w(px(90.))
                        .h(px(56.))
                        .flex_shrink_0(),
                )
                .when(!present, |d| d.aria_description("Exiting; unavailable"))
                .when(!present, |d| d.tab_index(-1))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if present {
                        this.clicks += 1;
                        cx.notify();
                    }
                }));

            row = row.child(
                self.pop
                    .item(("pop", id), present, card)
                    .transition(transition),
            );
        }

        for id in finished {
            self.presence.finish_exit(&id);
            window.request_animation_frame();
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child("G changes content height. R removes/restores Card 3 while its siblings reflow.")
            .child(accordion)
            .child(div().text_sm().child(
                "Popped exits reserve no flow space; the fading card stays in its measured bounds.",
            ))
            .child(self.pop.container("pop-container", row))
            .into_any_element()
    }

    fn input(&mut self, _: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let control = button("drag-card", "Drag or use arrows")
            .w(px(170.))
            .h(px(56.))
            .on_click(cx.listener(|this, _, _, cx| {
                this.clicks += 1;
                cx.notify();
            }));

        let element = self.gestures.bind(
            "gesture",
            control,
            Motion::new(Target::default().opacity(1.).border_radius(12.))
                .transition(self.transition()),
            GestureTargets::default()
                .while_hover(Target::default().opacity(0.8))
                .while_press(Target::default().opacity(0.5)),
        );

        let progress = ScrollMotion::new(
            "scroll-progress",
            self.scroll.clone(),
            Axis::Y,
            Target::default().width(0.),
            Target::default().width(480.),
            div().h(px(8.)).bg(rgb(0xb8c8ff)),
        )
        .transition(Transition::default());

        div().flex().flex_col().gap_4()
            .child("Hover/press feedback binds automatically. Drag inside the stage; release outside, or Escape to cancel.")
            .child(div().w(px(480.)).h(px(180.)).p_3().bg(rgb(0x222b3b)).rounded_lg().child(element))
            .child(format!("Viewport visible: {} · Clicks: {}",self.gestures.interaction().in_view,self.clicks))
            .child("Scroll below: the bar reads the native scroll handle and smooths its progress.")
            .child(div().w(px(480.)).h(px(8.)).bg(rgb(0x303b50)).child(progress))
            .child(div().id("scroll-box").w(px(480.)).h(px(160.)).overflow_scroll().track_scroll(&self.scroll)
                .child(div().flex().flex_col().children((0..12).map(|i|div().h(px(36.)).flex_shrink_0().p_2().bg(rgb(if i%2==0{0x26374d}else{0x202c40})).child(format!("Native scroll row {}",i+1)))))).into_any_element()
    }

    fn reorder(&self) -> gpui::AnyElement {
        let mut list = div().flex().flex_col().gap_3().w(px(340.));

        for key in self.reorder.order().iter().copied() {
            let card = button(
                ("reorder-card", usize::from(key)),
                format!("Item {key} — drag to reorder"),
            )
            .w(px(340.))
            .h(px(52.))
            .flex_shrink_0();

            list = list.child(
                self.reorder
                    .item(("reorder", usize::from(key)), key, card)
                    .transition(self.transition()),
            );
        }

        div().flex().flex_col().gap_4().child("Drag an item. Alt+Up / Alt+Down moves the focused item. Escape restores a pointer gesture's original order.")
            .child(list).child(format!("Committed reorder operations: {}",self.commits.get())).into_any_element()
    }

    fn shared(&self) -> gpui::AnyElement {
        let first = div()
            .absolute()
            .left(px(16.))
            .top(px(20.))
            .w(px(160.))
            .h(px(100.))
            .rounded_lg()
            .bg(rgb(0x6685ba))
            .flex()
            .items_center()
            .justify_center()
            .child("First appearance");

        let second = div()
            .absolute()
            .left(px(370.))
            .top(px(70.))
            .w(px(220.))
            .h(px(80.))
            .rounded_lg()
            .bg(rgb(0xa1689c))
            .flex()
            .items_center()
            .justify_center()
            .child("Second appearance");

        div().flex().flex_col().gap_4().child("G switches the active appearance. Reverse during movement to interrupt.")
            .child(div().relative().w(px(660.)).h(px(220.)).bg(rgb(0x222b3b)).rounded_lg()
                .child(self.shared.member(0,"shared-first",!self.alternate,first).transition(self.transition()))
                .child(self.shared.member(0,"shared-second",self.alternate,second).transition(self.transition())))
            .child("Both appearances share a position and crossfade. Their sizes change immediately; no size projection is applied.").into_any_element()
    }

    fn vector(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let transition = self.transition();

        let vector = self
            .vector
            .element(
                "vector",
                self.shapes[usize::from(self.alternate)].clone(),
                [230., 180.],
                Color::rgb(0xb8c8ff),
            )
            .unwrap()
            .transition(transition)
            .reveal(
                StrokeReveal::new(
                    if self.alternate { 0.35 } else { 1. },
                    if self.alternate { 0.2 } else { 0. },
                    0.5,
                )
                .unwrap(),
            );

        let vector = if self.filled {
            vector.fill()
        } else {
            vector.stroke(4.)
        };

        let svg_frame = self.svg.frame(
            SvgMotion::new(
                SvgTarget::default()
                    .scale(if self.alternate { 1.3 } else { 1. })
                    .rotate_degrees(if self.alternate { 360. } else { 0. })
                    .translate(if self.alternate { 30. } else { 0. }, 0.),
            )
            .transition(transition),
            window,
            cx,
        );

        let pivot = if self.pivot {
            gpui::point(px(0.), px(0.))
        } else {
            gpui::point(px(40.), px(40.))
        };

        let svg = gpui::svg()
            .path("motion.svg")
            .size(px(80.))
            .text_color(rgb(0xffd4a0))
            .with_transformation(svg_frame.transformation_about(size(px(80.), px(80.)), pivot));

        div().flex().flex_col().gap_4()
            .child("G morphs compatible curve controls and changes normalized stroke length/offset/spacing.")
            .child(div().flex().gap_4().child(button("fill","Stroke / fill").on_click(cx.listener(|this,_,_,cx|{this.filled= !this.filled;cx.notify();})))
                .child(button("pivot","SVG center / corner pivot").on_click(cx.listener(|this,_,_,cx|{this.pivot= !this.pivot;cx.notify();}))))
            .child(div().flex().gap_4().child(vector).child(div().w(px(200.)).h(px(180.)).p_6().border_1().border_color(rgb(0x566681)).child(svg)))
            .child(format!("SVG pivot: {} · Native path error: {:?}",if self.pivot{"top-left"}else{"center"},self.vector.error()))
            .child("Curve-length stroke drawing is approximated with 32 segments per curve. SVG layout and hitboxes stay fixed.").into_any_element()
    }

    fn styles(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let target = StyleTarget::default()
            .padding([if self.alternate { 28. } else { 12. }; 4])
            .radii(if self.alternate {
                [4., 24., 4., 24.]
            } else {
                [8.; 4]
            })
            .border_widths([if self.alternate { 4. } else { 1. }; 4])
            .color(
                ColorProperty::Background,
                Color::rgb(if self.alternate { 0x564473 } else { 0x283b59 }),
            )
            .color(
                ColorProperty::Text,
                Color::rgb(if self.alternate { 0xffd4a0 } else { 0xe9efff }),
            )
            .color(ColorProperty::Border, Color::rgb(0x90a5c9))
            .shadow(
                [8., 12.],
                if self.alternate { 24. } else { 4. },
                0.,
                Color::rgba(0., 0., 0., 0.4),
            )
            .value(StyleProperty::Blur, if self.blur { 2. } else { 0. })
            .value(StyleProperty::BackdropBlur, if self.blur { 8. } else { 0. });

        let frame = self.styles.frame(
            StyleMotion::new(target).transition(self.transition()),
            window,
            cx,
        );

        let card = frame.apply(div().w(px(360.)).h(px(150.)).child(
            "Native colors, padding, four corner radii, border widths, and shadow parameters.",
        ));

        div().flex().flex_col().gap_4().child("G changes styles. Blur uses CE's offscreen filter path and has a separate GPU cost.")
            .child(button("blur","Toggle blur / backdrop blur").on_click(cx.listener(|this,_,_,cx|{this.blur= !this.blur;cx.notify();})))
            .child(div().w(px(500.)).h(px(250.)).p_6().bg(rgb(0x283040)).child(card))
            .child("The visual spring reaches 90% of a resting step at 450 ms; T selects an 800 ms tween.").into_any_element()
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut pages = div().flex().gap_2();

        for (index, name) in [
            "Layout",
            "Input / scroll",
            "Reorder",
            "Shared",
            "Vector / SVG",
            "Styles",
        ]
        .into_iter()
        .enumerate()
        {
            pages = pages.child(
                button(("page", index), name)
                    .when(index == self.page, |d| d.bg(rgb(0x4a6087)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.gestures.cancel(cx);
                        this.page = index;
                        this.root.focus(window, cx);
                        cx.notify();
                    })),
            );
        }

        let panel = match self.page {
            0 => self.layout(window, cx),
            1 => self.input(window, cx),
            2 => self.reorder(),
            3 => self.shared(),
            4 => self.vector(window, cx),
            _ => self.styles(window, cx),
        };

        let status = format!(
            "{} · Reduced motion {} · {} clicks",
            if self.tween {
                "800 ms tween"
            } else {
                "450 ms visual spring"
            },
            cx.reduce_motion(),
            self.clicks
        );

        div()
            .id("native-gallery")
            .track_focus(&self.root)
            .tab_group()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .bg(rgb(0x171d29))
            .text_color(rgb(0xe9efff))
            .on_action(cx.listener(|this, _: &Toggle, _, cx| {
                this.alternate = !this.alternate;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Remove, window, cx| {
                this.removed = !this.removed;
                this.root.focus(window, cx);
                this.presence
                    .update((0..6).filter(|id| !this.removed || *id != 2))
                    .unwrap();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Timing, _, cx| {
                this.tween = !this.tween;
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &Reduced, _, cx| {
                let value = !cx.reduce_motion();

                cx.set_reduce_motion(value);
                cx.notify();
            }))
            .on_action(|_: &Next, w, cx| w.focus_next(cx))
            .on_action(|_: &Previous, w, cx| w.focus_prev(cx))
            .on_action(|_: &Close, w, _| w.remove_window())
            .child(div().text_2xl().child("Native motion integrations"))
            .child("G toggle · R remove/restore · T timing · M reduced motion · Tab navigate")
            .child(pages)
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        button("toggle", "Toggle example")
                            .on_click(|_, w, cx| w.dispatch_action(Box::new(Toggle), cx)),
                    )
                    .child(
                        button("remove", "Remove / restore Card 3")
                            .on_click(|_, w, cx| w.dispatch_action(Box::new(Remove), cx)),
                    )
                    .child(
                        button("timing", "Spring / tween")
                            .on_click(|_, w, cx| w.dispatch_action(Box::new(Timing), cx)),
                    )
                    .child(
                        button("reduce", "Reduced motion")
                            .on_click(|_, w, cx| w.dispatch_action(Box::new(Reduced), cx)),
                    ),
            )
            .child(
                div()
                    .id("gallery-status")
                    .role(Role::Label)
                    .aria_label(status.clone())
                    .text_sm()
                    .child(status),
            )
            .child(
                div()
                    .id("panel-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(panel),
            )
    }
}

fn main() {
    let assets = common::SvgAsset(include_bytes!("assets/motion.svg"));

    gpui_platform::application()
        .with_assets(assets)
        .run(|cx: &mut App| {
            let modifier = if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            };

            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_window_closed(|cx, _| cx.quit()).detach();
            cx.bind_keys([
                KeyBinding::new("g", Toggle, None),
                KeyBinding::new("r", Remove, None),
                KeyBinding::new("t", Timing, None),
                KeyBinding::new("m", Reduced, None),
                KeyBinding::new("tab", Next, None),
                KeyBinding::new("shift-tab", Previous, None),
                KeyBinding::new(&format!("{modifier}-w"), Close, None),
                KeyBinding::new(&format!("{modifier}-q"), Quit, None),
            ]);
            cx.set_menus(vec![
                Menu::new("Native motion").items([MenuItem::action("Quit", Quit)]),
                Menu::new("File").items([MenuItem::action("Close Window", Close)]),
                Menu::new("Motion").items([
                    MenuItem::action("Toggle Example", Toggle),
                    MenuItem::action("Remove / Restore", Remove),
                    MenuItem::action("Spring / Tween", Timing),
                    MenuItem::action("Reduced Motion", Reduced),
                ]),
            ]);
            let bounds = Bounds::centered(None, size(px(1040.), px(860.)), cx);

            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(1040.), px(860.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Native motion integrations".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    |w, cx| cx.new(|cx| Gallery::new(w, cx)),
                )
                .expect("open native gallery");

            window
                .update(cx, |_, w, _| w.activate_window())
                .expect("gallery window");
            cx.activate(true);
        });
}
