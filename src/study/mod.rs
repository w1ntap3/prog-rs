//! The `study` section: winter-semester calendar with attendance marking and
//! monthly attendance stats.

mod model;
mod schedule;

use std::rc::Rc;
use std::time::Duration;

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, FontWeight, KeyDownEvent, Point,
    Render, Window, div, point, px, rgb,
};

use crate::datetime_picker;
use crate::theme;

use self::model::{Assessment, Assignment, Attendance, StudyData};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Assignments,
    Assessments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    AssignmentName,
    AssessmentName,
}

/// Which entry's date/time the picker is editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Assignment,
    Assessment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Picker {
    Date(Target),
    Time(Target),
}

struct FocusHandles {
    assignment_name: gpui::FocusHandle,
    assessment_name: gpui::FocusHandle,
}

pub struct Study {
    data: StudyData,
    week: NaiveDate,
    status: Option<String>,
    assignment_subject: String,
    assessment_subject: String,
    assignment_name: String,
    assessment_name: String,
    assignment_at: NaiveDateTime,
    assessment_at: NaiveDateTime,
    open_subject: Option<Panel>,
    picker: Option<Picker>,
    picker_anchor: Point<gpui::Pixels>,
    cal_month: NaiveDate,
    focus: FocusHandles,
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
        let weeks = schedule::active_weeks();
        let today = Local::now().date_naive();
        let current = schedule::monday_of(today);
        let week = if current < weeks[0] {
            weeks[0]
        } else if current > weeks[weeks.len() - 1] {
            weeks[weeks.len() - 1]
        } else {
            current
        };
        let subjects = schedule::subjects();
        let default_subject = subjects.first().copied().unwrap_or("PRS").to_owned();
        let now = Local::now().naive_local();
        let this = Self {
            data,
            week,
            status,
            assignment_subject: default_subject.clone(),
            assessment_subject: default_subject,
            assignment_name: String::new(),
            assessment_name: String::new(),
            assignment_at: now,
            assessment_at: now,
            open_subject: None,
            picker: None,
            picker_anchor: point(px(0.), px(0.)),
            cal_month: datetime_picker::first_of_month(now.date()),
            focus: FocusHandles {
                assignment_name: cx.focus_handle(),
                assessment_name: cx.focus_handle(),
            },
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
        summarize_at(&self.data, month, Local::now().naive_local())
    }

    fn field_value(&self, field: Field) -> &str {
        match field {
            Field::AssignmentName => &self.assignment_name,
            Field::AssessmentName => &self.assessment_name,
        }
    }

