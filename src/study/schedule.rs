//! The fixed 2026 winter-semester timetable and session generation.
//!
//! The schedule itself is code (structure); attendance is user data and lives
//! in `crate::study::model`. The winter semester window is 1 Nov 2026 through
//! 10 Jan 2027. The summer timetable is intentionally unknown for now.

use chrono::{Datelike, IsoWeek, NaiveDate, NaiveDateTime, NaiveTime, Weekday};

pub const SEMESTER_START: (i32, u32, u32) = (2026, 11, 1);
pub const SEMESTER_END: (i32, u32, u32) = (2027, 1, 10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Lecture,
    Seminar,
    Lab,
}

impl SessionKind {
    pub fn label(self) -> &'static str {
        match self {
            SessionKind::Lecture => "lecture",
            SessionKind::Seminar => "seminar",
            SessionKind::Lab => "lab",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            SessionKind::Lecture => "L",
            SessionKind::Seminar => "S",
            SessionKind::Lab => "LAB",
        }
    }

    pub fn color(self) -> u32 {
        match self {
            SessionKind::Lecture => crate::theme::color::ACCENT,
            SessionKind::Seminar => crate::theme::color::MINT,
            SessionKind::Lab => crate::theme::color::PURPLE,
        }
    }

    pub fn counts_as_lecture(self) -> bool {
        matches!(self, SessionKind::Lecture)
    }

    pub fn counts_as_practice(self) -> bool {
        matches!(self, SessionKind::Seminar | SessionKind::Lab)
    }
}

#[derive(Debug, Clone, Copy)]
struct Template {
    weekday: Weekday,
    start: NaiveTime,
    end: NaiveTime,
    code: &'static str,
    name: &'static str,
    kind: SessionKind,
    odd_weeks_only: bool,
}

fn time(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).expect("schedule time is valid")
}

fn templates() -> Vec<Template> {
    let t = |weekday, start, end, code, name, kind, odd_weeks_only| Template {
        weekday,
        start,
        end,
        code,
        name,
        kind,
        odd_weeks_only,
    };

    vec![
        t(Weekday::Mon, time(9, 15), time(10, 45), "BE5B01PRS", "Probability and Statistics", SessionKind::Lecture, false),
        t(Weekday::Mon, time(9, 30), time(11, 30), "03240527", "Entrepreneurship Essentials", SessionKind::Lecture, false),
        t(Weekday::Mon, time(11, 0), time(12, 30), "BE5B01PRS", "Probability and Statistics", SessionKind::Lecture, false),
        t(Weekday::Mon, time(12, 45), time(14, 15), "BE5B01PRS", "Probability and Statistics", SessionKind::Seminar, false),
        t(Weekday::Mon, time(14, 45), time(16, 15), "BE5B33ALG", "Algorithms", SessionKind::Lecture, false),
        t(Weekday::Mon, time(16, 30), time(18, 0), "BE5B33ALG", "Algorithms", SessionKind::Seminar, false),
        t(Weekday::Tue, time(12, 45), time(14, 15), "BE5B02PH2", "Physics 2", SessionKind::Lab, true),
        t(Weekday::Tue, time(14, 30), time(16, 0), "BE5B31ZEO", "Fundamentals of Electrical Circuits", SessionKind::Lecture, false),
        t(Weekday::Tue, time(16, 15), time(17, 45), "BE5B02PH2", "Physics 2", SessionKind::Seminar, false),
        t(Weekday::Wed, time(9, 15), time(10, 45), "BE5B99CPL", "C Programming Language", SessionKind::Lecture, false),
        t(Weekday::Wed, time(11, 0), time(12, 30), "BE5B99CPL", "C Programming Language", SessionKind::Seminar, false),
        t(Weekday::Wed, time(14, 30), time(16, 0), "BE5B31ZEO", "Fundamentals of Electrical Circuits", SessionKind::Seminar, false),
        t(Weekday::Thu, time(9, 15), time(11, 45), "BE5B02PH2", "Physics 2", SessionKind::Lecture, false),
        t(Weekday::Fri, time(11, 0), time(12, 30), "BE5B32PKS", "Computer and Communication Networks", SessionKind::Lecture, false),
        t(Weekday::Fri, time(12, 45), time(14, 15), "BE5B32PKS", "Computer and Communication Networks", SessionKind::Seminar, false),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub date: NaiveDate,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub code: &'static str,
    pub name: &'static str,
    pub kind: SessionKind,
}

impl Session {
    /// Stable id stored in `study.toml` for attendance.
    pub fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.code,
            self.date,
            self.start.format("%H:%M"),
            self.kind.label()
        )
    }

    pub fn start_at(&self) -> NaiveDateTime {
        self.date.and_time(self.start)
    }
}

