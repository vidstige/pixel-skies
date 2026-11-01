//! Lookup tables of spectra over the unit square, sampled bilinearly.

use crate::parallel;
use crate::sky::spectrum::Spectrum;

pub struct Lut {
    width: usize,
    height: usize,
    cells: Vec<Spectrum>,
}

/// Coordinate in [0, 1] of node `i` out of `n`; nodes include both ends.
pub fn unit(i: usize, n: usize) -> f64 {
    i as f64 / (n - 1) as f64
}

/// Lower node and fraction towards the next for coordinate `u`, clamped.
pub fn locate(u: f64, n: usize) -> (usize, f64) {
    let x = u.clamp(0.0, 1.0) * (n - 1) as f64;
    let i = (x as usize).min(n - 2);
    (i, x - i as f64)
}

impl Lut {
    pub fn new(
        width: usize,
        height: usize,
        f: impl Fn(f64, f64) -> Spectrum + Sync,
    ) -> Self {
        let cell = |i: usize| f(unit(i % width, width), unit(i / width, height));
        Self { width, height, cells: parallel::map(width * height, cell) }
    }

    pub fn sample(&self, u: f64, v: f64) -> Spectrum {
        let (x, fx) = locate(u, self.width);
        let (y, fy) = locate(v, self.height);
        let at = |dx: usize, dy: usize| &self.cells[(y + dy) * self.width + x + dx];
        let (a, b, c, d) = (at(0, 0), at(1, 0), at(0, 1), at(1, 1));
        std::array::from_fn(|k| {
            let bottom = a[k] + (b[k] - a[k]) * fx;
            let top = c[k] + (d[k] - c[k]) * fx;
            bottom + (top - bottom) * fy
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_functions_are_reproduced_between_nodes() {
        let lut = Lut::new(5, 7, |u, v| [2.0 * u + 3.0 * v + u * v; crate::sky::spectrum::N]);
        let (u, v) = (0.37, 0.81);
        assert!((lut.sample(u, v)[0] - (2.0 * u + 3.0 * v + u * v)).abs() < 1e-12);
    }
}
