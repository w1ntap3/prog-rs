//! Central design tokens. Everything visual must reference this module so the
//! app stays consistent: one palette (the "Javid" scheme from
//! ~/.config/kitty/kitty.conf), one spacing scale, one type scale.
//! Never hardcode a color, size, or spacing anywhere else.
//!
//! Style language: flat and minimal — square corners, no shadows, no borders
//! except hairline separators. (Expected to grow fancier later; keep changes
//! centralized here regardless.)

/// The Javid colorscheme. Named by role, hex values match the kitty config.
/// The whole palette lives here even when unused, as the single source of truth
/// for future UI work.
pub mod color {
    pub const BACKGROUND: u32 = 0x0D_120F;
    pub const FOREGROUND: u32 = 0xC8_DCC7;
    pub const MUTED: u32 = 0x59_665D; // color8, de-emphasized text
    pub const ACCENT: u32 = 0x9F_E37F; // color2, selection & focus
    pub const SURFACE: u32 = 0x35_4735; // selection fill, separators, borders
    pub const PURPLE: u32 = 0xB8_8AD9; // color5
}

/// Window and element geometry. Units are GPUI px (logical pixels).
pub mod geometry {
    pub const SIDEBAR_WIDTH: f32 = 220.0;
    /// Restore size used when the maximized window is unmaximized.
    pub const RESTORE_WIDTH: f32 = 960.0;
    pub const RESTORE_HEIGHT: f32 = 600.0;
    pub const BRAND_HEIGHT: f32 = 56.0;
    pub const LOGO: f32 = 26.0;
    /// Accent rule under page titles; also the width of the nav selection bar.
    pub const RULE_W: f32 = 40.0;
    pub const RULE_H: f32 = 2.0;
}

/// Spacing scale (4px base).
pub mod space {
    pub const SIDEBAR_TOP: f32 = 12.0;
    pub const ITEM_GAP: f32 = 2.0;
    pub const ITEM_X: f32 = 16.0;
    pub const ITEM_Y: f32 = 10.0;
    pub const CONTENT_X: f32 = 36.0;
    pub const CONTENT_TOP: f32 = 28.0;
    pub const TIGHT: f32 = 8.0;
    pub const KICKER_GAP: f32 = 6.0;
}

/// Type scale.
pub mod type_scale {
    pub const BRAND: f32 = 15.0;
    pub const NAV: f32 = 13.0;
    pub const KICKER: f32 = 11.0;
    pub const TITLE: f32 = 26.0;
    pub const HINT: f32 = 12.0;
}

/// Font weights used by the type scale.
pub mod weight {
    pub const NAV: f32 = 400.0;
    pub const NAV_ACTIVE: f32 = 600.0;
    pub const BRAND: f32 = 600.0;
    pub const TITLE: f32 = 600.0;
}

/// UI font: the same 0xProto the user's terminal runs.
pub const FONT: &str = "0xProto Nerd Font";
