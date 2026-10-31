//! Multiple scattering in Hillaire's (2020) approximation. Light reaching
//! a point after two or more bounces is treated as isotropic and nearly
//! uniform around it. Second order scattering L₂ and the fraction f that
//! one more bounce returns then sum as a geometric series, Ψ = L₂/(1 − f),
//! tabulated over altitude and sun zenith cosine for unit irradiance.

use std::f64::consts::PI;

use crate::atmosphere::{distance_to_ground, distance_to_top, hits_ground};
use crate::atmosphere::{Atmosphere, BOTTOM_RADIUS, TOP_RADIUS};
use crate::lut::Lut;
use crate::march::march;
use crate::spectrum::{Spectrum, N};
use crate::transmittance::Transmittance;
use crate::vec3::{add, dot, length, normalize, scale, Vec3};

const SIZE: usize = 32;
/// Directions per axis of the stratified sphere around each point.
const DIRECTIONS: usize = 8;
const STEPS: usize = 40;
const ISOTROPIC: f64 = 1.0 / (4.0 * PI);

pub struct MultiScattering(Lut);

/// Equal area sphere directions: uniform in z and azimuth.
fn direction(i: usize) -> Vec3 {
    let n = DIRECTIONS as f64;
    let z = 1.0 - 2.0 * ((i / DIRECTIONS) as f64 + 0.5) / n;
    let phi = 2.0 * PI * ((i % DIRECTIONS) as f64 + 0.5) / n;
    let s = (1.0 - z * z).sqrt();
    [s * phi.cos(), s * phi.sin(), z]
}

fn psi(atm: &Atmosphere, tr: &Transmittance, r: f64, mu_s: f64) -> Spectrum {
    let origin = [0.0, 0.0, r];
    let sun = [(1.0 - mu_s * mu_s).sqrt(), 0.0, mu_s];
    let mut second = [0.0; N];
    let mut transfer = [0.0; N];
    for dir in (0..DIRECTIONS * DIRECTIONS).map(direction) {
        let ground = hits_ground(r, dir[2]);
        let t_max = if ground {
            distance_to_ground(r, dir[2])
        } else {
            distance_to_top(r, dir[2])
        };
        let t = march(atm, origin, dir, t_max, STEPS, |p, d, w| {
            let rp = length(p);
            let to_sun = tr.to_space(rp, dot(p, sun) / rp);
            for k in 0..N {
                let (sr, sm) = atm.scattering(d, k);
                second[k] += w[k] * (sr + sm) * to_sun[k] * ISOTROPIC;
                transfer[k] += w[k] * (sr + sm);
            }
        });
        if ground {
            let mu = dot(normalize(add(origin, scale(dir, t_max))), sun);
            let to_sun = tr.to_space(BOTTOM_RADIUS, mu);
            for k in 0..N {
                second[k] += t[k] * to_sun[k] * mu.max(0.0) * atm.albedo / PI;
            }
        }
    }
    // Solid angle per direction times the isotropic phase at the origin.
    let weight = 1.0 / (DIRECTIONS * DIRECTIONS) as f64;
    std::array::from_fn(|k| second[k] * weight / (1.0 - transfer[k] * weight))
}

impl MultiScattering {
    pub fn new(atm: &Atmosphere, tr: &Transmittance) -> Self {
        Self(Lut::new(SIZE, SIZE, |u, v| {
            let r = BOTTOM_RADIUS + v * (TOP_RADIUS - BOTTOM_RADIUS);
            psi(atm, tr, r, 2.0 * u - 1.0)
        }))
    }

    /// Ψ at `altitude` with the sun at zenith cosine `mu_s`. Scattering
    /// coefficient × Ψ × solar irradiance is the multiply scattered
    /// radiance a point sends in any direction.
    pub fn at(&self, altitude: f64, mu_s: f64) -> Spectrum {
        let v = altitude / (TOP_RADIUS - BOTTOM_RADIUS);
        self.0.sample(0.5 * (mu_s + 1.0), v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atmosphere::Params;

    fn ground_psi(albedo: f64) -> Spectrum {
        let p = Params { aerosol: 0.1, humidity: 0.5, ozone: 300.0, albedo };
        let atm = Atmosphere::new(p);
        psi(&atm, &Transmittance::new(&atm), BOTTOM_RADIUS + 2.0, 0.7)
    }

    #[test]
    fn brighter_ground_adds_multiple_scattering() {
        let (dark, bright) = (ground_psi(0.0), ground_psi(0.8));
        assert!((0..N).all(|k| bright[k] > dark[k] && dark[k] > 0.0));
    }

    #[test]
    fn directions_cover_the_sphere_evenly() {
        let n = DIRECTIONS * DIRECTIONS;
        let mean = (0..n).map(direction).fold([0.0; 3], add);
        assert!(length(mean) < 1e-9);
        assert!((0..n).all(|i| (length(direction(i)) - 1.0).abs() < 1e-12));
    }
}
