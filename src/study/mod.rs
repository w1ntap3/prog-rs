//! The `study` section: winter-semester calendar with attendance marking and
//! monthly attendance stats.

mod model;
mod schedule;

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use gpui::prelude::*;
use gpui::{AnyElement, ClickEvent, Context, FontWeight, Render, Window, div, px, rgb};

use crate::theme;

use self::model::{Attendance, StudyData};
use self::schedule::Session;

#[derive(Debug, Clone, Copy)]
pub struct Stat {
    pub attended: u32,
    pub total: u32,
}

impl Stat {
    fn text(self) -> String {
        if self.total == 0 {
            "0/0 (—)".to_owned()
        } else {
            let percent = (self.attended as f64 * 100.0 / self.total as f64).round() as u32;
            format!("{}/{} ({percent}%)", self.attended, self.total)
        }
    }
}

#[derive(Debug, Clone)]
pub struct StudySummary {
    pub overall: Stat,
    pub lectures: Stat,
    pub practice: Stat,
}

pub struct Study {
    data: StudyData,
    month: NaiveDate,
    status: Option<String>,
}

impl Study {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (data, status) = match StudyData::load_or_create() {
            Ok(data) => (data, None),
            Err(error) => (
                StudyData::default(),
                Some(format!("could not load study data: {error:#}")),
            ),
        };
        let months = schedule::semester_months();
        let today = Local::now().date_naive();
        let current = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
            .expect("first of month is valid");
        let month = if current < months[0] {
            months[0]
        } else if current > months[months.len() - 1] {
            months[months.len() - 1]
        } else {
            current
        };
        let this = Self {
            data,
            month,
            status,
        };
        this.start_tick(cx);
        this
    }

    fn start_tick(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_secs(60))
                .await;
            let alive = this.update(cx, |_, cx| cx.notify()).is_ok();
            if !alive {
                break;
            }
        })
        .detach();
    }

    /// Stats for the month currently shown on main (today's month, clamped to
    /// the known semester).
    pub fn current_summary(&self) -> StudySummary {
        let months = schedule::semester_months();
        let today = Local::now().date_naive();
        let current = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
            .expect("first of month is valid");
        let month = if current < months[0] {
            months[0]
        } else if current > months[months.len() - 1] {
            months[months.len() - 1]
        } else {
            current
        };
        self.summary_for(month)
    }

    pub fn summary_for(&self, month: NaiveDate) -> StudySummary {
        let now = Local::now().naive_local();
        let mut overall = Stat { attended: 0, total: 0 };
        let mut lectures = Stat { attended: 0, total: 0 };
        let mut practice = Stat { attended: 0, total: 0 };

        for session in schedule::sessions_in_month(month) {
            if session.start_at() > now {
                continue;
            }
            let attended = self.data.attendance.get(&session.key()) == Some(&Attendance::Attended);
            overall.total += 1;
            if attended {
                overall.attended += 1;
            }
            if session.kind.counts_as_lecture() {
                lectures.total += 1;
                if attended {
                    lectures.attended += 1;
                }
            } else if session.kind.counts_as_practice() {
                practice.total += 1;
                if attended {
                    practice.attended += 1;
                }
            }
        }

        StudySummary {
            overall,
            lectures,
            practice,
        }
    }

    /// Cycle a session: unmarked -> attended -> missed -> unmarked.
    fn cycle_attendance(&mut self, key: String, cx: &mut Context<Self>) {
        use Attendance::{Attended, Missed};
        let next = match self.data.attendance.get(&key) {
            None => Some(Attended),
            Some(Attended) => Some(Missed),
            Some(Missed) => None,
        };
        match next {
            Some(state) => {
                self.data.attendance.insert(key, state);
            }
            None => {
                self.data.attendance.remove(&key);
            }
        }
        self.status = match self.data.save() {
            Ok(()) => None,
            Err(error) => Some(format!("save failed: {error:#}")),
        };
        cx.notify();
    }

    fn move_month(&mut self, delta: i32, cx: &mut Context<Self>) {
        let months = schedule::semester_months();
        let Some(index) = months.iter().position(|month| *month == self.month) else {
            return;
        };
        let next = (index as i32 + delta).clamp(0, months.len() as i32 - 1) as usize;
        if next != index {
            self.month = months[next];
            cx.notify();
        }
    }

    fn month_nav(&self, cx: &Context<Self>) -> impl IntoElement {
        let months = schedule::semester_months();
        let index = months
            .iter()
            .position(|month| *month == self.month)
            .unwrap_or(0);
        let can_prev = index > 0;
        let can_next = index + 1 < months.len();

        let arrow = |label: &'static str,
                     id: &'static str,
                     enabled: bool,
                     on_click: Option<Box<dyn Fn(&mut Self, &mut Context<Self>)>>|
         -> AnyElement {
            let mut element = div()
                .id(id)
                .px(px(theme::space::DOT_GAP))
                .text_size(px(theme::type_scale::NAV))
                .text_color(rgb(if enabled {
                    theme::color::FOREGROUND
                } else {
                    theme::color::MUTED
                }));
            if let Some(handler) = on_click {
                element = element
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| handler(this, cx)));
            }
            element.child(label).into_any_element()
        };

        let prev = if can_prev {
            let handler = |this: &mut Self, cx: &mut Context<Self>| this.move_month(-1, cx);
            arrow(
                "‹",
                "study-month-prev",
                true,
                Some(Box::new(move |this, cx| handler(this, cx))),
            )
        } else {
            arrow("‹", "study-month-prev", false, None)
        };
        let next = if can_next {
            let handler = |this: &mut Self, cx: &mut Context<Self>| this.move_month(1, cx);
            arrow(
                "›",
                "study-month-next",
                true,
                Some(Box::new(move |this, cx| handler(this, cx))),
            )
        } else {
            arrow("›", "study-month-next", false, None)
        };

        div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(theme::space::TIGHT))
                    .child(prev)
                    .child(
                        div()
                            .text_size(px(theme::type_scale::NAV))
                            .font_weight(FontWeight(theme::weight::NAV_ACTIVE))
                            .text_color(rgb(theme::color::FOREGROUND))
                            .child(self.month.format("%B %Y").to_string().to_uppercase()),
                    )
                    .child(next),
            )
            .child(
                div()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child("winter semester · ends 10 Jan 2027"),
            )
    }

    fn session_row(
        &self,
        session: &Session,
        now: NaiveDateTime,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let key = session.key();
        let state = self.data.attendance.get(&key).copied();
        let markable = session.start_at() <= now;
        let kind_color = session.kind.color();

        let (accent, background, course_color) = if !markable {
            (
                theme::color::SURFACE,
                rgb(theme::color::SURFACE).alpha(0.12),
                theme::color::MUTED,
            )
        } else {
            match state {
                Some(Attendance::Attended) => (
                    theme::color::MINT,
                    rgb(theme::color::MINT).alpha(0.16),
                    theme::color::MINT,
                ),
                Some(Attendance::Missed) => (
                    theme::color::PURPLE,
                    rgb(theme::color::PURPLE).alpha(0.16),
                    theme::color::PURPLE,
                ),
                None => (
                    kind_color,
                    rgb(theme::color::SURFACE).alpha(0.18),
                    theme::color::FOREGROUND,
                ),
            }
        };

        let mut card = div()
            .id(format!("att-{key}"))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(theme::space::DOT_GAP / 2.0))
            .px(px(theme::space::DOT_GAP))
            .py(px(theme::space::DOT_GAP))
            .border_l_2()
            .border_color(rgb(accent))
            .bg(background)
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(theme::space::DOT_GAP))
                    .child(
                        div()
                            .whitespace_nowrap()
                            .text_size(px(theme::type_scale::MINI))
                            .text_color(rgb(theme::color::MUTED))
                            .child(session.start.format("%H:%M").to_string()),
                    )
                    .child(
                        div()
                            .whitespace_nowrap()
                            .px(px(theme::space::DOT_GAP / 2.0))
                            .text_size(px(theme::type_scale::MINI))
                            .text_color(rgb(kind_color))
                            .bg(rgb(kind_color).alpha(0.15))
                            .child(session.kind.short()),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .truncate()
                    .text_size(px(theme::type_scale::LEDGER))
                    .font_weight(FontWeight(theme::weight::NAV_ACTIVE))
                    .text_color(rgb(course_color))
                    .child(session.short),
            );
        if !markable {
            card = card.opacity(0.7);
        }
        if markable {
            card = card
                .cursor_pointer()
                .hover(|style| {
                    style
                        .bg(rgb(theme::color::SURFACE).alpha(0.5))
                        .border_color(rgb(theme::color::ACCENT))
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.cycle_attendance(key.clone(), cx)
                }));
        }
        card
    }

    fn calendar(&self, cx: &Context<Self>) -> impl IntoElement {
        let sessions = schedule::sessions_in_month(self.month);
        let mut by_day: BTreeMap<NaiveDate, Vec<Session>> = BTreeMap::new();
        for session in sessions {
            by_day.entry(session.date).or_default().push(session);
        }

        let first = NaiveDate::from_ymd_opt(self.month.year(), self.month.month(), 1)
            .expect("first of month is valid");
        let leading = match first.weekday().num_days_from_monday() {
            0..=4 => first.weekday().num_days_from_monday() as usize,
            _ => 0,
        };
        let days = schedule::days_in_month(first);
        let mut cells: Vec<Option<u32>> = vec![None; leading];
        for day in 1..=days {
            let date = NaiveDate::from_ymd_opt(self.month.year(), self.month.month(), day)
                .expect("calendar day is valid");
            if date.weekday().num_days_from_monday() < 5 {
                cells.push(Some(day));
            }
        }
        while cells.len() % 5 != 0 {
            cells.push(None);
        }
        let rows: Vec<Vec<Option<u32>>> = cells.chunks(5).map(<[Option<u32>]>::to_vec).collect();
        let today = Local::now().date_naive();
        let now = Local::now().naive_local();

        let weekdays = div()
            .w_full()
            .flex()
            .border_b_1()
            .border_color(rgb(theme::color::SURFACE))
            .children(
            ["MON", "TUE", "WED", "THU", "FRI"].map(|label| {
                div()
                    .flex_1()
                    .min_w_0()
                    .px(px(theme::space::DOT_GAP))
                    .pb(px(theme::space::DOT_GAP))
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child(label)
            }),
        );

        let grid = rows.into_iter().map(move |row| {
            div().w_full().flex().children(row.into_iter().map(|day| match day {
                None => div()
                    .flex_1()
                    .min_w_0()
                    .min_h(px(theme::geometry::CALENDAR_DAY_H))
                    .border_b_1()
                    .border_r_1()
                    .border_color(rgb(theme::color::SURFACE))
                    .overflow_x_hidden()
                    .into_any_element(),
                Some(day) => {
                    let date = NaiveDate::from_ymd_opt(self.month.year(), self.month.month(), day)
                        .expect("calendar day is valid");
                    let is_today = date == today;
                    let day_sessions = by_day.get(&date);
                    div()
                        .flex_1()
                        .min_w_0()
                        .min_h(px(theme::geometry::CALENDAR_DAY_H))
                        .border_b_1()
                        .border_r_1()
                        .border_color(rgb(theme::color::SURFACE))
                        .overflow_x_hidden()
                        .when(is_today, |el| {
                            el.bg(rgb(theme::color::ACCENT).alpha(0.06))
                        })
                        .px(px(theme::space::DOT_GAP))
                        .py(px(theme::space::DOT_GAP))
                        .flex()
                        .flex_col()
                        .gap(px(theme::space::DOT_GAP / 2.0))
                        .child(
                            div()
                                .text_size(px(theme::type_scale::HINT))
                                .font_weight(FontWeight(if is_today {
                                    theme::weight::NAV_ACTIVE
                                } else {
                                    theme::weight::NAV
                                }))
                                .text_color(rgb(if is_today {
                                    theme::color::ACCENT
                                } else {
                                    theme::color::MUTED
                                }))
                                .child(day.to_string()),
                        )
                        .children(
                            day_sessions
                                .into_iter()
                                .flatten()
                                .map(|session| self.session_row(session, now, cx)),
                        )
                        .into_any_element()
                }
            }))
        });

        div()
            .w_full()
            .flex()
            .flex_col()
            .border_t_1()
            .border_l_1()
            .border_color(rgb(theme::color::SURFACE))
            .child(weekdays)
            .children(grid)
    }
}