    fn field_value_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::AssignmentName => &mut self.assignment_name,
            Field::AssessmentName => &mut self.assessment_name,
        }
    }

    fn focus_handle(&self, field: Field) -> &gpui::FocusHandle {
        match field {
            Field::AssignmentName => &self.focus.assignment_name,
            Field::AssessmentName => &self.focus.assessment_name,
        }
    }

    fn focus_field(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle(field).focus(window, cx);
        cx.notify();
    }

    fn key(
        &mut self,
        field: Field,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        if key == "tab" {
            let next = match field {
                Field::AssignmentName => Field::AssessmentName,
                Field::AssessmentName => Field::AssignmentName,
            };
            self.focus_field(next, window, cx);
            return;
        }
        if key == "enter" {
            match field {
                Field::AssignmentName => self.add_assignment(cx),
                Field::AssessmentName => self.add_assessment(cx),
            }
            return;
        }
        if key == "backspace" {
            let value = self.field_value_mut(field);
            if modifiers.control || modifiers.platform {
                value.clear();
            } else {
                value.pop();
            }
            cx.notify();
            return;
        }
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        let Some(chars) = event.keystroke.key_char.as_deref() else {
            return;
        };
        self.field_value_mut(field).push_str(chars);
        cx.notify();
    }

    fn toggle_subject(&mut self, panel: Panel, cx: &mut Context<Self>) {
        self.open_subject = if self.open_subject == Some(panel) {
            None
        } else {
            Some(panel)
        };
        cx.notify();
    }

    fn subject(&self, panel: Panel) -> &str {
        match panel {
            Panel::Assignments => &self.assignment_subject,
            Panel::Assessments => &self.assessment_subject,
        }
    }

    fn set_subject(&mut self, panel: Panel, subject: &str, cx: &mut Context<Self>) {
        match panel {
            Panel::Assignments => self.assignment_subject = subject.to_owned(),
            Panel::Assessments => self.assessment_subject = subject.to_owned(),
        }
        self.open_subject = None;
        cx.notify();
    }

    fn datetime(&self, target: Target) -> NaiveDateTime {
        match target {
            Target::Assignment => self.assignment_at,
            Target::Assessment => self.assessment_at,
        }
    }

    fn set_datetime(&mut self, target: Target, value: NaiveDateTime) {
        match target {
            Target::Assignment => self.assignment_at = value,
            Target::Assessment => self.assessment_at = value,
        }
    }

    fn toggle_datetime_picker(
        &mut self,
        target: Target,
        position: Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let active = matches!(
            self.picker,
            Some(Picker::Date(active) | Picker::Time(active)) if active == target
        );
        if active {
            self.picker = None;
        } else {
            self.cal_month = datetime_picker::first_of_month(self.datetime(target).date());
            self.picker_anchor = position;
            self.picker = Some(Picker::Date(target));
        }
        cx.notify();
    }

    fn choose_date(&mut self, target: Target, date: NaiveDate, cx: &mut Context<Self>) {
        let time = self.datetime(target).time();
        self.set_datetime(target, date.and_time(time));
        self.cal_month = datetime_picker::first_of_month(date);
        // Straight into the time step so a full value needs no second click.
        self.picker = Some(Picker::Time(target));
        cx.notify();
    }

    fn choose_hour(&mut self, target: Target, hour: u32, cx: &mut Context<Self>) {
        let minute = self.datetime(target).minute();
        let time = NaiveTime::from_hms_opt(hour, minute, 0).expect("hour and minute are valid");
        self.set_datetime(target, self.datetime(target).date().and_time(time));
        cx.notify();
    }

    fn choose_minute(&mut self, target: Target, minute: u32, cx: &mut Context<Self>) {
        let hour = self.datetime(target).hour();
        let time = NaiveTime::from_hms_opt(hour, minute, 0).expect("hour and minute are valid");
        self.set_datetime(target, self.datetime(target).date().and_time(time));
        self.picker = None;
        cx.notify();
    }

    fn shift_calendar(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.cal_month = datetime_picker::add_months(self.cal_month, delta);
        cx.notify();
    }

    fn close_picker(&mut self, cx: &mut Context<Self>) {
        self.picker = None;
        cx.notify();
    }

    fn add_assignment(&mut self, cx: &mut Context<Self>) {
        let deadline = self.assignment_at;
        let name = self.assignment_name.trim().to_owned();
        if name.is_empty() {
            self.status = Some("give the assignment a name first".into());
            cx.notify();
            return;
        }
        self.data.assignments.push(Assignment {
            name,
            subject: self.assignment_subject.clone(),
            deadline,
        });
        self.assignment_name.clear();
        self.assignment_at = Local::now().naive_local();
        self.open_subject = None;
        self.picker = None;
        self.persist(cx);
    }

    fn add_assessment(&mut self, cx: &mut Context<Self>) {
        let when = self.assessment_at;
        let name = self.assessment_name.trim().to_owned();
        if name.is_empty() {
            self.status = Some("give the assessment a name first".into());
            cx.notify();
            return;
        }
        self.data.assessments.push(Assessment {
            name,
            subject: self.assessment_subject.clone(),
            when,
        });
        self.assessment_name.clear();
        self.assessment_at = Local::now().naive_local();
        self.open_subject = None;
        self.picker = None;
        self.persist(cx);
    }

    fn persist(&mut self, cx: &mut Context<Self>) {
        self.status = match self.data.save() {
            Ok(()) => None,
            Err(error) => Some(format!("save failed: {error:#}")),
        };
        cx.notify();
    }

    fn delete_assignment(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.data.assignments.len() {
            self.data.assignments.remove(index);
            self.persist(cx);
        }
    }

    fn delete_assessment(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.data.assessments.len() {
            self.data.assessments.remove(index);
            self.persist(cx);
        }
    }

    /// Cycle a session: unmarked -> attended -> missed -> cancelled -> unmarked.
    /// `cancelled` is only reachable once the session has ended.
    fn cycle_attendance(&mut self, key: String, allow_cancelled: bool, cx: &mut Context<Self>) {
        use Attendance::{Attended, Cancelled, Missed};
        let next = match self.data.attendance.get(&key) {
            None => Some(Attended),
            Some(Attended) => Some(Missed),
            Some(Missed) if allow_cancelled => Some(Cancelled),
            Some(Missed) | Some(Cancelled) => None,
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

    fn move_week(&mut self, delta: i32, cx: &mut Context<Self>) {
        let weeks = schedule::active_weeks();
        let Some(index) = weeks.iter().position(|week| *week == self.week) else {
            return;
        };
        let next = (index as i32 + delta).clamp(0, weeks.len() as i32 - 1) as usize;
        if next != index {
            self.week = weeks[next];
            cx.notify();
        }
    }

    fn week_nav(&self, cx: &Context<Self>) -> impl IntoElement {
        let weeks = schedule::active_weeks();
        let index = weeks
            .iter()
            .position(|week| *week == self.week)
            .unwrap_or(0);
        let can_prev = index > 0;
        let can_next = index + 1 < weeks.len();

        let arrow = |label: &'static str,
                     id: &'static str,
                     enabled: bool,
                     on_click: Option<Box<dyn Fn(&mut Self, &mut Context<Self>)>>|
         -> AnyElement {
            let mut element = div()
                .id(id)
                .w(px(theme::geometry::NAV_BUTTON))
                .h(px(theme::geometry::NAV_BUTTON))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(rgb(theme::color::SURFACE))
                .text_size(px(theme::type_scale::NAV))
                .text_color(rgb(if enabled {
                    theme::color::FOREGROUND
                } else {
                    theme::color::MUTED
                }));
            if let Some(handler) = on_click {
                element = element
                    .cursor_pointer()
                    .hover(|style| {
                        style
                            .bg(rgb(theme::color::SURFACE))
                            .text_color(rgb(theme::color::ACCENT))
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| handler(this, cx)));
            }
            element.child(label).into_any_element()
        };

        let prev = if can_prev {
            let handler = |this: &mut Self, cx: &mut Context<Self>| this.move_week(-1, cx);
            arrow(
                "‹",
                "study-week-prev",
                true,
                Some(Box::new(move |this, cx| handler(this, cx))),
            )
        } else {
            arrow("‹", "study-week-prev", false, None)
        };
        let next = if can_next {
            let handler = |this: &mut Self, cx: &mut Context<Self>| this.move_week(1, cx);
            arrow(
                "›",
                "study-week-next",
                true,
                Some(Box::new(move |this, cx| handler(this, cx))),
            )
        } else {
            arrow("›", "study-week-next", false, None)
        };

        let end = schedule::week_end(self.week);
        let label = if self.week.month() == end.month() {
            format!("{}–{}", self.week.format("%d"), end.format("%d %b %Y"))
        } else {
            format!(
                "{} – {}",
                self.week.format("%d %b"),
                end.format("%d %b %Y")
            )
        };

        div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(theme::space::TIGHT))
            .child(prev)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(theme::space::DOT_GAP))
                    .child(
                        div()
                            .text_size(px(theme::type_scale::NAV))
                            .font_weight(FontWeight(theme::weight::NAV_ACTIVE))
                            .text_color(rgb(theme::color::FOREGROUND))
                            .child(label.to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(px(theme::type_scale::MINI))
                            .text_color(rgb(theme::color::MUTED))
                            .child("winter semester · ends 10 Jan 2027"),
                    ),
            )
            .child(next)
    }

    fn text_field(
        &self,
        field: Field,
        placeholder: &str,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let focus = self.focus_handle(field);
        let focused = focus.is_focused(window);
        let focus = focus.clone();
        let value = self.field_value(field).to_owned();
        let empty = value.is_empty();
        div()
            .id(format!("study-field-{field:?}"))
            .w_full()
            .border_1()
            .border_color(rgb(if focused {
                theme::color::ACCENT
            } else {
                theme::color::SURFACE
            }))
            .bg(rgb(theme::color::SURFACE).alpha(0.12))
            .cursor_text()
            .track_focus(&focus)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.focus_field(field, window, cx)
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.key(field, event, window, cx)
            }))
            .child(
                div()
                    .relative()
                    .w_full()
                    .px(px(theme::space::FIELD_X))
                    .py(px(theme::space::FIELD_Y))
                    .flex()
                    .items_center()
                    .text_size(px(theme::type_scale::FIELD))
                    .text_color(rgb(theme::color::FOREGROUND))
                    .when(empty, |el| {
                        el.child(
                            div()
                                .absolute()
                                .inset_0()
                                .px(px(theme::space::FIELD_X))
                                .py(px(theme::space::FIELD_Y))
                                .text_size(px(theme::type_scale::FIELD))
                                .text_color(rgb(theme::color::MUTED))
                                .child(placeholder.to_owned()),
                        )
                    })
                    .child(div().child(value))
                    .when(focused, |el| {
                        el.child(
                            div()
                                .w(px(theme::geometry::HAIRLINE))
                                .h(px(theme::type_scale::FIELD + 2.0))
                                .bg(rgb(theme::color::ACCENT))
                                .with_animation(
                                    format!("study-caret-{field:?}"),
                                    Animation::new(Duration::from_millis(900)).repeat(),
                                    |el, t| el.opacity(if t < 0.5 { 1.0 } else { 0.0 }),
                                ),
                        )
                    }),
            )
    }

    fn datetime_button(&self, target: Target, cx: &Context<Self>) -> impl IntoElement {
        let open = matches!(
            self.picker,
            Some(Picker::Date(active) | Picker::Time(active)) if active == target
        );
        let at = self.datetime(target);
        div()
            .id(format!("study-datetime-{target:?}"))
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(theme::space::TIGHT))
            .px(px(theme::space::FIELD_X))
            .py(px(theme::space::FIELD_Y))
            .border_1()
            .border_color(rgb(if open {
                theme::color::ACCENT
            } else {
                theme::color::SURFACE
            }))
            .bg(rgb(theme::color::SURFACE).alpha(0.12))
            .text_size(px(theme::type_scale::FIELD))
            .text_color(rgb(theme::color::FOREGROUND))
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                this.toggle_datetime_picker(target, event.position(), cx)
            }))
            .child(at.format("%d-%b-%Y").to_string())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(theme::space::DOT_GAP))
                    .child(
                        div()
                            .text_color(rgb(theme::color::ACCENT))
                            .child(at.format("%H:%M").to_string()),
                    )
                    .child(
                        div()
                            .text_color(rgb(theme::color::MUTED))
                            .child("▾"),
                    ),
            )
    }

    fn picker_backdrop(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        self.picker.map(|_| {
            datetime_picker::backdrop(
                window,
                "study-picker-backdrop",
                cx,
                Rc::new(|this: &mut Self, cx: &mut Context<Self>| this.close_picker(cx)),
            )
        })
    }

    fn picker_overlay(&self, cx: &Context<Self>) -> Option<AnyElement> {
        self.picker.map(|picker| match picker {
            Picker::Date(target) => datetime_picker::overlay(
                self.picker_anchor,
                datetime_picker::calendar(
                    self.cal_month,
                    self.datetime(target).date(),
                    "study-calendar",
                    cx,
                    Rc::new(|this: &mut Self, delta, cx| this.shift_calendar(delta, cx)),
                    Rc::new(move |this: &mut Self, date, cx| this.choose_date(target, date, cx)),
                ),
            ),
            Picker::Time(target) => datetime_picker::overlay(
                self.picker_anchor,
                datetime_picker::time_panel(
                    self.datetime(target).time(),
                    "study-time",
                    cx,
                    Rc::new(move |this: &mut Self, hour, cx| this.choose_hour(target, hour, cx)),
                    Rc::new(move |this: &mut Self, minute, cx| {
                        this.choose_minute(target, minute, cx)
                    }),
                    Rc::new(|this: &mut Self, cx| this.close_picker(cx)),
                ),
            ),
        })
    }

    fn list_panel(
        &self,
        panel: Panel,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let (title, hint, entries): (
            &'static str,
            &'static str,
            Vec<(usize, String, String, NaiveDateTime)>,
        ) = match panel {
            Panel::Assignments => (
                "ASSIGNMENTS",
                "deadline · pick a date, then a time",
                self.data
                    .assignments
                    .iter()
                    .enumerate()
                    .map(|(index, assignment)| {
                        (
                            index,
                            assignment.name.clone(),
                            assignment.subject.clone(),
                            assignment.deadline,
                        )
                    })
                    .collect(),
            ),
            Panel::Assessments => (
                "ASSESSMENTS",
                "when it is · pick a date, then a time",
                self.data
                    .assessments
                    .iter()
                    .enumerate()
                    .map(|(index, assessment)| {
                        (
                            index,
                            assessment.name.clone(),
                            assessment.subject.clone(),
                            assessment.when,
                        )
                    })
                    .collect(),
            ),
        };
        let mut entries = entries;
        entries.sort_by_key(|(_, _, _, at)| *at);
        let empty = entries.is_empty();
        let subject = self.subject(panel).to_owned();
        let open = self.open_subject == Some(panel);
        let target = match panel {
            Panel::Assignments => Target::Assignment,
            Panel::Assessments => Target::Assessment,
        };

        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(theme::space::TIGHT))
            .p(px(theme::space::TIGHT))
            .border_1()
            .border_color(rgb(theme::color::SURFACE))
            .child(
                div()
                    .text_size(px(theme::type_scale::KICKER))
                    .text_color(rgb(theme::color::SEA_GREEN))
                    .child(title),
            )
            .child(self.text_field(
                match panel {
                    Panel::Assignments => Field::AssignmentName,
                    Panel::Assessments => Field::AssessmentName,
                },
                match panel {
                    Panel::Assignments => "assignment name",
                    Panel::Assessments => "assessment name",
                },
                window,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(theme::space::TIGHT))
                    .child(
                        div()
                            .id(format!("study-subject-{panel:?}"))
                            .w(px(theme::geometry::SUBJECT_W))
                            .px(px(theme::space::FIELD_X))
                            .py(px(theme::space::FIELD_Y))
                            .border_1()
                            .border_color(rgb(if open {
                                theme::color::ACCENT
                            } else {
                                theme::color::SURFACE
                            }))
                            .bg(rgb(theme::color::SURFACE).alpha(0.12))
                            .text_size(px(theme::type_scale::FIELD))
                            .text_color(rgb(theme::color::FOREGROUND))
                            .cursor_pointer()
                            .hover(|style| style.text_color(rgb(theme::color::ACCENT)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.toggle_subject(panel, cx)
                            }))
                            .child(format!("{subject} ▾")),
                    )
                    .child(self.datetime_button(target, cx))
                    .child(
                        div()
                            .id(format!("study-add-{panel:?}"))
                            .px(px(theme::space::ITEM_X))
                            .py(px(theme::space::ITEM_Y - 2.))
                            .bg(rgb(theme::color::SURFACE))
                            .text_size(px(theme::type_scale::NAV))
                            .text_color(rgb(theme::color::ACCENT))
                            .cursor_pointer()
                            .hover(|style| style.text_color(rgb(theme::color::FOREGROUND)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                match panel {
                                    Panel::Assignments => this.add_assignment(cx),
                                    Panel::Assessments => this.add_assessment(cx),
                                }
                            }))
                            .child("add"),
                    ),
            )
            .when(open, |el| {
                el.child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .border_1()
                        .border_color(rgb(theme::color::SURFACE))
                        .children(schedule::subjects().into_iter().map(|candidate| {
                            let selected = candidate == subject;
                            div()
                                .id(format!("study-subject-{panel:?}-{candidate}"))
                                .px(px(theme::space::TIGHT))
                                .py(px(theme::space::FIELD_Y))
                                .text_size(px(theme::type_scale::NAV))
                                .text_color(rgb(if selected {
                                    theme::color::ACCENT
                                } else {
                                    theme::color::MUTED
                                }))
                                .when(selected, |el| el.bg(rgb(theme::color::SURFACE)))
                                .cursor_pointer()
                                .hover(|style| style.text_color(rgb(theme::color::FOREGROUND)))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.set_subject(panel, candidate, cx)
                                }))
                                .child(candidate)
                        })),
                )
            })
            .child(
                div()
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child(hint),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .children(entries.into_iter().map(|(index, name, subject, at)| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(theme::space::TIGHT))
                            .py(px(theme::space::LEDGER_ROW_Y))
                            .border_b_1()
                            .border_color(rgb(theme::color::SURFACE))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(0.))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(theme::type_scale::LEDGER))
                                            .text_color(rgb(theme::color::FOREGROUND))
                                            .child(if name.trim().is_empty() {
                                                "untitled".to_owned()
                                            } else {
                                                name
                                            }),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(theme::space::DOT_GAP))
                                            .child(
                                                div()
                                                    .text_size(px(theme::type_scale::MINI))
                                                    .text_color(rgb(theme::color::ACCENT))
                                                    .child(subject),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(theme::type_scale::MINI))
                                                    .text_color(rgb(theme::color::MUTED))
                                                    .child(at.format("%d-%b-%Y %H:%M").to_string()),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id(format!("study-del-{panel:?}-{index}"))
                                    .px(px(theme::space::DOT_GAP))
                                    .text_size(px(theme::type_scale::LEDGER))
                                    .text_color(rgb(theme::color::MUTED))
                                    .cursor_pointer()
                                    .hover(|style| {
                                        style.text_color(rgb(theme::color::PURPLE))
                                    })
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        match panel {
                                            Panel::Assignments => {
                                                this.delete_assignment(index, cx)
                                            }
                                            Panel::Assessments => {
                                                this.delete_assessment(index, cx)
                                            }
                                        }
                                    }))
                                    .child("×"),
                            )
                    })),
            )
            .when(empty, |el| {
                el.child(
                    div()
                        .text_size(px(theme::type_scale::HINT))
                        .text_color(rgb(theme::color::MUTED))
                        .child("nothing yet"),
                )
            })
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
        let allow_cancelled = session.end_at() <= now;
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
                Some(Attendance::Cancelled) => (
                    theme::color::YELLOW,
                    rgb(theme::color::YELLOW).alpha(0.16),
                    theme::color::YELLOW,
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
                    this.cycle_attendance(key.clone(), allow_cancelled, cx)
                }));
        }
        card
    }

    fn calendar(&self, cx: &Context<Self>) -> impl IntoElement {
        let sessions = schedule::sessions_in_week(self.week);
        let mut by_day: Vec<Vec<Session>> = vec![Vec::new(); 5];
        for session in sessions {
            let offset = (session.date - self.week).num_days();
            if (0..5).contains(&offset) {
                by_day[offset as usize].push(session);
            }
        }

        let today = Local::now().date_naive();
        let now = Local::now().naive_local();
        let header = div()
            .w_full()
            .flex()
            .border_b_1()
            .border_color(rgb(theme::color::SURFACE))
            .children((0..5).map(|offset| {
                let date = self.week + chrono::Duration::days(offset);
                let is_today = date == today;
                div()
                    .flex_1()
                    .min_w_0()
                    .px(px(theme::space::DOT_GAP))
                    .py(px(theme::space::DOT_GAP))
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(if is_today {
                        theme::color::ACCENT
                    } else {
                        theme::color::MUTED
                    }))
                    .child(format!(
                        "{} {}",
                        date.format("%a").to_string().to_uppercase(),
                        date.format("%d")
                    ))
            }));

        let cells = (0..5).map(|offset| {
            let date = self.week + chrono::Duration::days(offset);
            let is_today = date == today;
            let day_sessions = &by_day[offset as usize];
            let empty = day_sessions.is_empty();
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
                .when(empty, |el| {
                    el.child(
                        div()
                            .text_size(px(theme::type_scale::MINI))
                            .text_color(rgb(theme::color::MUTED))
                            .child("no classes"),
                    )
                })
                .children(
                    day_sessions
                        .iter()
                        .map(|session| self.session_row(session, now, cx)),
                )
                .into_any_element()
        });

        div()
            .w_full()
            .flex()
            .flex_col()
            .border_t_1()
            .border_l_1()
            .border_color(rgb(theme::color::SURFACE))
            .child(header)
            .child(div().w_full().flex().children(cells))
    }
}

