//! Maps pixels to view directions. World axes are x east, y north, z up;
//! azimuth runs clockwise from north.

use std::f64::consts::FRAC_PI_2;

use crate::vec3::{add, cross, dot, length, normalize, scale, Vec3};

pub enum Camera {
    /// Pinhole looking towards `yaw` azimuth and `pitch` elevation, with
    /// `fov` across the image width. Angles in radians.
    Perspective { yaw: f64, pitch: f64, fov: f64 },
    /// Equidistant all-sky view: zenith in the centre, horizon on the
    /// rim, north up and east left as seen lying on your back.
    Fisheye,
}

pub fn direction(azimuth: f64, elevation: f64) -> Vec3 {
    let c = elevation.cos();
    [azimuth.sin() * c, azimuth.cos() * c, elevation.sin()]
}

/// Forward, right and up of a pinhole looking towards `yaw` and `pitch`.
fn basis(yaw: f64, pitch: f64) -> (Vec3, Vec3, Vec3) {
    let forward = direction(yaw, pitch);
    let right = normalize(cross(forward, [0.0, 0.0, 1.0]));
    (forward, right, cross(right, forward))
}

impl Camera {
    /// Direction through image point (x, y), measured in pixels from the
    /// top left corner, or nothing where the projection has no sky.
    pub fn ray(&self, x: f64, y: f64, width: f64, height: f64) -> Option<Vec3> {
        match *self {
            Camera::Perspective { yaw, pitch, fov } => {
                let (forward, right, up) = basis(yaw, pitch);
                let s = (fov / 2.0).tan();
                let u = (2.0 * x / width - 1.0) * s;
                let v = (1.0 - 2.0 * y / height) * s * height / width;
                Some(normalize(add(forward, add(scale(right, u), scale(up, v)))))
            }
            Camera::Fisheye => {
                let radius = width.min(height) / 2.0;
                let dx = (x - width / 2.0) / radius;
                let dy = (y - height / 2.0) / radius;
                let d = dx.hypot(dy);
                (d <= 1.0).then(|| direction((-dx).atan2(-dy), FRAC_PI_2 * (1.0 - d)))
            }
        }
    }

    /// Image point of `v`, seen from the eye, and how many pixels a
    /// meter spans there; nothing behind a pinhole or below a fisheye.
    pub fn project(&self, v: Vec3, width: f64, height: f64) -> Option<[f64; 3]> {
        match *self {
            Camera::Perspective { yaw, pitch, fov } => {
                let (forward, right, up) = basis(yaw, pitch);
                let (z, s) = (dot(v, forward), (fov / 2.0).tan());
                let x = (1.0 + dot(v, right) / z / s) * width / 2.0;
                let y = (1.0 - dot(v, up) / z / s * width / height) * height / 2.0;
                (z > 0.0).then_some([x, y, width / 2.0 / s / z])
            }
            Camera::Fisheye => {
                let radius = width.min(height) / 2.0;
                let d = length(v);
                let r = radius * (v[2] / d).acos() / FRAC_PI_2;
                let azimuth = v[0].atan2(v[1]);
                let (x, y) = (width / 2.0 - r * azimuth.sin(), height / 2.0 - r * azimuth.cos());
                (v[2] >= 0.0).then_some([x, y, radius / FRAC_PI_2 / d])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        dot(a, b) > 1.0 - 1e-9
    }

    #[test]
    fn perspective_centre_looks_along_yaw_and_pitch() {
        let camera = Camera::Perspective { yaw: 1.0, pitch: 0.3, fov: 1.2 };
        let ray = camera.ray(320.0, 240.0, 640.0, 480.0).unwrap();
        assert!(close(ray, direction(1.0, 0.3)));
    }

    #[test]
    fn perspective_image_edges_span_the_field_of_view() {
        let camera = Camera::Perspective { yaw: 0.0, pitch: 0.0, fov: 1.2 };
        let left = camera.ray(0.0, 50.0, 200.0, 100.0).unwrap();
        let right = camera.ray(200.0, 50.0, 200.0, 100.0).unwrap();
        assert!((dot(left, right).acos() - 1.2).abs() < 1e-9);
    }

    fn round_trips(camera: Camera) {
        let (w, h) = (640.0, 480.0);
        for (x, y) in [(250.0, 150.0), (320.0, 240.0), (420.0, 330.0)] {
            let ray = camera.ray(x, y, w, h).unwrap();
            let p = camera.project(scale(ray, 30.0), w, h).unwrap();
            assert!((p[0] - x).abs() < 1e-6 && (p[1] - y).abs() < 1e-6);
        }
    }

    #[test]
    fn projection_inverts_rays() {
        round_trips(Camera::Perspective { yaw: 1.0, pitch: 0.3, fov: 1.2 });
        round_trips(Camera::Fisheye);
    }

    #[test]
    fn fisheye_centre_is_zenith_and_rim_is_horizon() {
        let (w, h) = (100.0, 100.0);
        assert!(close(Camera::Fisheye.ray(50.0, 50.0, w, h).unwrap(), [0.0, 0.0, 1.0]));
        assert!(close(Camera::Fisheye.ray(50.0, 0.0, w, h).unwrap(), [0.0, 1.0, 0.0]));
        assert!(close(Camera::Fisheye.ray(0.0, 50.0, w, h).unwrap(), [1.0, 0.0, 0.0]));
        assert!(Camera::Fisheye.ray(1.0, 1.0, w, h).is_none());
    }
}
