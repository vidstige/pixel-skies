//! The solar disk: its radiance after extinction, tabulated over
//! elevation as seen from the ground.

use std::f64::consts::{FRAC_PI_2, PI};

use crate::lut::{locate, unit};
use crate::scattering::OBSERVER_RADIUS;
use crate::spectrum::{to_xyz, Xyz, SOLAR_IRRADIANCE};
use crate::transmittance::Transmittance;
use crate::vec3::{add, scale};

/// 0.2666°, half the sun's mean apparent diameter.
pub const ANGULAR_RADIUS: f64 = 0.004_653;
pub const SIZE: usize = 256;
/// Linear limb darkening coefficient for visible light.
const LIMB_DARKENING: f64 = 0.6;

pub struct Sun {
    pub radiance: Vec<Xyz>,
}

fn solid_angle() -> f64 {
    2.0 * PI * (1.0 - ANGULAR_RADIUS.cos())
}

/// Squared so nodes crowd the horizon, where extinction changes fastest.
fn elevation(u: f64) -> f64 {
    FRAC_PI_2 * u * u
}

impl Sun {
    pub fn new(tr: &Transmittance) -> Self {
        let radiance = |i: usize| {
            let t = tr.to_space(OBSERVER_RADIUS, elevation(unit(i, SIZE)).sin());
            let l = std::array::from_fn(|k| SOLAR_IRRADIANCE[k] * t[k] / solid_angle());
            to_xyz(&l)
        };
        Self { radiance: (0..SIZE).map(radiance).collect() }
    }

    /// Mean radiance of the disk centred at `elevation`.
    fn mean(&self, elevation: f64) -> Xyz {
        if elevation < 0.0 {
            return [0.0; 3];
        }
        let (i, f) = locate((elevation / FRAC_PI_2).sqrt(), SIZE);
        add(scale(self.radiance[i], 1.0 - f), scale(self.radiance[i + 1], f))
    }

    /// Irradiance on horizontal ground from a sun at `elevation`.
    pub fn irradiance(&self, elevation: f64) -> Xyz {
        scale(self.mean(elevation), solid_angle() * elevation.sin().max(0.0))
    }

    /// Radiance along a direction at `elevation` and `angle` off the
    /// disk's centre. Looking extinction up per direction rather than at
    /// the centre reddens the lower rim of a setting sun.
    pub fn disk(&self, elevation: f64, angle: f64) -> Xyz {
        let x = angle / ANGULAR_RADIUS;
        if x >= 1.0 {
            return [0.0; 3];
        }
        let mu = (1.0 - x * x).sqrt();
        let darkening = (1.0 - LIMB_DARKENING * (1.0 - mu)) / (1.0 - LIMB_DARKENING / 3.0);
        scale(self.mean(elevation), darkening)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atmosphere::{Atmosphere, Params};

    fn sun() -> Sun {
        let p = Params { aerosol: 0.1, humidity: 0.5, ozone: 300.0, albedo: 0.2 };
        Sun::new(&Transmittance::new(&Atmosphere::new(p)))
    }

    #[test]
    fn limb_darkening_keeps_the_mean_radiance() {
        let sun = sun();
        let n = 10_000;
        let dx = 1.0 / n as f64;
        // Mean over the disk area, in rings of radius x.
        let ring = |i: usize| {
            let x = (i as f64 + 0.5) * dx;
            sun.disk(1.0, x * ANGULAR_RADIUS)[1] * 2.0 * x * dx
        };
        let mean: f64 = (0..n).map(ring).sum();
        assert!((mean / sun.mean(1.0)[1] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn low_sun_is_redder_and_dimmer() {
        let sun = sun();
        let (low, high) = (sun.mean(0.02), sun.mean(1.0));
        assert!(low[2] / low[0] < 0.5 * high[2] / high[0]);
        assert!(low[1] < high[1]);
    }

    #[test]
    fn noon_sunlight_is_around_a_hundred_thousand_lux() {
        let lux = sun().irradiance(FRAC_PI_2)[1];
        assert!((80_000.0..130_000.0).contains(&lux), "{lux}");
    }
}
