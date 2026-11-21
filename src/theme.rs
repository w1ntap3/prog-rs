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
#[allow(dead_code, reason = "full palette kept for future UI")]
pub mod color {
    pub const BACKGROUND: u32 = 0x0D_120F;
    pub const FOREGROUND: u32 = 0xC8_DCC7;
    pub const MUTED: u32 = 0x59_665D; // color8, de-emphasized text
    pub const ACCENT: u32 = 0x9F_E37F; // color2, selection & focus
    pub const SURFACE: u32 = 0x35_4735; // selection fill, separators, borders
    pub const PURPLE: u32 = 0xB8_8AD9; // color5
    // Remaining palette colors, used sparingly to keep the UI from reading as
    // a single green wash.
    pub const YELLOW: u32 = 0xC4_D87C; // color3
    pub const SEA_GREEN: u32 = 0x6F_B88A; // color6
    pub const MINT: u32 = 0x7E_D6A1; // color14
    pub const BRIGHT_PURPLE: u32 = 0xD0_A5EF; // color13
    pub const BRIGHT_GREEN: u32 = 0xA8_E889; // color10
    pub const LIGHT_GREEN: u32 = 0x9B_CF78; // color12
    pub const LIGHT_PURPLE: u32 = 0xC4_9BE3; // color7
    pub const BRIGHTEST: u32 = 0xE0_E9DC; // color15
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
    /// Slideshow band height.
    pub const SLIDE_HEIGHT: f32 = 104.0;
    /// Chart height (the plot area inside it is padded by `space::CHART_PAD_*`).
    pub const GRAPH_HEIGHT: f32 = 220.0;
    /// Stroked chart lines.
    pub const LINE_W: f32 = 1.5;
    /// Hairline separators/guides.
    pub const HAIRLINE: f32 = 1.0;
    /// Slideshow progress dot.
    pub const DOT: f32 = 4.0;
    /// Common input widths.
    pub const FIELD_AMOUNT_W: f32 = 150.0;
    pub const FIELD_NOTE_W: f32 = 330.0;
    pub const FIELD_DATE_W: f32 = 130.0;
    pub const FIELD_TIME_W: f32 = 76.0;
    /// Ledger date+time column.
    pub const LEDGER_DATE_W: f32 = 84.0;
    /// Calendar / clock picker cells.
    pub const CAL_CELL: f32 = 26.0;
    pub const CAL_ROW: f32 = 22.0;
    pub const TIME_CELL: f32 = 30.0;
    /// Chart legend line swatch.
    pub const LEGEND_W: f32 = 14.0;
    /// Confirmation dialog width.
    pub const MODAL_W: f32 = 440.0;
    /// Appeal clock square.
    pub const CLOCK_BOX: f32 = 360.0;
    /// Chart hover tooltip.
    pub const HOVER_TOOLTIP_W: f32 = 230.0;
    pub const HOVER_TOOLTIP_H: f32 = 96.0;
    pub const HOVER_OFFSET: f32 = 12.0;
    /// Custom vertical scrollbar at the right edge of the content area.
    pub const SCROLLBAR_HIT_W: f32 = 12.0;
    pub const SCROLLBAR_IDLE_W: f32 = 3.0;
    pub const SCROLLBAR_HOVER_W: f32 = 6.0;
    pub const SCROLLBAR_MIN_H: f32 = 24.0;
    pub const SCROLLBAR_MARGIN: f32 = 3.0;
    /// Cap for centered settings status lines.
    pub const SETTINGS_STATUS_W: f32 = 560.0;
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
    pub const GROUP_Y: f32 = 26.0;
    pub const DOT_GAP: f32 = 4.0;
    /// Inset around the chart's plot area.
    pub const CHART_PAD_X: f32 = 8.0;
    pub const CHART_PAD_Y: f32 = 14.0;
    /// Text field inner padding.
    pub const FIELD_X: f32 = 8.0;
    pub const FIELD_Y: f32 = 6.0;
    /// Ledger row vertical padding.
    pub const LEDGER_ROW_Y: f32 = 5.0;
    /// Ledger scroll cap before it starts scrolling.
    pub const LEDGER_MAX_HEIGHT: f32 = 240.0;
}

/// Type scale.
pub mod type_scale {
    pub const BRAND: f32 = 15.0;
    pub const NAV: f32 = 13.0;
    pub const KICKER: f32 = 11.0;
    pub const TITLE: f32 = 26.0;
    pub const HINT: f32 = 12.0;
    /// Big accent line in the slideshow.
    pub const STATEMENT: f32 = 30.0;
    /// Individual relapse-free clock segments.
    pub const CLOCK_DIGIT: f32 = 38.0;
    /// Text field contents.
    pub const FIELD: f32 = 14.0;
    /// Ledger rows and other dense lists.
    pub const LEDGER: f32 = 12.0;
    /// Axis labels and clues.
    pub const MINI: f32 = 11.0;
}

/// Font weights used by the type scale.
pub mod weight {
    pub const NAV: f32 = 400.0;
    pub const NAV_ACTIVE: f32 = 600.0;
    pub const BRAND: f32 = 600.0;
    pub const TITLE: f32 = 600.0;
    pub const STATEMENT: f32 = 600.0;
}

/// UI font: the same 0xProto the user's terminal runs.
pub const FONT: &str = "0xProto Nerd Font";
