//! Private study attendance data (`study.toml`).
//!
//! The timetable is code (see `schedule.rs`); only explicit attendance marks
//! are user data, stored under the private XDG data directory.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::data;

pub const VERSION: u32 = 1;
const FILE: &str = "study.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Attendance {
    Attended,
    Missed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyData {
    pub version: u32,
    #[serde(default)]
    pub attendance: BTreeMap<String, Attendance>,
}

impl Default for StudyData {
    fn default() -> Self {
        Self {
            version: VERSION,
            attendance: BTreeMap::new(),
        }
    }
}

impl StudyData {
    pub fn file_path() -> PathBuf {
        data::dir().join(FILE)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let data: StudyData =
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
            bail!("unsupported study data version {} (expected {VERSION})", self.version);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_attendance() {
        let dir = std::env::temp_dir().join(format!("progrs-study-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE);

        let mut data = StudyData::default();
        data.attendance
            .insert("BE5B01PRS|2026-11-02|09:15|lecture".into(), Attendance::Attended);
        data.save_to(&path).unwrap();
        assert_eq!(StudyData::load_from(&path).unwrap(), data);
        fs::remove_dir_all(&dir).unwrap();
    }
}
