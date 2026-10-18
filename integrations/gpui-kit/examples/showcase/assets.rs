use super::{heading, spring, surface};
use gpui_ce_motion::{Motion, Target};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::StyledExt as _;

use gpui_kit::{
    AppContext, Context, Entity, Image, ImageFormat, ObjectFit, Render, RenderImage, SharedString,
    Subscription, Window,
    component::{
        ActiveTheme, Disableable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        tab::{Tab, TabBar},
    },
    div, img,
    prelude::*,
    rems,
};

use gpui_motion_kit::MotionState;

use std::sync::Arc;

const ARTWORK: [&[u8]; 6] = [
    include_bytes!("art/cover-0.svg"),
    include_bytes!("art/cover-1.svg"),
    include_bytes!("art/cover-2.svg"),
    include_bytes!("art/cover-3.svg"),
    include_bytes!("art/cover-4.svg"),
    include_bytes!("art/cover-5.svg"),
];

struct Asset {
    name: SharedString,
    kind: usize,
    image: Arc<RenderImage>,
    slot: usize,
    visible: bool,
    motion: MotionState,
    preview: MotionState,
}

pub(super) struct Browser {
    assets: Vec<Asset>,
    category: usize,
    selected: Option<usize>,
    search: Entity<InputState>,
    query: String,
    inspector: MotionState,
    _subscription: Subscription,
}

impl Browser {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Find an asset…"));

        let subscription = cx.subscribe(&search, |this, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.query = input.read(cx).value().to_lowercase();
                this.filter();
                cx.notify();
            }
        });

        let names = [
            "Alpine", "Sundial", "Assembly", "Orbit", "Daybreak", "Current",
        ];

        let assets = names
            .into_iter()
            .enumerate()
            .map(|(id, name)| Asset {
                name: name.into(),
                kind: id % 2 + 1,
                // Decode bundled artwork once; rendering never reads the source checkout.
                image: Image::from_bytes(ImageFormat::Svg, ARTWORK[id].to_vec())
                    .to_image_data(cx.svg_renderer())
                    .expect("valid embedded showcase artwork"),
                slot: id,
                visible: true,
                motion: MotionState::default(),
                preview: MotionState::default(),
            })
            .collect();

        Self {
            assets,
            category: 0,
            selected: Some(0),
            search,
            query: String::new(),
            inspector: MotionState::default(),
            _subscription: subscription,
        }
    }

    fn filter(&mut self) {
        let mut slot = 0;

        for asset in &mut self.assets {
            asset.visible = (self.category == 0 || self.category == asset.kind)
                && asset.name.to_lowercase().contains(&self.query);

            if asset.visible {
                asset.slot = slot;
                slot += 1;
            }
        }
    }
}

impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rem = f32::from(window.rem_size());
        let mut grid = div().relative().w(rems(33.)).h(rems(28.)).flex_shrink_0();
        let mut previews = div().relative().w_full().h_64();

        for (id, asset) in self.assets.iter_mut().enumerate() {
            let x = (asset.slot % 3) as f32 * rem * 11.;
            let y = (asset.slot / 3) as f32 * rem * 14.;

            let frame = asset.motion.frame(
                Motion::new(Target::default().x(x).y(y))
                    .exit(Target::default().x(x).y(y + 12.).opacity(0.))
                    .present(asset.visible)
                    .transition(spring()),
                window,
                cx,
            );

            let preview = asset.preview.frame(
                Motion::new(Target::default())
                    .initial(Target::default().y(8.).opacity(0.))
                    .exit(Target::default().y(-8.).opacity(0.))
                    .present(self.selected == Some(id))
                    .transition(spring()),
                window,
                cx,
            );

            if frame.is_mounted() {
                grid = grid.child(
                    frame
                        .apply(
                            Button::new(format!("asset-{id}"))
                                .ghost()
                                .w(rems(10.))
                                .h(rems(13.))
                                .p_2()
                                .disabled(!frame.is_present())
                                .accessibility_label(format!("Inspect {}", asset.name))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_3()
                                        .w_full()
                                        .child(
                                            img(asset.image.clone())
                                                .w_full()
                                                .h_32()
                                                .object_fit(ObjectFit::Cover)
                                                .rounded_md(),
                                        )
                                        .child(div().font_medium().child(asset.name.clone()))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(if asset.kind == 1 {
                                                    "Artwork"
                                                } else {
                                                    "Icon study"
                                                }),
                                        ),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected = Some(id);
                                    cx.notify();
                                })),
                        )
                        .absolute(),
                );
            }

            if preview.is_mounted() {
                previews = previews.child(
                    preview
                        .apply(
                            img(asset.image.clone())
                                .size_full()
                                .object_fit(ObjectFit::Contain),
                        )
                        .absolute()
                        .size_full(),
                );
            }
        }

        let selected_name = self
            .selected
            .map(|id| self.assets[id].name.clone())
            .unwrap_or_else(|| "Inspector".into());

        let inspector = self.inspector.frame(
            Motion::new(Target::default().width(if self.selected.is_some() {
                rem * 20.
            } else {
                0.
            }))
            .transition(spring()),
            window,
            cx,
        );

        div().size_full().flex().flex_col()
            .child(heading("A home for your ideas.", "Filter the collection. Inspect a thumbnail. Keep your place as everything moves.", cx))
            .child(div().flex().items_center().justify_between().gap_6().mb_6()
                .child(TabBar::new("asset-filters").pill().selected_index(self.category)
                    .children(["All", "Artwork", "Icon studies"].map(|label| Tab::new().label(label)))
                    .on_click(cx.listener(|this, index: &usize, _, cx| { this.category = *index; this.filter(); cx.notify(); })))
                .child(Input::new(&self.search).id("asset-search").w_64()))
            .child(div().id("asset-workspace").flex().flex_1().min_h_0().gap_8().overflow_x_scroll()
                .child(grid)
                .child(inspector.apply(div().id("asset-inspector").test_support().overflow_hidden().flex_shrink_0())
                    .child(surface(cx).w(rems(20.)).p_5().flex().flex_col().gap_4()
                        .child(div().flex().items_center().justify_between().child(div().font_semibold().child(selected_name))
                            .child(Button::new("close-inspector").ghost().label("Close").on_click(cx.listener(|this, _, _, cx| { this.selected = None; cx.notify(); }))))
                        .child(previews)
                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child("SVG artwork · 400 × 300"))
                        .child("A closer look, without losing your place in the collection."))))
    }
}
