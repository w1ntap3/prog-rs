//! Reusable date and time picker popovers.
//!
//! The owning view keeps the selected value, the visible calendar month and
//! the popover anchor; these helpers only render the calendar/time grid and
//! route clicks back through callbacks. Popovers are deferred overlays so they
//! escape scroll containers and stay clamped to the window.

use std::rc::Rc;

use chrono::{Datelike, Local, NaiveDate, NaiveTime, Timelike};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, MouseButton, MouseDownEvent, Point, Window, anchored, deferred, div,
    point, px, rgb,
};

use crate::theme;

/// First day of the month containing `date`.
pub fn first_of_month(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).expect("first of month is valid")
}

/// Shift a date by whole months, clamping the day to the target month.
pub fn add_months(date: NaiveDate, delta: i32) -> NaiveDate {
    let index = date.year() * 12 + date.month0() as i32 + delta;
    let year = index.div_euclid(12);
    let month = (index.rem_euclid(12) + 1) as u32;
    let day = date.day().min(days_in_month_of(year, month));
    NaiveDate::from_ymd_opt(year, month, day).expect("clamped date is valid")
}

/// Number of days in the month containing `date`.
pub fn days_in_month(date: NaiveDate) -> u32 {
    days_in_month_of(date.year(), date.month())
}

fn days_in_month_of(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("first of month is valid")
        .pred_opt()
        .expect("first of month has a predecessor")
        .day()
}

/// Calendar popover. `on_shift` moves the visible month, `on_pick` selects a
/// day. Callers typically chain into a time popover from `on_pick`.
pub fn calendar<V: 'static>(
    month: NaiveDate,
    selected: NaiveDate,
    id: &'static str,
    cx: &Context<V>,
    on_shift: Rc<dyn Fn(&mut V, i32, &mut Context<V>)>,
    on_pick: Rc<dyn Fn(&mut V, NaiveDate, &mut Context<V>)>,
) -> AnyElement {
    let today = Local::now().date_naive();
    let leading = month.weekday().num_days_from_monday() as usize;
    let mut cells: Vec<Option<u32>> = vec![None; leading];
    cells.extend((1..=days_in_month(month)).map(Some));
    while cells.len() % 7 != 0 {
        cells.push(None);
    }
    let rows: Vec<Vec<Option<u32>>> = cells.chunks(7).map(<[Option<u32>]>::to_vec).collect();

    let shift = |label: &'static str, delta: i32, id: &'static str| {
        let on_shift = on_shift.clone();
        div()
            .id(format!("{id}-shift-{delta}"))
            .px(px(theme::space::DOT_GAP))
            .text_size(px(theme::type_scale::NAV))
            .text_color(rgb(theme::color::MUTED))
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
            .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                on_shift(this, delta, cx)
            }))
            .child(label)
    };

    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .child(shift("‹", -1, id))
        .child(
            div()
                .text_size(px(theme::type_scale::NAV))
                .text_color(rgb(theme::color::FOREGROUND))
                .child(month.format("%B %Y").to_string()),
        )
        .child(shift("›", 1, id));

    let weekdays = div().flex().children(
        ["mo", "tu", "we", "th", "fr", "sa", "su"].map(|label| {
            div()
                .w(px(theme::geometry::CAL_CELL))
                .flex()
                .justify_center()
                .text_size(px(theme::type_scale::MINI))
                .text_color(rgb(theme::color::MUTED))
                .child(label)
        }),
    );

    let grid = rows.into_iter().map(move |row| {
        let on_pick = on_pick.clone();
        div().flex().children(row.into_iter().map(move |day| match day {
            None => div()
                .w(px(theme::geometry::CAL_CELL))
                .h(px(theme::geometry::CAL_ROW))
                .into_any_element(),
            Some(day) => {
                let date = NaiveDate::from_ymd_opt(month.year(), month.month(), day)
                    .expect("calendar day is valid");
                let is_selected = date == selected;
                let is_today = date == today;
                let on_pick = on_pick.clone();
                div()
                    .id(format!("{id}-day-{day}"))
                    .w(px(theme::geometry::CAL_CELL))
                    .h(px(theme::geometry::CAL_ROW))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(if is_selected {
                        theme::color::ACCENT
                    } else if is_today {
                        theme::color::FOREGROUND
                    } else {
                        theme::color::MUTED
                    }))
                    .when(is_selected, |el| el.bg(rgb(theme::color::SURFACE)))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        on_pick(this, date, cx)
                    }))
                    .child(day.to_string())
                    .into_any_element()
            }
        }))
    });

    div()
        .flex()
        .flex_col()
        .gap(px(theme::space::TIGHT))
        .p(px(theme::space::TIGHT))
        .bg(rgb(theme::color::BACKGROUND))
        .occlude()
        .border_1()
        .border_color(rgb(theme::color::SURFACE))
        .w(px(theme::geometry::CAL_CELL * 7.0 + theme::space::TIGHT * 2.0))
        .max_w_full()
        .child(header)
        .child(weekdays)
        .children(grid)
        .into_any_element()
}

