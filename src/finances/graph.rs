//! Custom-painted "money left vs baseline" chart.
//!
//! Flat style: two stroked polylines (baseline muted, actual accent), a hairline
//! at zero and a hairline at today. Labels are regular divs on top of the
//! canvas, so they stay selectable/consistent with the rest of the type scale.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    Bounds, IntoElement, PathBuilder, Pixels, Point, Window, canvas, div, point, px, rgb,
};
use gpui::prelude::*;

use super::stats::Summary;
use crate::theme;

struct ChartLayout {
    plan: Vec<Point<Pixels>>,
    actual: Vec<Point<Pixels>>,
    today: Point<Pixels>,
    zero_y: Pixels,
}

/// The visible Y range always contains the baseline budget and every point the
/// actual line reaches, so a single huge gain or expense can never draw outside
/// the chart.
fn axis_range(summary: &Summary) -> (f32, f32) {
    let mut low = 0i64;
    let mut high = 0i64;
    for (_, money) in summary.points.iter() {
        low = low.min(*money);
        high = high.max(*money);
    }
    high = high
        .max(summary.budget)
        .max(summary.available)
        .max(summary.remaining)
        .max(summary.plan_amount)
        .max(0);
    let low = low as f32;
    let high = high as f32;
    if high - low < 1.0 {
        (low, low + 1.0)
    } else {
        (low, high)
    }
}

impl ChartLayout {
    fn compute(bounds: Bounds<Pixels>, summary: &Summary) -> Self {
        let pad_x = theme::space::CHART_PAD_X;
        let pad_y = theme::space::CHART_PAD_Y;
        let left = bounds.origin.x.as_f32() + pad_x;
        let right = bounds.origin.x.as_f32() + bounds.size.width.as_f32() - pad_x;
        let top = bounds.origin.y.as_f32() + pad_y;
        let bottom = bounds.origin.y.as_f32() + bounds.size.height.as_f32() - pad_y;

        let total = summary.total_days.max(1) as f32;
        let (low, high) = axis_range(summary);
        let span = (high - low).max(1.0);

        let x = |day: f32| left + (right - left) * (day / total);
        let y = |money: f32| bottom - (bottom - top) * ((money - low) / span);

        let plan = if summary.plan_available && summary.plan_start_x < total {
            vec![
                point(
                    px(x(summary.plan_start_x.min(total))),
                    px(y(summary.plan_amount as f32)),
                ),
                point(px(x(total)), px(y(0.0))),
            ]
        } else {
            Vec::new()
        };

        let actual = summary
            .points
            .iter()
            .map(|(day, money)| point(px(x(*day as f32)), px(y(*money as f32))))
            .collect();

        Self {
            plan,
            actual,
            today: point(px(x(summary.day_index as f32)), px(top)),
            zero_y: px(y(0.0)),
        }
    }
}

pub fn plot(
    summary: &Summary,
    bounds_cell: Rc<Cell<Bounds<Pixels>>>,
) -> impl IntoElement {
    let summary = summary.clone();
    let canvas_summary = summary.clone();
    let chart = canvas(
        move |bounds, _, _| {
            bounds_cell.set(bounds);
            ChartLayout::compute(bounds, &canvas_summary)
        },
        |bounds, layout, window, _| {
            let plan_color = rgb(theme::color::YELLOW);
            let surface = rgb(theme::color::SURFACE);
            let accent = rgb(theme::color::ACCENT);

            // Zero line.
            let left = bounds.origin.x.as_f32() + theme::space::CHART_PAD_X;
            let right = bounds.origin.x.as_f32() + bounds.size.width.as_f32() - theme::space::CHART_PAD_X;
            let mut zero = PathBuilder::stroke(px(theme::geometry::HAIRLINE));
            zero.move_to(point(px(left), layout.zero_y));
            zero.line_to(point(px(right), layout.zero_y));
            if let Ok(path) = zero.build() {
                window.paint_path(path, surface);
            }

            // Today marker.
            let mut today = PathBuilder::stroke(px(theme::geometry::HAIRLINE));
            today.move_to(layout.today);
            today.line_to(point(
                layout.today.x,
                px(bounds.origin.y.as_f32() + bounds.size.height.as_f32() - theme::space::CHART_PAD_Y),
            ));
            if let Ok(path) = today.build() {
                window.paint_path(path, surface);
            }

            paint_line(&layout.plan, plan_color, window);
            paint_line(&layout.actual, accent, window);
        },
    );

    let (low, high) = axis_range(&summary);
    let setup_notice = if summary.available > 0 {
        None
    } else {
        Some("set this category's balance to see the baseline")
    };

    div()
        .relative()
        .size_full()
        .child(chart.absolute().inset_0())
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .text_size(px(theme::type_scale::MINI))
                .text_color(rgb(theme::color::MUTED))
                .child(super::stats::money(high.round() as i64)),
        )
        .child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .text_size(px(theme::type_scale::MINI))
                .text_color(rgb(theme::color::MUTED))
                .child(super::stats::money(low.round() as i64)),
        )
        .when_some(setup_notice, |el, notice| {
            el.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child(notice),
            )
        })
}

fn paint_line(points: &[Point<Pixels>], color: impl Into<gpui::Background>, window: &mut Window) {
    let mut points = points.iter();
    let Some(first) = points.next() else { return };
    let mut path = PathBuilder::stroke(px(theme::geometry::LINE_W));
    path.move_to(*first);
    for point in points {
        path.line_to(*point);
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
