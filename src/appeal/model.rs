//! Appeal records: the private data behind the `appeal` section.
//!
//! The only fact stored is the timestamp of the last relapse; the clock in the
//! UI is derived from it. Data lives in the private data dir (`appeal.toml`),
//! so it is included in data export/import and never in the repo.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};

use crate::data;

pub const VERSION: u32 = 1;
const FILE: &str = "appeal.toml";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppealData {
    pub version: u32,
    pub last_relapse: NaiveDateTime,
}

impl Default for AppealData {
    fn default() -> Self {
        Self {
            version: VERSION,
            last_relapse: now_seconds(),
        }
    }
}

impl AppealData {
    pub fn file_path() -> PathBuf {
        data::dir().join(FILE)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let data: AppealData =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        data.validate()?;
        Ok(data)
    }

    pub fn load_or_create() -> Result<Self> {
        let path = Self::file_path();
        if path.exists() {
            return Self::load_from(&path);
        }
        let data = Self::default();
        data.save()?;
        Ok(data)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        self.validate()?;
        if let Some(dir) = path.parent() {
            crate::paths::ensure_dir(dir)?;
        }
        fs::write(path, toml::to_string_pretty(self)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::file_path())
    }

    fn validate(&self) -> Result<()> {
        if self.version != VERSION {
            bail!("unsupported appeal data version {} (expected {VERSION})", self.version);
        }
        Ok(())
    }
}

/// Current local time, truncated to whole seconds.
pub fn now_seconds() -> NaiveDateTime {
    let now = Local::now().naive_local();
    now.with_nanosecond(0).unwrap_or(now)
}

/// `MM:DD:HH:MM:SS` since `from`, using calendar months.
pub fn format_elapsed(from: NaiveDateTime, to: NaiveDateTime) -> String {
    let (months, days, hours, minutes, seconds) = elapsed_breakdown(from, to);
    format!("{months:02}:{days:02}:{hours:02}:{minutes:02}:{seconds:02}")
}

/// Calendar months first, then the remaining time in days/hours/minutes/seconds.
pub fn elapsed_breakdown(
    from: NaiveDateTime,
    to: NaiveDateTime,
) -> (i64, i64, i64, i64, i64) {
    if to <= from {
        return (0, 0, 0, 0, 0);
    }

    let mut months =
        (to.year() - from.year()) * 12 + (to.month() as i32 - from.month() as i32);
    months = months.max(0);
    let mut anchor = add_months(from, months);
    if anchor > to {
        months -= 1;
        anchor = add_months(from, months);
    }

    let remaining = to - anchor;
    (
        months as i64,
        remaining.num_days(),
        remaining.num_hours() % 24,
        remaining.num_minutes() % 60,
        remaining.num_seconds() % 60,
    )
}

fn add_months(value: NaiveDateTime, delta: i32) -> NaiveDateTime {
    let index = value.year() * 12 + value.month0() as i32 + delta;
    let year = index.div_euclid(12);
    let month = (index.rem_euclid(12) + 1) as u32;
    let day = value.day().min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day)
        .expect("clamped date is valid")
        .and_time(value.time())
}

fn days_in_month(year: i32, month: u32) -> u32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, s)
            .unwrap()
    }

    #[test]
    fn formats_days_hours_minutes_seconds() {
        let from = dt(2026, 11, 21, 10, 0, 0);
        let to = dt(2026, 11, 23, 12, 34, 56);
        assert_eq!(format_elapsed(from, to), "00:02:02:34:56");
    }

    #[test]
    fn handles_calendar_months() {
        let from = dt(2026, 1, 31, 12, 0, 0);
        let to = dt(2026, 3, 1, 12, 0, 0);
        assert_eq!(format_elapsed(from, to), "01:01:00:00:00");
    }

    #[test]
    fn clamps_roundtrip_and_validates_version() {
        let dir = std::env::temp_dir().join(format!("progrs-appeal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE);
        let data = AppealData {
            version: VERSION,
            last_relapse: dt(2026, 11, 1, 9, 30, 0),
        };
        data.save_to(&path).unwrap();
        assert_eq!(AppealData::load_from(&path).unwrap(), data);
        fs::remove_dir_all(&dir).unwrap();
    }
}
