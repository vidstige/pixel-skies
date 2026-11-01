//! Light scattered towards an observer on the ground, precomputed over
//! view elevation, sun elevation and the azimuth between them.
//!
//! As in Bruneton and Neyret (2008), single scattering is stored without
//! its phase function, which is applied per pixel instead: the integrand
//! is then smooth in all three coordinates and a coarse table holds the
//! sharp forward peak of Mie scattering exactly. Multiple scattering is
//! smooth already and is stored as is. Everything is converted to XYZ at
//! the end, which is exact since all that follows is linear in radiance.

use std::f64::consts::{FRAC_PI_2, PI};

use crate::sky::atmosphere::{distance_to_top, Atmosphere, BOTTOM_RADIUS};
use crate::sky::lut::{locate, unit};
use crate::sky::march::march;
use crate::sky::multiscattering::MultiScattering;
use crate::parallel;
use crate::sky::spectrum::{to_xyz, Xyz, N, SOLAR_IRRADIANCE};
use crate::sky::transmittance::Transmittance;
use crate::vec3::{dot, length};

pub const VIEW: usize = 128;
pub const SUN: usize = 96;
pub const AZIMUTH: usize = 64;
pub const CELLS: usize = VIEW * SUN * AZIMUTH;
pub const OBSERVER_RADIUS: f64 = BOTTOM_RADIUS + 2.0;
const STEPS: usize = 128;
/// Below this the sky is dark enough to treat as night.
const LOWEST_SUN: f64 = -20.0 * PI / 180.0;

/// Rayleigh and Mie single scattering without phase, then multiple
/// scattering, each in XYZ.
pub type Sample = [Xyz; 3];

pub struct Scattering {
    pub cells: Vec<Sample>,
}

/// Squared so that nodes crowd the horizon, where the sky changes fastest.
fn view_elevation(u: f64) -> f64 {
    FRAC_PI_2 * (1.0 - u).powi(2)
}

fn view_unit(elevation: f64) -> f64 {
    1.0 - (elevation.max(0.0) / FRAC_PI_2).sqrt()
}

/// Signed square root of elevation, so nodes crowd sunrise and sunset.
fn sun_lowest() -> f64 {
    -(-LOWEST_SUN / FRAC_PI_2).sqrt()
}

fn sun_elevation(u: f64) -> f64 {
    let t = sun_lowest() + u * (1.0 - sun_lowest());
    FRAC_PI_2 * t * t.abs()
}

fn sun_unit(elevation: f64) -> f64 {
    let t = elevation.signum() * (elevation.abs() / FRAC_PI_2).sqrt();
    (t - sun_lowest()) / (1.0 - sun_lowest())
}

fn index(view: usize, sun: usize, azimuth: usize) -> usize {
    (view * SUN + sun) * AZIMUTH + azimuth
}

fn cell(
    atm: &Atmosphere,
    tr: &Transmittance,
    ms: &MultiScattering,
    view: f64,
    sun: f64,
    azimuth: f64,
) -> Sample {
    let dir = [view.cos(), 0.0, view.sin()];
    let to_sun = [sun.cos() * azimuth.cos(), sun.cos() * azimuth.sin(), sun.sin()];
    let t_max = distance_to_top(OBSERVER_RADIUS, dir[2]);
    let origin = [0.0, 0.0, OBSERVER_RADIUS];
    let (mut rayleigh, mut mie, mut multiple) = ([0.0; N], [0.0; N], [0.0; N]);
    march(atm, origin, dir, t_max, STEPS, |p, d, w| {
        let r = length(p);
        let mu = dot(p, to_sun) / r;
        let direct = tr.to_space(r, mu);
        let psi = ms.at(r - BOTTOM_RADIUS, mu);
        for k in 0..N {
            let (sr, sm) = atm.scattering(d, k);
            rayleigh[k] += w[k] * sr * direct[k];
            mie[k] += w[k] * sm * direct[k];
            multiple[k] += w[k] * (sr + sm) * psi[k];
        }
    });
    [rayleigh, mie, multiple]
        .map(|s| to_xyz(&std::array::from_fn(|k| s[k] * SOLAR_IRRADIANCE[k])))
}

impl Scattering {
    pub fn new(atm: &Atmosphere, tr: &Transmittance, ms: &MultiScattering) -> Self {
        let cells = parallel::map(CELLS, |i| {
            let view = view_elevation(unit(i / (SUN * AZIMUTH), VIEW));
            let sun = sun_elevation(unit(i / AZIMUTH % SUN, SUN));
            let azimuth = PI * unit(i % AZIMUTH, AZIMUTH);
            cell(atm, tr, ms, view, sun, azimuth)
        });
        Self { cells }
    }

    /// Trilinear lookup. `azimuth` is the angle in [0, π] between the
    /// horizontal directions of view and sun.
    pub fn sample(&self, view: f64, sun: f64, azimuth: f64) -> Sample {
        let (v, fv) = locate(view_unit(view), VIEW);
        let (s, fs) = locate(sun_unit(sun), SUN);
        let (a, fa) = locate(azimuth / PI, AZIMUTH);
        let mut out = [[0.0; 3]; 3];
        for corner in 0..8 {
            let (dv, ds, da) = (corner & 1, corner >> 1 & 1, corner >> 2);
            let weight = |d: usize, f: f64| if d == 0 { 1.0 - f } else { f };
            let w = weight(dv, fv) * weight(ds, fs) * weight(da, fa);
            let c = &self.cells[index(v + dv, s + ds, a + da)];
            for (o, x) in out.iter_mut().flatten().zip(c.iter().flatten()) {
                *o += w * x;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sky::atmosphere::Params;

    fn sky(view: f64, sun: f64, azimuth: f64) -> Sample {
        let p = Params { aerosol: 0.1, humidity: 0.5, ozone: 300.0, albedo: 0.2 };
        let atm = Atmosphere::new(p);
        let tr = Transmittance::new(&atm);
        let ms = MultiScattering::new(&atm, &tr);
        cell(&atm, &tr, &ms, view.to_radians(), sun.to_radians(), azimuth.to_radians())
    }

    /// Blue over red in XYZ: z̄ peaks in the blue and x̄ mostly in the red.
    fn blueness(xyz: Xyz) -> f64 {
        xyz[2] / xyz[0]
    }

    #[test]
    fn elevation_mappings_round_trip() {
        for u in [0.0, 0.13, 0.5, 0.97, 1.0] {
            assert!((view_unit(view_elevation(u)) - u).abs() < 1e-12);
            assert!((sun_unit(sun_elevation(u)) - u).abs() < 1e-12);
        }
    }

    #[test]
    fn sun_nodes_span_twilight_to_zenith() {
        assert!((sun_elevation(0.0) - LOWEST_SUN).abs() < 1e-12);
        assert!((sun_elevation(1.0) - FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn rayleigh_light_is_bluer_than_mie_light() {
        let [rayleigh, mie, _] = sky(40.0, 40.0, 90.0);
        assert!(blueness(rayleigh) > blueness(mie));
    }

    #[test]
    fn sky_towards_the_setting_sun_is_redder_than_at_noon() {
        let noon = sky(2.0, 60.0, 0.0)[0];
        let sunset = sky(2.0, 1.0, 0.0)[0];
        assert!(blueness(sunset) < 0.7 * blueness(noon));
    }

    #[test]
    fn twilight_sky_is_lit_only_by_what_reaches_above_the_shadow() {
        let day = sky(30.0, 10.0, 90.0)[0][1];
        let dusk = sky(30.0, -6.0, 90.0)[0][1];
        assert!(dusk > 0.0 && dusk < 0.01 * day);
    }
}