fn stat_block(label: &'static str, stat: Stat) -> impl IntoElement {
    div()
        .flex_1()
        .px(px(theme::space::TIGHT))
        .py(px(theme::space::TIGHT))
        .border_r_1()
        .border_color(rgb(theme::color::SURFACE))
        .flex()
        .flex_col()
        .gap(px(theme::space::DOT_GAP))
        .child(
            div()
                .text_size(px(theme::type_scale::MINI))
                .text_color(rgb(theme::color::MUTED))
                .child(label),
        )
        .child(
            div()
                .text_size(px(theme::type_scale::STAT))
                .font_weight(FontWeight(theme::weight::NAV_ACTIVE))
                .text_color(rgb(theme::color::FOREGROUND))
                .child(stat.text()),
        )
}

/// The same three-number stats row shown on the study tab.
pub fn summary_row(summary: StudySummary) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .border_t_1()
        .border_b_1()
        .border_color(rgb(theme::color::SURFACE))
        .child(stat_block("ALL CLASSES", summary.overall))
        .child(stat_block("LECTURES", summary.lectures))
        .child(stat_block("LABS & SEMINARS", summary.practice))
}

impl Render for Study {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let summary = self.summary_for(self.month);

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(theme::space::TIGHT))
            .child(self.month_nav(cx))
            .child(summary_row(summary))
            .child(self.calendar(cx))
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
