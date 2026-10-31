//! Earth's atmosphere: the shell it fills, how dense each constituent is
//! with altitude, and the optical coefficients set by aerosol load,
//! humidity and ozone.

use crate::spectrum::{self, Spectrum, OZONE_CROSS_SECTION};

pub const BOTTOM_RADIUS: f64 = 6_360_000.0;
pub const TOP_RADIUS: f64 = 6_460_000.0;
const RAYLEIGH_SCALE_HEIGHT: f64 = 8000.0;
const MIE_SCALE_HEIGHT: f64 = 1200.0;
/// Rayleigh scattering at sea level is this times λ⁻⁴, λ in µm, per m.
const RAYLEIGH: f64 = 1.24062e-6;
const OZONE_PEAK: f64 = 25_000.0;
/// Half width of the ozone tent, which is also its column integral.
const OZONE_WIDTH: f64 = 15_000.0;
/// Molecules per m² in one Dobson unit.
const DOBSON_UNIT: f64 = 2.687e20;
const DRY_ANGSTROM: f64 = 1.3;
const DRY_SINGLE_SCATTERING_ALBEDO: f64 = 0.9;
const HANEL_EXPONENT: f64 = 0.4;
const DRY_ASYMMETRY: f64 = 0.7;
const WET_ASYMMETRY: f64 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Optical depth of the dry aerosol at 550 nm.
    pub aerosol: f64,
    /// Relative humidity in [0, 1).
    pub humidity: f64,
    /// Ozone column in Dobson units.
    pub ozone: f64,
    /// Ground albedo.
    pub albedo: f64,
}

/// Number densities relative to their peak: Rayleigh, Mie, ozone.
pub type Density = [f64; 3];

/// Coefficients per metre, at each constituent's peak density.
pub struct Atmosphere {
    pub albedo: f64,
    rayleigh: Spectrum,
    mie_scattering: Spectrum,
    mie_extinction: Spectrum,
    ozone: Spectrum,
}

pub fn density(altitude: f64) -> Density {
    [
        (-altitude / RAYLEIGH_SCALE_HEIGHT).exp(),
        (-altitude / MIE_SCALE_HEIGHT).exp(),
        (1.0 - (altitude - OZONE_PEAK).abs() / OZONE_WIDTH).max(0.0),
    ]
}

fn discriminant(r: f64, mu: f64, radius: f64) -> f64 {
    r * r * (mu * mu - 1.0) + radius * radius
}

/// Distance from radius `r` along zenith cosine `mu` to the top.
pub fn distance_to_top(r: f64, mu: f64) -> f64 {
    -r * mu + discriminant(r, mu, TOP_RADIUS).max(0.0).sqrt()
}

pub fn distance_to_ground(r: f64, mu: f64) -> f64 {
    -r * mu - discriminant(r, mu, BOTTOM_RADIUS).max(0.0).sqrt()
}

pub fn hits_ground(r: f64, mu: f64) -> bool {
    mu < 0.0 && discriminant(r, mu, BOTTOM_RADIUS) >= 0.0
}

/// Hänel's growth of aerosol scattering as particles take up water.
fn growth(humidity: f64) -> f64 {
    (1.0 - humidity).powf(-HANEL_EXPONENT)
}

/// Swollen droplets scatter more strongly forward.
pub fn mie_asymmetry(humidity: f64) -> f64 {
    let wet = 1.0 - 1.0 / growth(humidity);
    DRY_ASYMMETRY + (WET_ASYMMETRY - DRY_ASYMMETRY) * wet
}

impl Atmosphere {
    /// Water adds scattering but no absorption, and the larger droplets
    /// scatter more greyly, so the Ångström exponent falls with growth.
    pub fn new(p: Params) -> Self {
        let f = growth(p.humidity);
        let column = p.aerosol / MIE_SCALE_HEIGHT;
        let angstrom = |l: f64, alpha: f64| (l / 550.0).powf(-alpha);
        let albedo = DRY_SINGLE_SCATTERING_ALBEDO;
        let mie_scattering = spectrum::from_fn(|l| {
            column * albedo * f * angstrom(l, DRY_ANGSTROM / f)
        });
        let absorption =
            spectrum::from_fn(|l| column * (1.0 - albedo) * angstrom(l, DRY_ANGSTROM));
        let ozone = p.ozone * DOBSON_UNIT / OZONE_WIDTH;
        Self {
            albedo: p.albedo,
            rayleigh: spectrum::from_fn(|l| RAYLEIGH * (l / 1000.0).powi(-4)),
            mie_extinction: std::array::from_fn(|k| mie_scattering[k] + absorption[k]),
            mie_scattering,
            ozone: OZONE_CROSS_SECTION.map(|sigma| sigma * ozone),
        }
    }

    /// Rayleigh and Mie scattering coefficients in wavelength bin `k`.
    pub fn scattering(&self, d: Density, k: usize) -> (f64, f64) {
        (self.rayleigh[k] * d[0], self.mie_scattering[k] * d[1])
    }

    pub fn extinction(&self, d: Density, k: usize) -> f64 {
        self.rayleigh[k] * d[0] + self.mie_extinction[k] * d[1] + self.ozone[k] * d[2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(humidity: f64) -> Params {
        Params { aerosol: 0.1, humidity, ozone: 300.0, albedo: 0.2 }
    }

    /// Optical depth of the Mie layer at 550 nm, summed over the column.
    fn mie_depth(p: Params) -> f64 {
        let atm = Atmosphere::new(p);
        let k = (0..spectrum::N).find(|&k| spectrum::wavelength(k) == 550.0).unwrap();
        let mie = |h: f64| atm.mie_extinction[k] * density(h)[1];
        (0..10_000).map(|i| mie(i as f64 + 0.5)).sum()
    }

    #[test]
    fn dry_aerosol_depth_matches_the_parameter() {
        assert!((mie_depth(params(0.0)) - 0.1).abs() < 1e-3);
    }

    #[test]
    fn humidity_thickens_the_haze() {
        assert!(mie_depth(params(0.9)) > 2.0 * mie_depth(params(0.0)));
    }

    #[test]
    fn ozone_column_matches_the_parameter() {
        let column: f64 = (0..50_000).map(|h| density(h as f64 + 0.5)[2]).sum();
        assert!((column - OZONE_WIDTH).abs() < 1.0);
    }

    #[test]
    fn horizontal_ray_at_the_ground_grazes_but_does_not_hit_it() {
        assert!(!hits_ground(BOTTOM_RADIUS + 1.0, 0.0));
        assert!(hits_ground(BOTTOM_RADIUS + 1.0, -0.01));
    }
}
