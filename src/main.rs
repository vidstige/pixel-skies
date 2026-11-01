use std::path::Path;
use std::time::Instant;
use std::{env, fs};

use pixel_skies::args::Args;
use pixel_skies::camera::{direction, Camera};
use pixel_skies::png;
use pixel_skies::sky::atmosphere::Params;
use pixel_skies::sky::{render, sky_io, Sky};
use pixel_skies::trees::forest::{self, Forest};
use pixel_skies::trees::silhouette;

const SKY: &str = "out/sky.bin";
const IMAGE: &str = "out/sky.png";
const TREES: &str = "out/trees.png";
const USAGE: &str = "\
usage:
  pixel-skies bake [--aerosol 0.05] [--humidity 0.5] [--ozone 300]
                   [--albedo 0.2] [--out out/sky.bin]
  pixel-skies render [--sky out/sky.bin] [--elevation 10] [--azimuth 180]
                     [--camera perspective|fisheye] [--yaw <azimuth>]
                     [--pitch 12] [--fov 80] [--width 1280] [--height 720]
                     [--exposure 0] [--out out/sky.png]
  pixel-skies trees [--seed 1] [--distance 150] [--density 0.06]
                    [--yaw 180] [--pitch 12] [--fov 80]
                    [--width 1280] [--height 720] [--out out/trees.png]";

fn write(path: &str, bytes: &[u8]) {
    let parent = Path::new(path).parent().unwrap();
    fs::create_dir_all(parent).unwrap();
    fs::write(path, bytes).unwrap();
}

fn bake(args: &Args) {
    let params = Params {
        aerosol: args.get("aerosol", 0.05),
        humidity: args.get("humidity", 0.5),
        ozone: args.get("ozone", 300.0),
        albedo: args.get("albedo", 0.2),
    };
    let start = Instant::now();
    let sky = Sky::bake(params);
    let path = args.get("out", SKY.to_string());
    write(&path, &sky_io::encode(&sky));
    eprintln!("baked {path} in {:.1?}", start.elapsed());
}

fn render(args: &Args) {
    let sky = sky_io::decode(&fs::read(args.get("sky", SKY.to_string())).unwrap());
    let azimuth: f64 = args.get("azimuth", 180.0);
    let sun = direction(azimuth.to_radians(), args.get("elevation", 10.0f64).to_radians());
    let camera = match args.get("camera", "perspective".to_string()).as_str() {
        "fisheye" => Camera::Fisheye,
        _ => Camera::Perspective {
            yaw: args.get("yaw", azimuth).to_radians(),
            pitch: args.get("pitch", 12.0f64).to_radians(),
            fov: args.get("fov", 80.0f64).to_radians(),
        },
    };
    let (width, height) = (args.get("width", 1280), args.get("height", 720));
    let start = Instant::now();
    let pixels = render::render(&sky, sun, &camera, width, height, args.get("exposure", 0.0));
    let path = args.get("out", IMAGE.to_string());
    write(&path, &png::encode(&pixels, width as u32, height as u32));
    eprintln!("rendered {path} in {:.1?}", start.elapsed());
}

fn trees(args: &Args) {
    let yaw = args.get("yaw", 180.0f64).to_radians();
    let pitch = args.get("pitch", 12.0f64).to_radians();
    let fov = args.get("fov", 80.0f64).to_radians();
    let camera = Camera::Perspective { yaw, pitch, fov };
    let forest = Forest {
        edge: args.get("distance", 150.0),
        density: args.get("density", 0.06),
        seed: args.get("seed", 1),
    };
    let (width, height) = (args.get("width", 1280), args.get("height", 720));
    let start = Instant::now();
    let planted = forest::plant(&forest, yaw, fov);
    let inside = |p| forest::floor(&forest, yaw, p);
    let pixels = silhouette::render(&planted, inside, &camera, width, height);
    let path = args.get("out", TREES.to_string());
    write(&path, &png::encode(&pixels, width as u32, height as u32));
    eprintln!("planted {} trees in {path} in {:.1?}", planted.len(), start.elapsed());
}

fn main() {
    let mut words = env::args().skip(1);
    let command = words.next();
    let args = Args::parse(words);
    match command.as_deref() {
        Some("bake") => bake(&args),
        Some("render") => render(&args),
        Some("trees") => trees(&args),
        _ => eprintln!("{USAGE}"),
    }
}
