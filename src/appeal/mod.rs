//! The `appeal` section: a relapse-free clock backed by `appeal.toml`.

mod model;

use std::time::Duration;

use chrono::Local;
use gpui::prelude::*;
use gpui::{ClickEvent, Context, FontWeight, Render, Window, div, px, rgb};

use crate::theme;

use self::model::{AppealData, format_elapsed, now_seconds};

pub struct Appeal {
    data: AppealData,
    confirming: bool,
    status: Option<String>,
}

impl Appeal {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (data, status) = match AppealData::load_or_create() {
            Ok(data) => (data, None),
            Err(error) => (
                AppealData::default(),
                Some(format!("could not load relapse data: {error:#}")),
            ),
        };
        let this = Self {
            data,
            confirming: false,
            status,
        };
        this.start_tick(cx);
        this
    }

    fn start_tick(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_secs(1))
                .await;
            let alive = this
                .update(cx, |_, cx| cx.notify())
                .is_ok();
            if !alive {
                break;
            }
        })
        .detach();
    }

    pub fn confirming(&self) -> bool {
        self.confirming
    }

    pub fn request_reset(&mut self, cx: &mut Context<Self>) {
        self.confirming = true;
        cx.notify();
    }

    pub fn cancel_reset(&mut self, cx: &mut Context<Self>) {
        self.confirming = false;
        cx.notify();
    }

    pub fn confirm_reset(&mut self, cx: &mut Context<Self>) {
        self.confirming = false;
        self.data.last_relapse = now_seconds();
        self.status = match self.data.save() {
            Ok(()) => None,
            Err(error) => Some(format!("save failed: {error:#}")),
        };
        cx.notify();
    }

    fn kicker(text: &'static str) -> impl IntoElement {
        div()
            .text_size(px(theme::type_scale::KICKER))
            .text_color(rgb(theme::color::SEA_GREEN))
            .child(text.to_uppercase())
    }
}

impl Render for Appeal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Local::now().naive_local();
        let clock = format_elapsed(self.data.last_relapse, now);

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(theme::space::GROUP_Y))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(theme::space::TIGHT))
                    .child(Self::kicker("since last relapse"))
                    .child(
                        div()
                            .text_size(px(theme::type_scale::CLOCK))
                            .font_weight(FontWeight(theme::weight::STATEMENT))
                            .text_color(rgb(theme::color::ACCENT))
                            .child(clock),
                    )
                    .child(
                        div()
                            .text_size(px(theme::type_scale::MINI))
                            .text_color(rgb(theme::color::MUTED))
                            .child("months : days : hours : minutes : seconds"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(theme::space::DOT_GAP))
                    .child(
                        div()
                            .text_size(px(theme::type_scale::HINT))
                            .text_color(rgb(theme::color::MUTED))
                            .child(format!(
                                "last relapse {}",
                                self.data.last_relapse.format("%d-%b-%Y %H:%M:%S")
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(theme::type_scale::MINI))
                            .text_color(rgb(theme::color::MUTED))
                            .child("the counter updates every second"),
                    ),
            )
            .child(
                div()
                    .pt(px(theme::space::TIGHT))
                    .border_t_1()
                    .border_color(rgb(theme::color::SURFACE))
                    .child(
                        div()
                            .id("appeal-reset")
                            .px(px(theme::space::ITEM_X))
                            .py(px(theme::space::ITEM_Y - 2.))
                            .bg(rgb(theme::color::SURFACE))
                            .text_size(px(theme::type_scale::NAV))
                            .text_color(rgb(theme::color::PURPLE))
                            .cursor_pointer()
                            .hover(|style| style.text_color(rgb(theme::color::BRIGHT_PURPLE)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.request_reset(cx)
                            }))
                            .child("record relapse / reset clock"),
                    ),
            )
            .when_some(self.status.clone(), |el, status| {
                el.child(
                    div()
                        .text_size(px(theme::type_scale::HINT))
                        .text_color(rgb(theme::color::YELLOW))
                        .child(status),
                )
            })
    }
}
