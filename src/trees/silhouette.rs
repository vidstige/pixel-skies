//! Rasterizes trees to black silhouettes on transparent pixels, seen
//! through the same camera as the sky.
//!
//! Each limb is a tapered capsule whose pixel coverage is estimated from
//! its distance field: full inside, ramping over a pixel at the edge, and
//! scaled by width for limbs thinner than a pixel. Overlapping limbs
//! combine as if they covered independent parts of a pixel, so a
//! distant crown of many faint shoots still adds up to a solid shape.

use std::thread;

use crate::camera::Camera;
use crate::parallel;
use crate::trees::spruce::{grow, Limb, Spruce};
use crate::vec3::{add, scale, Vec3};

/// Where the observer's eye stands by default, in meters.
pub const EYE: Vec3 = [0.0, 0.0, 1.7];
/// Samples per pixel along each axis for the forest floor.
const FLOOR_SAMPLES: usize = 3;

type Pixel = [f64; 2];

struct View<'a> {
    camera: &'a Camera,
    eye: Vec3,
    width: usize,
    height: usize,
}

fn sub(a: Pixel, b: Pixel) -> Pixel {
    [a[0] - b[0], a[1] - b[1]]
}

fn dot(a: Pixel, b: Pixel) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// Coverage of pixel `p` by a capsule from `a` to `b` with radii `ra`
/// and `rb`, all in pixels.
fn coverage(p: Pixel, a: Pixel, b: Pixel, ra: f64, rb: f64) -> f64 {
    let (ab, ap) = (sub(b, a), sub(p, a));
    let t = (dot(ap, ab) / dot(ab, ab).max(1e-12)).clamp(0.0, 1.0);
    let d = (ap[0] - t * ab[0]).hypot(ap[1] - t * ab[1]);
    let r = ra + (rb - ra) * t;
    (r + 0.5 - d).clamp(0.0, (2.0 * r).min(1.0))
}

/// Multiplies the light passing each pixel of `clear` by what `limb`
/// leaves of it.
fn splat(clear: &mut [f64], view: &View, limb: &Limb) {
    let (w, h) = (view.width as f64, view.height as f64);
    let project = |p: Vec3| view.camera.project(add(p, scale(view.eye, -1.0)), w, h);
    let (Some(a), Some(b)) = (project(limb.a), project(limb.b)) else {
        return;
    };
    let (ra, rb) = (limb.ra * a[2], limb.rb * b[2]);
    let (a, b) = ([a[0], a[1]], [b[0], b[1]]);
    let reach = ra.max(rb) + 1.0;
    let lo = |i: usize| (a[i].min(b[i]) - reach).max(0.0) as usize;
    let hi = |i: usize, n: usize| ((a[i].max(b[i]) + reach).max(0.0) as usize).min(n);
    for y in lo(1)..hi(1, view.height) {
        for x in lo(0)..hi(0, view.width) {
            let c = coverage([x as f64 + 0.5, y as f64 + 0.5], a, b, ra, rb);
            clear[y * view.width + x] *= 1.0 - c;
        }
    }
}

/// Fraction of pixel (x, y) that looks down on ground `inside` the forest.
fn floor(view: &View, x: usize, y: usize, inside: &impl Fn(Vec3) -> bool) -> f64 {
    let n = FLOOR_SAMPLES;
    let hit = |i: usize| {
        let sx = x as f64 + ((i % n) as f64 + 0.5) / n as f64;
        let sy = y as f64 + ((i / n) as f64 + 0.5) / n as f64;
        let dir = view.camera.ray(sx, sy, view.width as f64, view.height as f64)?;
        (dir[2] < 0.0).then(|| add(view.eye, scale(dir, view.eye[2] / -dir[2])))
    };
    let count = (0..n * n).filter_map(hit).filter(|&p| inside(p)).count();
    count as f64 / (n * n) as f64
}

/// Light passing each pixel past every `stride`th tree from `first`.
fn layer(trees: &[Spruce], first: usize, stride: usize, view: &View) -> Vec<f64> {
    let mut clear = vec![1.0; view.width * view.height];
    for tree in trees.iter().skip(first).step_by(stride) {
        for limb in grow(tree) {
            splat(&mut clear, view, &limb);
        }
    }
    clear
}

/// RGBA pixels, black where trees or forest floor `inside` cover them,
/// seen from `eye`. Trees splat in a layer per thread, since coverage
/// multiplies in any order.
pub fn render(
    trees: &[Spruce],
    inside: impl Fn(Vec3) -> bool + Sync,
    camera: &Camera,
    eye: Vec3,
    width: usize,
    height: usize,
) -> Vec<u8> {
    let view = View { camera, eye, width, height };
    let threads = thread::available_parallelism().map_or(1, |t| t.get());
    let layers = parallel::map(threads, |i| layer(trees, i, threads, &view));
    let row = |y: usize| -> Vec<u8> {
        let alpha = |x: usize| {
            let clear = 1.0 - floor(&view, x, y, &inside);
            let clear = layers.iter().fold(clear, |c, layer| c * layer[y * width + x]);
            [0, 0, 0, (255.0 * (1.0 - clear)).round() as u8]
        };
        (0..width).flat_map(alpha).collect()
    };
    parallel::map(height, row).concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Summed coverage down a pixel column crossing a horizontal capsule.
    fn column(radius: f64) -> f64 {
        let (a, b) = ([-10.0, 20.3], [10.0, 20.3]);
        (0..40).map(|y| coverage([0.5, y as f64 + 0.5], a, b, radius, radius)).sum()
    }

    #[test]
    fn coverage_adds_up_to_the_width_of_thick_and_thin_limbs() {
        for radius in [0.1, 0.3, 2.0, 5.5] {
            assert!((column(radius) - 2.0 * radius).abs() < 0.1, "radius {radius}");
        }
    }

    #[test]
    fn coverage_is_full_inside_and_none_beyond_a_pixel_outside() {
        let (a, b) = ([0.0, 0.0], [10.0, 0.0]);
        assert_eq!(coverage([5.0, 1.0], a, b, 2.0, 2.0), 1.0);
        assert_eq!(coverage([5.0, 2.6], a, b, 2.0, 2.0), 0.0);
        assert_eq!(coverage([12.6, 0.0], a, b, 2.0, 2.0), 0.0);
    }
}
