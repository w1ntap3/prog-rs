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
    short: &'static str,
    kind: SessionKind,
    odd_weeks_only: bool,
}

fn time(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).expect("schedule time is valid")
}

fn templates() -> Vec<Template> {
    let t = |weekday, start, end, code, short, kind, odd_weeks_only| Template {
        weekday,
        start,
        end,
        code,
        short,
        kind,
        odd_weeks_only,
    };

    vec![
        t(Weekday::Mon, time(9, 15), time(10, 45), "BE5B01PRS", "PRS", SessionKind::Lecture, false),
        t(Weekday::Mon, time(9, 30), time(11, 30), "03240527", "ENT", SessionKind::Lecture, false),
        t(Weekday::Mon, time(11, 0), time(12, 30), "BE5B01PRS", "PRS", SessionKind::Lecture, false),
        t(Weekday::Mon, time(12, 45), time(14, 15), "BE5B01PRS", "PRS", SessionKind::Seminar, false),
        t(Weekday::Mon, time(14, 45), time(16, 15), "BE5B33ALG", "ALG", SessionKind::Lecture, false),
        t(Weekday::Mon, time(16, 30), time(18, 0), "BE5B33ALG", "ALG", SessionKind::Seminar, false),
        t(Weekday::Tue, time(12, 45), time(14, 15), "BE5B02PH2", "PH2", SessionKind::Lab, true),
        t(Weekday::Tue, time(14, 30), time(16, 0), "BE5B31ZEO", "ZEO", SessionKind::Lecture, false),
        t(Weekday::Tue, time(16, 15), time(17, 45), "BE5B02PH2", "PH2", SessionKind::Seminar, false),
        t(Weekday::Wed, time(9, 15), time(10, 45), "BE5B99CPL", "CPL", SessionKind::Lecture, false),
        t(Weekday::Wed, time(11, 0), time(12, 30), "BE5B99CPL", "CPL", SessionKind::Seminar, false),
        t(Weekday::Wed, time(14, 30), time(16, 0), "BE5B31ZEO", "ZEO", SessionKind::Seminar, false),
        t(Weekday::Thu, time(9, 15), time(11, 45), "BE5B02PH2", "PH2", SessionKind::Lecture, false),
        t(Weekday::Fri, time(11, 0), time(12, 30), "BE5B32PKS", "PKS", SessionKind::Lecture, false),
        t(Weekday::Fri, time(12, 45), time(14, 15), "BE5B32PKS", "PKS", SessionKind::Seminar, false),
    ]
}

/// Unique short course ids in timetable order.
pub fn subjects() -> Vec<&'static str> {
    let mut subjects = Vec::new();
    for template in templates() {
        if !subjects.contains(&template.short) {
            subjects.push(template.short);
        }
    }
    subjects
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub date: NaiveDate,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub code: &'static str,
    pub short: &'static str,
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

    pub fn end_at(&self) -> NaiveDateTime {
        self.date.and_time(self.end)
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


pub fn monday_of(date: NaiveDate) -> NaiveDate {
    date - chrono::Duration::days(date.weekday().num_days_from_monday() as i64)
}

/// Monday-Friday sessions for the week starting `week_start` (a Monday).
pub fn sessions_in_week(week_start: NaiveDate) -> Vec<Session> {
    let templates = templates();
    let start = semester_start();
    let end = semester_end();
    let mut sessions = Vec::new();
    for offset in 0..5 {
        let date = week_start + chrono::Duration::days(offset);
        if date < start || date > end {
            continue;
        }
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
                short: template.short,
                kind: template.kind,
            });
        }
    }
    sessions.sort_by_key(|session| (session.date, session.start));
    sessions
}

/// Weeks inside the semester that actually contain at least one session.
pub fn active_weeks() -> Vec<NaiveDate> {
    let mut weeks = Vec::new();
    let mut monday = monday_of(semester_start());
    let last = monday_of(semester_end());
    while monday <= last {
        if !sessions_in_week(monday).is_empty() {
            weeks.push(monday);
        }
        monday += chrono::Duration::days(7);
    }
    weeks
}

pub fn week_end(week_start: NaiveDate) -> NaiveDate {
    week_start + chrono::Duration::days(4)
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
                short: template.short,
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
        assert!(sessions.iter().any(|session| session.short == "PRS"));
        assert!(sessions.iter().any(|session| session.short == "ENT"));
        assert!(sessions.iter().all(|session| session.date >= semester_start()));
    }

    #[test]
    fn january_stops_on_the_tenth() {
        let month = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();
        let sessions = sessions_in_month(month);
        assert!(sessions.iter().all(|session| session.date <= semester_end()));
    }

    #[test]
    fn active_weeks_exclude_empty_weeks() {
        let weeks = active_weeks();
        assert!(!weeks.is_empty());
        assert!(weeks.iter().all(|week| week.weekday() == Weekday::Mon));
        assert!(weeks.iter().all(|week| !sessions_in_week(*week).is_empty()));
        assert_eq!(weeks[0], monday_of(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()));
    }

    #[test]
    fn semester_months_are_november_to_january() {
        let months = semester_months();
        assert_eq!(months.len(), 3);
        assert_eq!(months[0], NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(months[2], NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
    }
}