/// Time popover: hours, then minutes in five-minute steps. Picking a minute
/// should normally close the popover; `on_done` closes it without changing the
/// minute (useful when only the hour matters).
pub fn time_panel<V: 'static>(
    selected: NaiveTime,
    id: &'static str,
    cx: &Context<V>,
    on_hour: Rc<dyn Fn(&mut V, u32, &mut Context<V>)>,
    on_minute: Rc<dyn Fn(&mut V, u32, &mut Context<V>)>,
    on_done: Rc<dyn Fn(&mut V, &mut Context<V>)>,
) -> AnyElement {
    let hour = selected.hour();
    let minute = selected.minute();

    let hour_grid = div()
        .flex()
        .flex_wrap()
        .w(px(theme::geometry::TIME_CELL * 6.0))
        .children((0..24).map(|h| {
            let is_selected = h == hour;
            let on_hour = on_hour.clone();
            div()
                .id(format!("{id}-hour-{h}"))
                .w(px(theme::geometry::TIME_CELL))
                .h(px(theme::geometry::CAL_ROW))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(theme::type_scale::MINI))
                .text_color(rgb(if is_selected {
                    theme::color::ACCENT
                } else {
                    theme::color::MUTED
                }))
                .when(is_selected, |el| el.bg(rgb(theme::color::SURFACE)))
                .cursor_pointer()
                .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                    on_hour(this, h, cx)
                }))
                .child(format!("{h:02}"))
        }));

    let minute_grid = div()
        .flex()
        .flex_wrap()
        .w(px(theme::geometry::TIME_CELL * 6.0))
        .children((0..60).step_by(5).map(|m| {
            let is_selected = m == minute;
            let on_minute = on_minute.clone();
            div()
                .id(format!("{id}-minute-{m}"))
                .w(px(theme::geometry::TIME_CELL))
                .h(px(theme::geometry::CAL_ROW))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(theme::type_scale::MINI))
                .text_color(rgb(if is_selected {
                    theme::color::ACCENT
                } else {
                    theme::color::MUTED
                }))
                .when(is_selected, |el| el.bg(rgb(theme::color::SURFACE)))
                .cursor_pointer()
                .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                    on_minute(this, m, cx)
                }))
                .child(format!("{m:02}"))
        }));

    div()
        .flex()
        .flex_col()
        .gap(px(theme::space::TIGHT))
        .p(px(theme::space::TIGHT))
        .bg(rgb(theme::color::BACKGROUND))
        .occlude()
        .border_1()
        .border_color(rgb(theme::color::SURFACE))
        .w(px(theme::geometry::TIME_CELL * 6.0 + theme::space::TIGHT * 2.0))
        .max_w_full()
        .child(
            div()
                .text_size(px(theme::type_scale::KICKER))
                .text_color(rgb(theme::color::SEA_GREEN))
                .child("hour"),
        )
        .child(hour_grid)
        .child(
            div()
                .text_size(px(theme::type_scale::KICKER))
                .text_color(rgb(theme::color::SEA_GREEN))
                .child("minute"),
        )
        .child(minute_grid)
        .child(
            div()
                .id(format!("{id}-done"))
                .text_size(px(theme::type_scale::NAV))
                .text_color(rgb(theme::color::ACCENT))
                .cursor_pointer()
                .hover(|style| style.text_color(rgb(theme::color::FOREGROUND)))
                .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                    on_done(this, cx)
                }))
                .child("done"),
        )
        .into_any_element()
}

/// Full-window click layer that closes the popover.
pub fn backdrop<V: 'static>(
    window: &Window,
    id: &'static str,
    cx: &Context<V>,
    close: Rc<dyn Fn(&mut V, &mut Context<V>)>,
) -> AnyElement {
    let viewport = window.viewport_size();
    deferred(
        anchored().position(point(px(0.), px(0.))).child(
            div()
                .id(id)
                .w(px(viewport.width.as_f32()))
                .h(px(viewport.height.as_f32()))
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| close(this, cx)),
                ),
        ),
    )
    .with_priority(1)
    .into_any_element()
}

/// Floating popover clamped to the window around `anchor`.
pub fn overlay(anchor: Point<gpui::Pixels>, panel: AnyElement) -> AnyElement {
    deferred(
        anchored()
            .snap_to_window_with_margin(px(theme::space::TIGHT))
            .position(anchor)
            .offset(point(px(0.), px(theme::space::DOT_GAP)))
            .child(panel),
    )
    .with_priority(2)
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_math_clamps_and_wraps() {
        let jan = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        assert_eq!(add_months(jan, 1), NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
        assert_eq!(add_months(jan, 12), NaiveDate::from_ymd_opt(2027, 1, 31).unwrap());
        assert_eq!(first_of_month(jan), NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(days_in_month(jan), 31);
    }
}
