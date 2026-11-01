//! From scene XYZ to display sRGB: exposure, a filmic curve, and sRGB
//! encoding.

use std::str::FromStr;

use crate::sky::spectrum::Xyz;
use crate::vec3::{dot, Vec3};

/// Linear sRGB in [0, 1].
pub type Rgb = [f64; 3];

/// How a display squeezes scene light into its range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    /// Every channel through the filmic curve on its own, as on film:
    /// bright colours drift towards yellow and white.
    Channels,
    /// The brightest channel through the curve and the others scaled
    /// along, keeping hue. Only light far beyond white fades to white.
    Hue,
}

/// Exposed light this many times the display's white is half faded to
/// white under `Curve::Hue`.
const WHITE: f64 = 32.0;

const XYZ_TO_SRGB: [Vec3; 3] = [
    [3.2406, -1.5372, -0.4986],
    [-0.9689, 1.8758, 0.0415],
    [0.0557, -0.2040, 1.0570],
];

/// Narkowicz's fit of the ACES filmic curve. Bright saturated colours
/// roll off towards white, like film.
fn filmic(x: f64) -> f64 {
    (x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14)).min(1.0)
}

fn hue_preserving(rgb: Rgb) -> Rgb {
    let peak = rgb.into_iter().fold(f64::MIN_POSITIVE, f64::max);
    let white = peak * peak / (peak * peak + WHITE * WHITE);
    rgb.map(|c| (c / peak * (1.0 - white) + white) * filmic(peak))
}

pub fn tonemap(xyz: Xyz, exposure: f64, curve: Curve) -> Rgb {
    let rgb = XYZ_TO_SRGB.map(|row| (dot(row, xyz) * exposure).max(0.0));
    match curve {
        Curve::Channels => rgb.map(filmic),
        Curve::Hue => hue_preserving(rgb),
    }
}

impl FromStr for Curve {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "channels" => Ok(Curve::Channels),
            "hue" => Ok(Curve::Hue),
            _ => Err(format!("unknown curve {s}")),
        }
    }
}

/// sRGB transfer function and quantization. `noise`, in units of one
/// level, dithers away the banding of smooth gradients.
pub fn encode(c: f64, noise: f64) -> u8 {
    let e = if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (e * 255.0 + noise).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    const D65: Xyz = [0.9505, 1.0, 1.089];

    fn neutral(rgb: Rgb) -> bool {
        (rgb[0] - rgb[1]).abs() < 1e-3 && (rgb[1] - rgb[2]).abs() < 1e-3
    }

    #[test]
    fn d65_white_maps_to_neutral_grey() {
        assert!(neutral(tonemap(D65, 1.0, Curve::Channels)));
        assert!(neutral(tonemap(D65, 1.0, Curve::Hue)));
    }

    #[test]
    fn curves_agree_on_greys() {
        let (a, b) = (tonemap(D65, 0.7, Curve::Channels), tonemap(D65, 0.7, Curve::Hue));
        assert!((a[1] - b[1]).abs() < 0.01);
    }

    fn orange(scale: f64) -> Rgb {
        [4.0 * scale, 1.0 * scale, 0.2 * scale]
    }

    #[test]
    fn bright_orange_keeps_its_hue() {
        let rgb = hue_preserving(orange(1.0));
        assert!(rgb[0] / rgb[1] > 3.0 && rgb[1] / rgb[2] > 3.0);
        let film = orange(1.0).map(filmic);
        assert!(film[0] / film[1] < 1.5);
    }

    #[test]
    fn blinding_light_fades_to_white() {
        assert!(neutral(hue_preserving(orange(1e4))));
    }

    #[test]
    fn curve_is_monotonic_and_saturates() {
        let samples: Vec<f64> = (0..1000).map(|i| filmic(i as f64 * 0.02)).collect();
        assert!(samples.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(filmic(1e6), 1.0);
    }

    #[test]
    fn encoding_spans_the_byte() {
        assert_eq!((encode(0.0, 0.0), encode(1.0, 0.0)), (0, 255));
    }
}
