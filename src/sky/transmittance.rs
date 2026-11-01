//! Transmittance from any point to space, tabulated over radius and zenith
//! cosine in Bruneton's (2017) parametrization, which spends its texels
//! on rays that miss the ground and packs them densely near the horizon.

use crate::sky::atmosphere::{distance_to_top, hits_ground, Atmosphere};
use crate::sky::atmosphere::{BOTTOM_RADIUS, TOP_RADIUS};
use crate::sky::lut::Lut;
use crate::sky::march::march;
use crate::sky::spectrum::{Spectrum, N};

const WIDTH: usize = 256;
const HEIGHT: usize = 64;
const STEPS: usize = 200;

pub struct Transmittance(Lut);

/// Distance to the top along the horizon, from the ground.
fn horizon() -> f64 {
    (TOP_RADIUS * TOP_RADIUS - BOTTOM_RADIUS * BOTTOM_RADIUS).sqrt()
}

fn to_unit(r: f64, mu: f64) -> (f64, f64) {
    let rho = (r * r - BOTTOM_RADIUS * BOTTOM_RADIUS).max(0.0).sqrt();
    let (d_min, d_max) = (TOP_RADIUS - r, rho + horizon());
    ((distance_to_top(r, mu) - d_min) / (d_max - d_min), rho / horizon())
}

fn from_unit(u: f64, v: f64) -> (f64, f64) {
    let h = horizon();
    let rho = h * v;
    let r = (rho * rho + BOTTOM_RADIUS * BOTTOM_RADIUS).sqrt();
    let (d_min, d_max) = (TOP_RADIUS - r, rho + h);
    let d = d_min + u * (d_max - d_min);
    let mu = if d == 0.0 { 1.0 } else { (h * h - rho * rho - d * d) / (2.0 * r * d) };
    (r, mu.clamp(-1.0, 1.0))
}

fn integrate(atm: &Atmosphere, r: f64, mu: f64) -> Spectrum {
    let dir = [(1.0 - mu * mu).sqrt(), 0.0, mu];
    march(atm, [0.0, 0.0, r], dir, distance_to_top(r, mu), STEPS, |_, _, _| {})
}

impl Transmittance {
    pub fn new(atm: &Atmosphere) -> Self {
        Self(Lut::new(WIDTH, HEIGHT, |u, v| {
            let (r, mu) = from_unit(u, v);
            integrate(atm, r, mu)
        }))
    }

    /// Transmittance to space from radius `r` along zenith cosine `mu`,
    /// zero where the planet is in the way.
    pub fn to_space(&self, r: f64, mu: f64) -> Spectrum {
        if hits_ground(r, mu) {
            return [0.0; N];
        }
        let (u, v) = to_unit(r, mu);
        self.0.sample(u, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sky::atmosphere::Params;

    fn atmosphere() -> Atmosphere {
        Atmosphere::new(Params { aerosol: 0.1, humidity: 0.5, ozone: 300.0, albedo: 0.2 })
    }

    #[test]
    fn unit_coordinates_round_trip() {
        let (r, mu) = (BOTTOM_RADIUS + 3000.0, 0.2);
        let (u, v) = to_unit(r, mu);
        let (r2, mu2) = from_unit(u, v);
        assert!((r - r2).abs() < 1e-3 && (mu - mu2).abs() < 1e-9);
    }

    #[test]
    fn table_matches_direct_integration_off_the_nodes() {
        let atm = atmosphere();
        let table = Transmittance::new(&atm);
        let (r, mu) = (BOTTOM_RADIUS + 1234.0, 0.0731);
        let (a, b) = (table.to_space(r, mu), integrate(&atm, r, mu));
        assert!((0..N).all(|k| (a[k] - b[k]).abs() < 2e-3 * b[k].max(1e-3)));
    }

    #[test]
    fn red_light_gets_through_better_than_blue_at_the_horizon() {
        let atm = atmosphere();
        let t = integrate(&atm, BOTTOM_RADIUS + 2.0, 0.0);
        assert!(t[34] > 10.0 * t[10]);
    }
}
