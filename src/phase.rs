//! Phase functions over the cosine `nu` of the scattering angle.

use std::f64::consts::PI;

pub fn rayleigh(nu: f64) -> f64 {
    3.0 / (16.0 * PI) * (1.0 + nu * nu)
}

/// Cornette-Shanks: Henyey-Greenstein with a Rayleigh-like correction
/// that behaves better for aerosols.
pub fn cornette_shanks(nu: f64, g: f64) -> f64 {
    let k = 3.0 / (8.0 * PI) * (1.0 - g * g) / (2.0 + g * g);
    k * (1.0 + nu * nu) / (1.0 + g * g - 2.0 * g * nu).powf(1.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Integral over the sphere, in cos θ.
    fn total(f: impl Fn(f64) -> f64) -> f64 {
        let n = 200_000;
        let dnu = 2.0 / n as f64;
        2.0 * PI * (0..n).map(|i| f(-1.0 + (i as f64 + 0.5) * dnu) * dnu).sum::<f64>()
    }

    #[test]
    fn rayleigh_integrates_to_one() {
        assert!((total(rayleigh) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cornette_shanks_integrates_to_one() {
        for g in [0.0, 0.5, 0.8] {
            assert!((total(|nu| cornette_shanks(nu, g)) - 1.0).abs() < 1e-3, "{g}");
        }
    }
}
