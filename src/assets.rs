//! Compile-time embedded assets, keeping the install a single binary.

use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};

/// Asset path of the app logo (see `assets/prog-rs.svg`).
pub const LOGO: &str = "prog-rs.svg";

pub struct EmbeddedAssets;

impl AssetSource for EmbeddedAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path {
            LOGO => Ok(Some(Cow::Borrowed(include_bytes!("../assets/prog-rs.svg")))),
            _ => Ok(None),
        }
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![SharedString::from(LOGO)])
    }
}
