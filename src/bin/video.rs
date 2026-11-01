//! Renders a sped-up sunset over a treeline as raw RGBA frames on stdout,
//! for ffmpeg to encode. The sun sinks along its slanted path while the
//! camera trucks slowly sideways, so the forest shifts against the sky.

use std::f64::consts::FRAC_PI_2;
use std::io::{self, Write};
use std::time::Instant;
use std::{env, fs};

use pixel_skies::args::Args;
use pixel_skies::camera::{direction, Camera};
use pixel_skies::composite::over;
use pixel_skies::sky::{render, sky_io};
use pixel_skies::trees::forest::{self, Forest};
use pixel_skies::trees::silhouette::{self, EYE};
use pixel_skies::vec3::{add, scale, Vec3};

const SKY: &str = "out/sky.bin";

/// Sun at `elevation` degrees, setting at `azimuth` seen from
/// `latitude`. Near the horizon its path is tilted from vertical by the
/// latitude, so for every degree it sinks it slides tan(latitude)
/// degrees towards the pole.
fn sun(azimuth: f64, latitude: f64, elevation: f64) -> Vec3 {
    let slide = elevation * latitude.to_radians().tan();
    direction((azimuth - slide).to_radians(), elevation.to_radians())
}

fn main() {
    let args = Args::parse(env::args().skip(1));
    let sky = sky_io::decode(&fs::read(args.get("sky", SKY.to_string())).unwrap());
    let azimuth: f64 = args.get("azimuth", 270.0);
    let latitude: f64 = args.get("latitude", 60.0);
    let (from, to): (f64, f64) = (args.get("from", 8.0), args.get("to", -4.0));
    let yaw = azimuth.to_radians();
    let fov = args.get("fov", 60.0f64).to_radians();
    let pitch = args.get("pitch", 12.0f64).to_radians();
    let camera = Camera::Perspective { yaw, pitch, fov };
    let forest = Forest {
        edge: args.get("distance", 150.0),
        density: args.get("density", 0.06),
        seed: args.get("seed", 1),
    };
    // Planted for the centre view: a few meters of travel stay well
    // within the margin `plant` leaves around the field of view.
    let planted = forest::plant(&forest, yaw, fov);
    let inside = |p| forest::floor(&forest, yaw, p);
    let travel: f64 = args.get("travel", 8.0);
    let right = direction(yaw + FRAC_PI_2, 0.0);
    let frames = args.get("frames", 300);
    let (width, height) = (args.get("width", 1280), args.get("height", 720));
    let mut out = io::stdout().lock();
    for i in 0..frames {
        let start = Instant::now();
        let t = i as f64 / frames as f64;
        let sun = sun(azimuth, latitude, from + (to - from) * t);
        let eye = add(EYE, scale(right, (t - 0.5) * travel));
        let background = render::render(&sky, sun, &camera, width, height, 0.0);
        let trees = silhouette::render(&planted, inside, &camera, eye, width, height);
        out.write_all(&over(&trees, &background)).unwrap();
        eprintln!("frame {}/{frames} in {:.1?}", i + 1, start.elapsed());
    }
}
