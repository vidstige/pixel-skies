//! From scene XYZ to display sRGB: exposure, a filmic curve, and sRGB
//! encoding.

use crate::sky::spectrum::Xyz;
use crate::vec3::{dot, Vec3};

/// Linear sRGB in [0, 1].
pub type Rgb = [f64; 3];

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

pub fn tonemap(xyz: Xyz, exposure: f64) -> Rgb {
    XYZ_TO_SRGB.map(|row| filmic((dot(row, xyz) * exposure).max(0.0)))
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

    #[test]
    fn d65_white_maps_to_neutral_grey() {
        let rgb = tonemap([0.9505, 1.0, 1.089], 1.0);
        assert!((rgb[0] - rgb[1]).abs() < 1e-3 && (rgb[1] - rgb[2]).abs() < 1e-3);
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
