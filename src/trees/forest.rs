//! Plants a spruce forest on flat ground ahead of the observer. Its edge
//! wavers in and out, trees thin into clearings and grow in clumps of
//! similar height, and edge trees keep their branches to the ground
//! while those inside lose their lower ones to shade.

use crate::camera::direction;
use crate::trees::rng::{hash, Rng};
use crate::trees::spruce::Spruce;
use crate::vec3::{add, dot, scale, Vec3};

pub struct Forest {
    /// Meters ahead to the forest edge, on average.
    pub edge: f64,
    /// Trees per square meter.
    pub density: f64,
    pub seed: u64,
}

/// Meters into the forest that get trees; further in, only the floor
/// shows between the trunks.
const DEPTH: f64 = 300.0;
const MEAN_HEIGHT: f64 = 22.0;
/// Fraction of young trees in the understory, and of old ones standing
/// above the canopy.
const SAPLINGS: f64 = 0.25;
const VETERANS: f64 = 0.03;
/// Meters across a clump, a clearing and a bay of the edge.
const CLUMP: f64 = 40.0;
const CLEARING: f64 = 150.0;
const BAY: f64 = 120.0;
/// How far the edge wavers, as a fraction of its distance.
const WAVER: f64 = 0.3;
/// Meters inside the edge where trees still have full crowns.
const EDGE_BAND: f64 = 8.0;

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Smooth value noise in [0, 1) with features about a unit apart.
fn noise(seed: u64, x: f64, y: f64) -> f64 {
    let (i, j) = (x.floor() as i64, y.floor() as i64);
    let (u, v) = (smoothstep(x - x.floor()), smoothstep(y - y.floor()));
    let at = |di: i64, dj: i64| hash(seed, (i + di) * 1_000_003 + j + dj);
    let lower = at(0, 0) + (at(1, 0) - at(0, 0)) * u;
    let upper = at(0, 1) + (at(1, 1) - at(0, 1)) * u;
    lower + (upper - lower) * v
}

/// Forward and right along the ground for a view towards `yaw`.
fn axes(yaw: f64) -> (Vec3, Vec3) {
    let forward = direction(yaw, 0.0);
    (forward, [forward[1], -forward[0], 0.0])
}

/// Distance ahead to the forest edge, `lateral` meters to the right.
fn edge(forest: &Forest, lateral: f64) -> f64 {
    let wave = noise(forest.seed, lateral / BAY, 0.0) - 0.5;
    forest.edge * (1.0 + 2.0 * WAVER * wave)
}

/// Whether ground point `p` lies inside the forest, under the trees.
pub fn floor(forest: &Forest, yaw: f64, p: Vec3) -> bool {
    let (forward, right) = axes(yaw);
    dot(p, forward) > edge(forest, dot(p, right))
}

/// A tree at `base`, `ahead` and `lateral` meters along the axes, unless
/// it falls outside the forest or in a clearing.
fn tree(forest: &Forest, rng: &mut Rng, base: Vec3, ahead: f64, lateral: f64) -> Option<Spruce> {
    let inside = ahead - edge(forest, lateral);
    let open = noise(forest.seed + 1, lateral / CLEARING, ahead / CLEARING);
    if inside < 0.0 || rng.next() > smoothstep((1.6 * open).min(1.0)) {
        return None;
    }
    let clump = noise(forest.seed + 2, lateral / CLUMP, ahead / CLUMP);
    let age = match rng.next() {
        r if r < SAPLINGS => rng.range(0.15, 0.5),
        r if r < SAPLINGS + VETERANS => rng.range(1.2, 1.5),
        _ => 1.0,
    };
    let height = MEAN_HEIGHT * (0.65 + 0.7 * clump) * age * (1.0 + 0.1 * rng.normal());
    let shaded = (inside / EDGE_BAND).min(1.0);
    let crown_base = shaded * rng.range(0.25, 0.5) + rng.range(0.0, 0.08);
    Some(Spruce { base, height, crown_base, seed: rng.seed() })
}

/// Trees within view of a camera looking towards `yaw` with horizontal
/// field of view `fov`, both in radians.
pub fn plant(forest: &Forest, yaw: f64, fov: f64) -> Vec<Spruce> {
    let (forward, right) = axes(yaw);
    let near = forest.edge * (1.0 - WAVER);
    let far = forest.edge * (1.0 + WAVER) + DEPTH;
    let spread = (fov / 2.0).tan();
    let half = far * spread + MEAN_HEIGHT;
    let n = (forest.density * 2.0 * half * (far - near)) as usize;
    let mut rng = Rng::new(forest.seed);
    let mut trees = Vec::new();
    for _ in 0..n {
        let ahead = rng.range(near, far);
        let lateral = rng.range(-half, half);
        if lateral.abs() > ahead * spread + MEAN_HEIGHT {
            continue;
        }
        let base = add(scale(forward, ahead), scale(right, lateral));
        trees.extend(tree(forest, &mut rng, base, ahead, lateral));
    }
    trees
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forest() -> Forest {
        Forest { edge: 100.0, density: 0.02, seed: 5 }
    }

    #[test]
    fn every_tree_stands_on_the_forest_floor() {
        let yaw = 1.0;
        let trees = plant(&forest(), yaw, 1.2);
        assert!(trees.len() > 100);
        assert!(trees.iter().all(|t| floor(&forest(), yaw, t.base)));
    }

    #[test]
    fn only_trees_near_the_edge_keep_low_branches() {
        let (forward, right) = axes(0.0);
        for t in plant(&forest(), 0.0, 1.2) {
            let inside = dot(t.base, forward) - edge(&forest(), dot(t.base, right));
            assert!(inside < EDGE_BAND || t.crown_base > 0.25);
        }
    }
}
