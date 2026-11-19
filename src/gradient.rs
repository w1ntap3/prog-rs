//! Gradient sampling math, independent of any UI code.

use crate::theme::GRADIENT_STOPS;

/// Linear interpolation between two 0xRRGGBB colors.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let lerp = |x: u32, y: u32| (x as f32 + (y as f32 - x as f32) * t).round() as u32;
    (lerp((a >> 16) & 0xff, (b >> 16) & 0xff) << 16)
        | (lerp((a >> 8) & 0xff, (b >> 8) & 0xff) << 8)
        | lerp(a & 0xff, b & 0xff)
}

/// Hermite smoothstep, for softer transitions between gradient stops.
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Sample the looping [`GRADIENT_STOPS`] palette at `t`, wrapping around [0, 1).
pub fn sample(t: f32) -> u32 {
    let t = t.rem_euclid(1.0);
    let f = t * GRADIENT_STOPS.len() as f32;
    let i = f as usize % GRADIENT_STOPS.len();
    let j = (i + 1) % GRADIENT_STOPS.len();
    mix(GRADIENT_STOPS[i], GRADIENT_STOPS[j], smooth(f - f.floor()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::GRADIENT_STOPS;

    #[test]
    fn sample_hits_exact_stops_at_exact_positions() {
        let n = GRADIENT_STOPS.len() as f32;
        for (i, stop) in GRADIENT_STOPS.iter().enumerate() {
            assert_eq!(sample(i as f32 / n), *stop);
        }
    }

    #[test]
    fn sample_is_periodic() {
        for t in [0.0f32, 0.17, 0.5, 0.93] {
            assert_eq!(sample(t), sample(t + 1.0));
        }
    }

    #[test]
    fn mix_endpoints_are_exact() {
        assert_eq!(mix(0x000000, 0xFFFFFF, 0.0), 0x000000);
        assert_eq!(mix(0x000000, 0xFFFFFF, 1.0), 0xFFFFFF);
    }
}
