//! XDG base directories for user configuration and user data.
//!
//! The split is deliberate:
//! - **config** describes the app *structure* (sections, layout) — shareable,
//!   importable, exportable.
//! - **data** holds *personal content* (finances and any future user records).
//!   Private: it must never be committed to the repository and only leaves the
//!   machine through an explicit user-triggered export.

use std::path::{Path, PathBuf};

fn first_env_dir(key: &str, fallback: PathBuf) -> PathBuf {
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or(fallback)
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// `$XDG_CONFIG_HOME` or `~/.config`.
pub fn config_home() -> PathBuf {
    first_env_dir("XDG_CONFIG_HOME", home().join(".config"))
}

/// `$XDG_DATA_HOME` or `~/.local/share`.
pub fn data_home() -> PathBuf {
    first_env_dir("XDG_DATA_HOME", home().join(".local/share"))
}

/// This app's configuration directory, e.g. `~/.config/prog-rs`.
pub fn config_dir() -> PathBuf {
    config_home().join("prog-rs")
}

/// This app's private data directory, e.g. `~/.local/share/prog-rs`.
pub fn data_dir() -> PathBuf {
    data_home().join("prog-rs")
}

/// Create a directory (and parents) if missing.
pub fn ensure_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_app_name() {
        assert!(config_dir().ends_with("prog-rs"));
        assert!(data_dir().ends_with("prog-rs"));
    }
}
