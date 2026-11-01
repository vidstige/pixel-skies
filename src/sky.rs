//! A baked sky: everything precomputed for one atmosphere, ready to be
//! looked up for any sun position and view direction.

pub mod atmosphere;
mod lut;
mod march;
mod multiscattering;
mod phase;
pub mod render;
mod scattering;
pub mod sky_io;
mod spectrum;
mod sun;
pub mod tonemap;
mod transmittance;

use crate::vec3::{add, dot, scale, Vec3};
use atmosphere::{mie_asymmetry, Atmosphere, Params};
use multiscattering::MultiScattering;
use phase::{cornette_shanks, rayleigh};
use scattering::Scattering;
use spectrum::Xyz;
use sun::Sun;
use transmittance::Transmittance;

pub struct Sky {
    pub params: Params,
    pub scattering: Scattering,
    pub sun: Sun,
}

impl Sky {
    pub fn bake(params: Params) -> Self {
        let atm = Atmosphere::new(params);
        let tr = Transmittance::new(&atm);
        let ms = MultiScattering::new(&atm, &tr);
        let scattering = Scattering::new(&atm, &tr, &ms);
        Self { params, scattering, sun: Sun::new(&tr) }
    }

    /// Light the air sends from direction `dir`, above the horizon, with
    /// the sun towards `sun`.
    pub fn scattered(&self, sun: Vec3, dir: Vec3) -> Xyz {
        let cross = sun[0] * dir[1] - sun[1] * dir[0];
        let azimuth = cross.atan2(sun[0] * dir[0] + sun[1] * dir[1]).abs();
        let [r, m, ms] = self.scattering.sample(dir[2].asin(), sun[2].asin(), azimuth);
        let nu = dot(sun, dir);
        let g = mie_asymmetry(self.params.humidity);
        add(add(scale(r, rayleigh(nu)), scale(m, cornette_shanks(nu, g))), ms)
    }

    /// Scattered light plus the solar disk.
    pub fn radiance(&self, sun: Vec3, dir: Vec3) -> Xyz {
        let angle = dot(sun, dir).clamp(-1.0, 1.0).acos();
        add(self.scattered(sun, dir), self.sun.disk(dir[2].asin(), angle))
    }
}