fn summarize_at(data: &StudyData, month: NaiveDate, now: NaiveDateTime) -> StudySummary {
    let mut overall = Stat { attended: 0, total: 0 };
    let mut lectures = Stat { attended: 0, total: 0 };
    let mut practice = Stat { attended: 0, total: 0 };

    for session in schedule::sessions_in_month(month) {
        if session.start_at() > now {
            continue;
        }
        let state = data.attendance.get(&session.key()).copied();
        if state == Some(Attendance::Cancelled) {
            continue;
        }
        let attended = state == Some(Attendance::Attended);
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let middle = self.week + chrono::Duration::days(3);
        let stats_month = NaiveDate::from_ymd_opt(middle.year(), middle.month(), 1)
            .expect("first of month is valid");
        let summary = self.summary_for(stats_month);

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(theme::space::TIGHT))
            .child(self.week_nav(cx))
            .child(
                div()
                    .text_size(px(theme::type_scale::KICKER))
                    .text_color(rgb(theme::color::SEA_GREEN))
                    .child(format!(
                        "{} attendance",
                        stats_month.format("%B %Y").to_string().to_uppercase()
                    )),
            )
            .child(summary_row(summary))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(theme::space::GROUP_Y))
                    .text_size(px(theme::type_scale::MINI))
                    .text_color(rgb(theme::color::MUTED))
                    .child("click a class:")
                    .child(
                        div()
                            .text_color(rgb(theme::color::MINT))
                            .child("attended"),
                    )
                    .child(
                        div()
                            .text_color(rgb(theme::color::PURPLE))
                            .child("missed"),
                    )
                    .child(
                        div()
                            .text_color(rgb(theme::color::YELLOW))
                            .child("didn't happen"),
                    ),
            )
            .child(self.calendar(cx))
            .child(
                div()
                    .w_full()
                    .flex()
                    .gap(px(theme::space::GROUP_Y))
                    .child(self.list_panel(Panel::Assignments, window, cx))
                    .child(self.list_panel(Panel::Assessments, window, cx)),
            )
            .when_some(self.status.clone(), |el, status| {
                el.child(
                    div()
                        .text_size(px(theme::type_scale::HINT))
                        .text_color(rgb(theme::color::YELLOW))
                        .child(status),
                )
            })
            .children(self.picker_backdrop(window, cx))
            .children(self.picker_overlay(cx))
    }
}
