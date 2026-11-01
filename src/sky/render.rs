//! Turns a baked sky, a sun position and a camera into RGBA pixels.

use std::f64::consts::{FRAC_PI_2, PI};

use crate::camera::{direction, Camera};
use crate::parallel;
use crate::sky::Sky;
use crate::sky::spectrum::Xyz;
use crate::sky::tonemap::{encode, tonemap, Curve, Rgb};
use crate::vec3::{add, scale, Vec3};

/// Display value a daylight sky's mean radiance is exposed to, before
/// the curve.
const KEY: f64 = 0.15;
/// Mean radiance of a daylight sky in cd/m².
const DAYLIGHT: f64 = 5000.0;
/// How far exposure follows the sky as it dims, from 0 (fixed exposure)
/// to 1 (twilight as bright as noon).
const ADAPTATION: f64 = 0.8;
/// Mean sky radiance in cd/m² below which exposure stops adapting, so
/// night after dusk falls dark.
const DIMMEST: f64 = 0.01;
/// Samples per pixel along each axis.
const SUPERSAMPLING: usize = 3;
const DOME_ELEVATIONS: usize = 32;
const DOME_AZIMUTHS: usize = 64;

struct Scene<'a> {
    sky: &'a Sky,
    sun: Vec3,
    camera: &'a Camera,
    ground: Xyz,
    exposure: f64,
    curve: Curve,
}

/// Irradiance on horizontal ground from the sky dome, sun excluded.
fn dome_irradiance(sky: &Sky, sun: Vec3) -> Xyz {
    let (de, da) = (FRAC_PI_2 / DOME_ELEVATIONS as f64, 2.0 * PI / DOME_AZIMUTHS as f64);
    let patch = |i: usize| {
        let e = (i / DOME_AZIMUTHS) as f64 * de + 0.5 * de;
        let a = (i % DOME_AZIMUTHS) as f64 * da + 0.5 * da;
        let projected_solid_angle = e.sin() * e.cos() * de * da;
        scale(sky.scattered(sun, direction(a, e)), projected_solid_angle)
    };
    (0..DOME_ELEVATIONS * DOME_AZIMUTHS).map(patch).fold([0.0; 3], add)
}

/// Uniform noise in [-0.5, 0.5] from a hash of the pixel and channel.
fn noise(x: usize, y: usize, c: usize) -> f64 {
    let h = (x as u32).wrapping_mul(73_856_093)
        ^ (y as u32).wrapping_mul(19_349_663)
        ^ (c as u32).wrapping_mul(83_492_791);
    let h = h.wrapping_mul(2_654_435_761);
    (h >> 8) as f64 / (1u32 << 24) as f64 - 0.5
}

/// The observer stands on the ground, so below the horizon is ground.
fn radiance(scene: &Scene, dir: Vec3) -> Xyz {
    if dir[2] < 0.0 {
        scene.ground
    } else {
        scene.sky.radiance(scene.sun, dir)
    }
}

/// Tonemaps each sample before averaging, so the sun's edge, far too
/// bright for any blend of it to stay below white, still antialiases.
fn pixel(scene: &Scene, x: usize, y: usize, width: usize, height: usize) -> [u8; 4] {
    let n = SUPERSAMPLING;
    let sample = |i: usize| {
        let sx = x as f64 + ((i % n) as f64 + 0.5) / n as f64;
        let sy = y as f64 + ((i / n) as f64 + 0.5) / n as f64;
        let dir = scene.camera.ray(sx, sy, width as f64, height as f64)?;
        Some(tonemap(radiance(scene, dir), scene.exposure, scene.curve))
    };
    let samples: Vec<Rgb> = (0..n * n).filter_map(sample).collect();
    let sum = samples.iter().copied().fold([0.0; 3], add);
    let mean = scale(sum, 1.0 / samples.len().max(1) as f64);
    let alpha = (255 * samples.len() / (n * n)) as u8;
    let dither = |c: usize| noise(x, y, c) + noise(x + 7919, y, c);
    [encode(mean[0], dither(0)), encode(mean[1], dither(1)), encode(mean[2], dither(2)), alpha]
}

/// Exposure meters the mean sky radiance, partially adapting to dimmer
/// skies down to dusk; `ev` shifts from there in stops, and `curve`
/// fits the result to the display.
pub fn render(
    sky: &Sky,
    sun: Vec3,
    camera: &Camera,
    width: usize,
    height: usize,
    ev: f64,
    curve: Curve,
) -> Vec<u8> {
    let dome = dome_irradiance(sky, sun);
    let lit = add(dome, sky.sun.irradiance(sun[2].asin()));
    let ground = scale(lit, sky.params.albedo / PI);
    let mean = (dome[1] / PI).max(DIMMEST);
    let exposure = KEY * 2f64.powf(ev) / DAYLIGHT * (DAYLIGHT / mean).powf(ADAPTATION);
    let scene = Scene { sky, sun, camera, ground, exposure, curve };
    let row = |y: usize| -> Vec<[u8; 4]> {
        (0..width).map(|x| pixel(&scene, x, y, width, height)).collect()
    };
    parallel::map(height, row).concat().concat()
}
