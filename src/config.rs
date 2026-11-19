//! Application *structure* configuration: the list of sections, stored as
//! human-editable TOML at `$XDG_CONFIG_HOME/prog-rs/config.toml`.
//!
//! Config describes structure only — never personal data (that lives in
//! `crate::data`). Export/import is file-based, so structure can move between
//! machines without touching anyone's private content.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::paths;

pub const VERSION: u32 = 1;
const FILE: &str = "config.toml";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    /// Sections shown in the menu, in order. "main" first by convention.
    pub sections: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: VERSION,
            sections: ["main", "finances", "appeal", "settings"]
                .map(str::to_owned)
                .to_vec(),
        }
    }
}

impl Config {
    /// Where the live config file lives.
    pub fn file_path() -> std::path::PathBuf {
        paths::config_dir().join(FILE)
    }

    /// Load from `path`, validating it.
    pub fn load_from(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        let config: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    /// Load the live config; on first run, create it with defaults.
    pub fn load_or_create() -> Result<Self> {
        let path = Self::file_path();
        if path.exists() {
            return Self::load_from(&path);
        }
        let config = Self::default();
        paths::ensure_dir(&path.parent().expect("config path has a parent"))?;
        config.save_to(&path)?;
        Ok(config)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            paths::ensure_dir(dir)?;
        }
        fs::write(path, toml::to_string_pretty(self)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    /// Save to the default config location.
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::file_path())
    }

    /// Write this config to an arbitrary destination for sharing.
    pub fn export(&self, dest: &Path) -> Result<()> {
        self.save_to(dest)
    }

    /// Replace the live config with a file from elsewhere (validated first).
    pub fn import(source: &Path) -> Result<Self> {
        let config = Self::load_from(source)?;
        config.save()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.version != VERSION {
            bail!("unsupported config version {} (expected {VERSION})", self.version);
        }
        if self.sections.is_empty() {
            bail!("config has no sections");
        }
        for (i, section) in self.sections.iter().enumerate() {
            if section.trim().is_empty() {
                bail!("section {} is empty", i + 1);
            }
            if self.sections[..i]
                .iter()
                .any(|earlier| earlier == section)
            {
                bail!("duplicate section {section:?}");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("progrs-cfg-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn roundtrip_through_file() {
        let dir = temp_dir("roundtrip");
        let config = Config::default();
        let path = dir.join(FILE);
        config.export(&path).unwrap();
        assert_eq!(Config::load_from(&path).unwrap(), config);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn import_replaces_live_shape_and_validates() {
        let dir = temp_dir("import");
        let path = dir.join("shared.toml");
        let custom = Config {
            version: VERSION,
            sections: ["main", "projects"].map(str::to_owned).to_vec(),
        };
        custom.export(&path).unwrap();
        // load_from is the validation gate import() uses.
        assert_eq!(Config::load_from(&path).unwrap(), custom);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn import_replaces_live_config_file() {
        let dir = temp_dir("live");
        let previous = std::env::var_os("XDG_CONFIG_HOME");
        // SAFETY: single-threaded effect, restored below; other tests never
        // depend on the XDG value.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };

        let result = (|| {
            let src = dir.join("incoming.toml");
            let custom = Config {
                version: VERSION,
                sections: ["main", "tools"].map(str::to_owned).to_vec(),
            };
            custom.export(&src).unwrap();
            let imported = Config::import(&src).unwrap();
            assert_eq!(imported, custom);
            assert_eq!(Config::load_from(&Config::file_path()).unwrap(), custom);
        })();

        match previous {
            Some(value) => unsafe { std::env::set_var("XDG_CONFIG_HOME", value) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        };
        let _ = result;
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_bad_configs() {
        let dir = temp_dir("bad");

        let empty = dir.join("empty.toml");
        fs::write(&empty, "version = 1\nsections = []\n").unwrap();
        assert!(Config::load_from(&empty).is_err());

        let dupes = dir.join("dupes.toml");
        fs::write(&dupes, "version = 1\nsections = [\"a\", \"a\"]\n").unwrap();
        assert!(Config::load_from(&dupes).is_err());

        let old = dir.join("old.toml");
        fs::write(&old, "version = 99\nsections = [\"a\"]\n").unwrap();
        assert!(Config::load_from(&old).is_err());

        let unknown = dir.join("unknown.toml");
        fs::write(&unknown, "version = 1\nsections = [\"a\"]\nnope = 1\n").unwrap();
        assert!(Config::load_from(&unknown).is_err());

        fs::remove_dir_all(&dir).unwrap();
    }
}
