//! Seeded random numbers: SplitMix64 streams and a stateless hash.

pub struct Rng(u64);

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Uniform in [0, 1) from a seed and an integer coordinate.
pub fn hash(seed: u64, i: i64) -> f64 {
    let h = mix(seed.wrapping_mul(0x9e3779b97f4a7c15) ^ i as u64);
    (mix(h) >> 11) as f64 / (1u64 << 53) as f64
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(mix(seed))
    }

    /// Uniform over all 64-bit integers, as seeds for other streams.
    pub fn seed(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        mix(self.0)
    }

    /// Uniform in [0, 1).
    pub fn next(&mut self) -> f64 {
        (self.seed() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.next() < p
    }

    /// Standard normal, by Box-Muller.
    pub fn normal(&mut self) -> f64 {
        let (u, v) = (1.0 - self.next(), self.next());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_mean_and_normal_spread_match_their_distributions() {
        let mut rng = Rng::new(7);
        let n = 100_000;
        let mean = (0..n).map(|_| rng.next()).sum::<f64>() / n as f64;
        let var = (0..n).map(|_| rng.normal().powi(2)).sum::<f64>() / n as f64;
        assert!((mean - 0.5).abs() < 0.01);
        assert!((var - 1.0).abs() < 0.02);
    }

    #[test]
    fn hash_is_deterministic_and_seed_dependent() {
        assert_eq!(hash(1, 42), hash(1, 42));
        assert_ne!(hash(1, 42), hash(2, 42));
    }
}