pub fn semester_start() -> NaiveDate {
    NaiveDate::from_ymd_opt(SEMESTER_START.0, SEMESTER_START.1, SEMESTER_START.2)
        .expect("semester start is valid")
}

pub fn semester_end() -> NaiveDate {
    NaiveDate::from_ymd_opt(SEMESTER_END.0, SEMESTER_END.1, SEMESTER_END.2)
        .expect("semester end is valid")
}

/// Months that make up the known winter semester: November, December, January.
pub fn semester_months() -> Vec<NaiveDate> {
    let start = semester_start();
    let end = semester_end();
    let mut months = Vec::new();
    let mut cursor = NaiveDate::from_ymd_opt(start.year(), start.month(), 1)
        .expect("first of month is valid");
    let last = NaiveDate::from_ymd_opt(end.year(), end.month(), 1)
        .expect("first of month is valid");
    while cursor <= last {
        months.push(cursor);
        cursor = next_month(cursor);
    }
    months
}

pub fn next_month(date: NaiveDate) -> NaiveDate {
    let (year, month) = if date.month() == 12 {
        (date.year() + 1, 1)
    } else {
        (date.year(), date.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1).expect("first of month is valid")
}

pub fn days_in_month(date: NaiveDate) -> u32 {
    let next = next_month(date);
    next.pred_opt()
        .expect("first of month has a predecessor")
        .day()
}

/// All sessions in `month` (first day of month), clipped to the semester.
pub fn sessions_in_month(month: NaiveDate) -> Vec<Session> {
    let first = NaiveDate::from_ymd_opt(month.year(), month.month(), 1)
        .expect("first of month is valid");
    let next = next_month(first);
    let start = first.max(semester_start());
    let end = next.pred_opt().expect("month has a last day").min(semester_end());

    let templates = templates();
    let mut sessions = Vec::new();
    let mut date = start;
    while date <= end {
        for template in &templates {
            if template.weekday != date.weekday() {
                continue;
            }
            if template.odd_weeks_only && !is_odd_week(date.iso_week()) {
                continue;
            }
            sessions.push(Session {
                date,
                start: template.start,
                end: template.end,
                code: template.code,
                name: template.name,
                kind: template.kind,
            });
        }
        date = date.succ_opt().expect("date has a successor");
    }
    sessions.sort_by_key(|session| (session.date, session.start));
    sessions
}

/// The schedule says "odd weeks"; semester parity is approximated with ISO
/// week numbers until an explicit academic week anchor is available.
fn is_odd_week(week: IsoWeek) -> bool {
    week.week() % 2 == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_generation_matches_weekly_template() {
        let month = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let sessions = sessions_in_month(month);
        assert!(!sessions.is_empty());
        assert!(sessions.iter().any(|session| session.code == "BE5B01PRS"));
        assert!(sessions.iter().any(|session| session.code == "03240527"));
        assert!(sessions.iter().all(|session| session.date >= semester_start()));
    }

    #[test]
    fn january_stops_on_the_tenth() {
        let month = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();
        let sessions = sessions_in_month(month);
        assert!(sessions.iter().all(|session| session.date <= semester_end()));
    }

    #[test]
    fn semester_months_are_november_to_january() {
        let months = semester_months();
        assert_eq!(months.len(), 3);
        assert_eq!(months[0], NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(months[2], NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
    }
}
