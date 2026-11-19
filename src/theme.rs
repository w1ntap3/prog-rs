//! Central design tokens. Everything visual must reference this module so the
//! app stays consistent: one palette (the "Javid" scheme from
//! ~/.config/kitty/kitty.conf), one set of sizes. Never hardcode a color or
//! spacing anywhere else.

/// The Javid colorscheme. Named by role, hex values match the kitty config.
/// The whole palette lives here even when unused, as the single source of truth
/// for future UI work.
#[expect(dead_code, reason = "full palette kept as design documentation")]
pub mod color {
    pub const BACKGROUND: u32 = 0x0D120F;
    pub const FOREGROUND: u32 = 0xC8DCC7;

    pub const BRIGHT_GREEN: u32 = 0x9FE37F; // color2
    pub const YELLOW_GREEN: u32 = 0xC4D87C; // color3
    pub const SEA_GREEN: u32 = 0x6FB88A;    // color4
    pub const PURPLE: u32 = 0xB88AD9;       // color5 / color1
    pub const MINT: u32 = 0x7ED6A1;         // color6
    pub const BORDER: u32 = 0x354735;       // active/inactive border
}

/// Gradient accents in cycle order: green -> mint -> purple -> sea green.
pub const GRADIENT_STOPS: [u32; 4] = [
    color::BRIGHT_GREEN,
    color::MINT,
    color::PURPLE,
    color::SEA_GREEN,
];

/// Fixed geometry. Units are GPUI px (logical pixels).
pub mod size {
    pub const WINDOW: f32 = 160.0;
    pub const BOX: f32 = 100.0;
    pub const RADIUS: f32 = 16.0;
}

/// Durations for recurring motion.
pub mod motion {
    use std::time::Duration;

    /// One full gradient cycle.
    pub const GRADIENT_CYCLE: Duration = Duration::from_secs(12);
}
