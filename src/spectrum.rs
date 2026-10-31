//! Spectra sampled every 10 nm over 360–830 nm, and their conversion to
//! CIE XYZ.

use std::sync::LazyLock;

pub const N: usize = 48;
const LAMBDA_MIN: f64 = 360.0;
const STEP: f64 = 10.0;
/// Lumens per watt at 555 nm, so Y comes out in cd/m² for radiance in
/// W/(m² sr nm).
const LUMINOUS_EFFICACY: f64 = 683.0;

pub type Spectrum = [f64; N];
pub type Xyz = [f64; 3];

/// Solar irradiance at the top of the atmosphere in W/(m² nm), averaged
/// per bin. Same data as Bruneton's precomputed scattering demo.
#[rustfmt::skip]
pub const SOLAR_IRRADIANCE: Spectrum = [
    1.11776, 1.14259, 1.01249, 1.14716, 1.72765, 1.73054, 1.6887, 1.61253,
    1.91198, 2.03474, 2.02042, 2.02212, 1.93377, 1.95809, 1.91686, 1.8298,
    1.8685, 1.8931, 1.85149, 1.8504, 1.8341, 1.8345, 1.8147, 1.78158, 1.7533,
    1.6965, 1.68194, 1.64654, 1.6048, 1.52143, 1.55622, 1.5113, 1.474, 1.4482,
    1.41018, 1.36775, 1.34188, 1.31429, 1.28303, 1.26758, 1.2367, 1.2082,
    1.18737, 1.14683, 1.12362, 1.1058, 1.07124, 1.04992,
];

/// Ozone absorption cross section in m² at 233 K (IUP Bremen), averaged
/// per bin.
#[rustfmt::skip]
pub const OZONE_CROSS_SECTION: Spectrum = [
    1.18e-27, 2.182e-28, 2.818e-28, 6.636e-28, 1.527e-27, 2.763e-27, 5.52e-27,
    8.451e-27, 1.582e-26, 2.316e-26, 3.669e-26, 4.924e-26, 7.752e-26, 9.016e-26,
    1.48e-25, 1.602e-25, 2.139e-25, 2.755e-25, 3.091e-25, 3.5e-25, 4.266e-25,
    4.672e-25, 4.398e-25, 4.701e-25, 5.019e-25, 4.305e-25, 3.74e-25, 3.215e-25,
    2.662e-25, 2.238e-25, 1.852e-25, 1.473e-25, 1.209e-25, 9.423e-26, 7.455e-26,
    6.566e-26, 5.105e-26, 4.15e-26, 4.228e-26, 3.237e-26, 2.451e-26, 2.801e-26,
    2.534e-26, 1.624e-26, 1.465e-26, 2.078e-26, 1.383e-26, 7.105e-27,
];

/// Wavelength of bin `i` in nm.
pub fn wavelength(i: usize) -> f64 {
    LAMBDA_MIN + STEP * i as f64
}

pub fn from_fn(f: impl Fn(f64) -> f64) -> Spectrum {
    std::array::from_fn(|i| f(wavelength(i)))
}

/// Gaussian with different widths either side of its peak.
fn lobe(x: f64, mu: f64, left: f64, right: f64) -> f64 {
    let s = if x < mu { left } else { right };
    (-0.5 * ((x - mu) / s).powi(2)).exp()
}

/// CIE 1931 colour matching functions, in the multi-lobe fit of Wyman,
/// Sloan and Shirley (2013).
fn cmf(l: f64) -> Xyz {
    [
        1.056 * lobe(l, 599.8, 37.9, 31.0) + 0.362 * lobe(l, 442.0, 16.0, 26.7)
            - 0.065 * lobe(l, 501.1, 20.4, 26.2),
        0.821 * lobe(l, 568.8, 46.9, 40.5) + 0.286 * lobe(l, 530.9, 16.3, 31.1),
        1.217 * lobe(l, 437.0, 11.8, 36.0) + 0.681 * lobe(l, 459.0, 26.0, 13.8),
    ]
}

static WEIGHTS: LazyLock<[Xyz; N]> = LazyLock::new(|| {
    std::array::from_fn(|i| cmf(wavelength(i)).map(|c| c * STEP * LUMINOUS_EFFICACY))
});

pub fn to_xyz(s: &Spectrum) -> Xyz {
    let weighted = |k: usize| (0..N).map(|i| s[i] * WEIGHTS[i][k]).sum();
    std::array::from_fn(weighted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chromaticity(xyz: Xyz) -> (f64, f64) {
        let sum: f64 = xyz.iter().sum();
        (xyz[0] / sum, xyz[1] / sum)
    }

    #[test]
    fn luminosity_integrates_to_the_cie_value() {
        let y: f64 = (0..N).map(|i| cmf(wavelength(i))[1] * STEP).sum();
        assert!((y - 106.86).abs() < 1.0, "{y}");
    }

    #[test]
    fn equal_energy_spectrum_is_achromatic() {
        let (x, y) = chromaticity(to_xyz(&[1.0; N]));
        assert!((x - 1.0 / 3.0).abs() < 0.01 && (y - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn sunlight_above_the_atmosphere_is_near_white() {
        let (x, y) = chromaticity(to_xyz(&SOLAR_IRRADIANCE));
        assert!((x - 0.32).abs() < 0.02 && (y - 0.33).abs() < 0.02, "{x} {y}");
    }
}
