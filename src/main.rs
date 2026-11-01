mod args;
mod camera;
mod deflate;
mod parallel;
mod png;
mod sky;
mod vec3;

use std::path::Path;
use std::time::Instant;
use std::{env, fs};

use args::Args;
use camera::{direction, Camera};
use sky::atmosphere::Params;
use sky::{render, sky_io, Sky};

const SKY: &str = "out/sky.bin";
const IMAGE: &str = "out/sky.png";
const USAGE: &str = "\
usage:
  pixel-skies bake [--aerosol 0.05] [--humidity 0.5] [--ozone 300]
                   [--albedo 0.2] [--out out/sky.bin]
  pixel-skies render [--sky out/sky.bin] [--elevation 10] [--azimuth 180]
                     [--camera perspective|fisheye] [--yaw <azimuth>]
                     [--pitch 12] [--fov 80] [--width 1280] [--height 720]
                     [--exposure 0] [--out out/sky.png]";

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

fn main() {
    let mut words = env::args().skip(1);
    let command = words.next();
    let args = Args::parse(words);
    match command.as_deref() {
        Some("bake") => bake(&args),
        Some("render") => render(&args),
        _ => eprintln!("{USAGE}"),
    }
}
