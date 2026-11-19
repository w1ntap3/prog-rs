//! Personal data store: the private counterpart to `crate::config`.
//!
//! Everything under `$XDG_DATA_HOME/prog-rs` is user content (finances and any
//! future records). Rules of the house:
//! - it is never committed and never read by config/export machinery;
//! - it only leaves the machine via an explicit `export` the user triggers;
//! - sections will gain real file formats here as they mature — the directory
//!   itself is the format-agnostic unit that export/import operates on.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::paths;

/// The live (private) data directory.
#[allow(dead_code, reason = "scaffold: will be wired to in-app export/import")]
pub fn dir() -> std::path::PathBuf {
    paths::data_dir()
}

/// Copy the whole data directory into `dest` (created if missing).
///
/// v1 exports as a plain directory tree; a single-file archive can replace
/// this later without changing call sites.
#[allow(dead_code, reason = "scaffold: will be wired to in-app export/import")]
pub fn export(dest: &Path) -> Result<()> {
    if dest.starts_with(dir()) {
        bail!("export destination must not be inside the data directory");
    }
    let src = dir();
    if !src.exists() {
        // Nothing collected yet; still hand over an empty, valid store.
        fs::create_dir_all(dest)
            .with_context(|| format!("creating {}", dest.display()))?;
        return Ok(());
    }
    copy_tree(&src, dest)
}

/// Replace the live data directory with a copy of `src` (e.g. a previous
/// export). The previous contents are removed first so imports are exact.
#[allow(dead_code, reason = "scaffold: will be wired to in-app export/import")]
pub fn import(src: &Path) -> Result<()> {
    if !src.is_dir() {
        bail!("import source {} is not a directory", src.display());
    }
    if dir().starts_with(src) {
        bail!("import source must not contain the data directory");
    }
    let target = dir();
    if target.exists() {
        fs::remove_dir_all(&target)
            .with_context(|| format!("clearing {}", target.display()))?;
    }
    copy_tree(src, &target)
}

/// Recursive copy of a directory tree.
fn copy_tree(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?;
    for entry in
        fs::read_dir(src).with_context(|| format!("reading {}", src.display()))?
    {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .with_context(|| format!("copying {} -> {}", from.display(), to.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("progrs-data-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn copy_tree_handles_nesting() {
        let src = temp("src");
        fs::create_dir_all(src.join("finances/2026")).unwrap();
        fs::write(src.join("finances/2026/jan.csv"), "a,b").unwrap();
        fs::write(src.join("note.txt"), "hi").unwrap();

        let dest = temp("dest");
        copy_tree(&src, &dest).unwrap();

        assert_eq!(fs::read_to_string(dest.join("note.txt")).unwrap(), "hi");
        assert_eq!(
            fs::read_to_string(dest.join("finances/2026/jan.csv")).unwrap(),
            "a,b"
        );
        fs::remove_dir_all(&src).unwrap();
        fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    fn export_refuses_dest_inside_data_dir() {
        let inside = dir().join("escape");
        assert!(export(&inside).is_err());
    }
}
