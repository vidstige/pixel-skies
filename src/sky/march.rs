//! Ray marching through the atmosphere.

use crate::sky::atmosphere::{density, Atmosphere, Density, BOTTOM_RADIUS};
use crate::sky::spectrum::{Spectrum, N};
use crate::vec3::{add, length, scale, Vec3};

/// Marches `steps` segments from `origin` along `dir` out to `t_max`,
/// spaced quadratically since air is densest near where rays start.
/// Calls `visit` at each segment's midpoint with its densities and the
/// segment's weight ∫T dt, integrated exactly for the segment's
/// extinction (Hillaire 2015), so scattering coefficient × weight is the
/// light the segment sends back. Returns the transmittance to `t_max`.
pub fn march(
    atm: &Atmosphere,
    origin: Vec3,
    dir: Vec3,
    t_max: f64,
    steps: usize,
    mut visit: impl FnMut(Vec3, Density, &Spectrum),
) -> Spectrum {
    let at = |i: usize| t_max * (i as f64 / steps as f64).powi(2);
    let mut transmittance = [1.0; N];
    for i in 0..steps {
        let (t0, t1) = (at(i), at(i + 1));
        let p = add(origin, scale(dir, 0.5 * (t0 + t1)));
        let d = density(length(p) - BOTTOM_RADIUS);
        let mut weight = [0.0; N];
        for k in 0..N {
            let e = atm.extinction(d, k);
            let step = (-e * (t1 - t0)).exp();
            weight[k] = transmittance[k] * (1.0 - step) / e;
            transmittance[k] *= step;
        }
        visit(p, d, &weight);
    }
    transmittance
}
