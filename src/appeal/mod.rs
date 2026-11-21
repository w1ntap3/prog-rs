//! The `appeal` section: a relapse-free clock backed by `appeal.toml`.

mod model;

use std::time::Duration;

use chrono::{Local, NaiveDateTime};
use gpui::prelude::*;
use gpui::{ClickEvent, Context, FontWeight, Render, Window, div, px, rgb};

use crate::theme;

use self::model::{AppealData, now_seconds};

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

    pub fn breakdown(&self) -> (i64, i64, i64, i64, i64) {
        model::elapsed_breakdown(self.data.last_relapse, Local::now().naive_local())
    }

    pub fn since(&self) -> NaiveDateTime {
        self.data.last_relapse
    }
}

/// Shared clock body used by the appeal tab and the informational copy on main.
pub fn clock_view(
    (months, days, hours, minutes, seconds): (i64, i64, i64, i64, i64),
    since: NaiveDateTime,
) -> impl IntoElement {
    let cell = |value: i64, label: &'static str| {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(theme::space::DOT_GAP))
            .child(
                div()
                    .text_size(px(theme::type_scale::CLOCK_DIGIT))
                    .font_weight(FontWeight(theme::weight::STATEMENT))
                    .text_color(rgb(theme::color::ACCENT))
                    .child(format!("{value:02}")),
            )
            .child(
                div()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child(label),
            )
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(theme::space::TIGHT))
        .child(
            div()
                .text_size(px(theme::type_scale::KICKER))
                .text_color(rgb(theme::color::SEA_GREEN))
                .child("RELAPSE-FREE"),
        )
        .child(
            div()
                .text_size(px(theme::type_scale::MINI))
                .text_color(rgb(theme::color::MUTED))
                .child(format!("since {}", since.format("%d-%b-%Y %H:%M"))),
        )
        .child(
            div()
                .mt(px(theme::space::TIGHT))
                .flex()
                .items_end()
                .justify_center()
                .gap(px(theme::space::TIGHT))
                .child(cell(months, "MONTHS"))
                .child(cell(days, "DAYS"))
                .child(cell(hours, "HOURS"))
                .child(cell(minutes, "MINUTES"))
                .child(cell(seconds, "SECONDS")),
        )
}

impl Render for Appeal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fields = model::elapsed_breakdown(
            self.data.last_relapse,
            Local::now().naive_local(),
        );
        let since = self.data.last_relapse;

        div()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(theme::space::TIGHT))
            .pt(px(theme::space::GROUP_Y))
            .child(
                div()
                    .w(px(theme::geometry::CLOCK_BOX))
                    .h(px(theme::geometry::CLOCK_BOX))
                    .border_1()
                    .border_color(rgb(theme::color::SURFACE))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(theme::space::GROUP_Y))
                    .child(clock_view(fields, since))
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
                            .child("RESET CLOCK"),
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
