//! Procedural Norway spruce as tapered capsules, in meters, world axes.
//!
//! The crown is whorls of branches around a leaning trunk, longest some
//! way down and shortening to a bare leader at the top. Upper branches
//! reach up, lower ones sag and curl their tips up again, and every
//! branch is hung with needled shoots that droop more the lower they
//! sit. Below the live crown only dead stubs remain.

use std::f64::consts::TAU;

use crate::camera::direction;
use crate::trees::rng::Rng;
use crate::vec3::{add, scale, Vec3};

/// Tapered capsule from `a` to `b`, radius `ra` at `a` and `rb` at `b`.
pub struct Limb {
    pub a: Vec3,
    pub b: Vec3,
    pub ra: f64,
    pub rb: f64,
}

pub struct Spruce {
    pub base: Vec3,
    pub height: f64,
    /// Height of the lowest live branches as a fraction of `height`.
    pub crown_base: f64,
    pub seed: u64,
}

/// Meters between whorls low in the crown, and up at the leader.
const WHORL_SPACING: f64 = 0.45;
const LEADER_SPACING: f64 = 0.8;
/// Widest crown radius per meter of height, plus a fixed part that
/// makes saplings stouter.
const SLENDERNESS: (f64, f64) = (0.1, 0.18);
const SAPLING_RADIUS: f64 = 0.4;
/// Meters between side shoots along a branch.
const SHOOT_SPACING: f64 = 0.2;
/// Half-thickness of a needled shoot.
const NEEDLE_RADIUS: f64 = 0.045;
/// Most a crown reaches further to one side, from crowding or wind.
const LOPSIDED: f64 = 0.35;
/// Chances of a whorl lost to frost or browsing, a branch missing or
/// broken short, and a side shoot missing.
const WHORL_GAPS: f64 = 0.1;
const BRANCHES_MISSING: f64 = 0.05;
const BRANCHES_BROKEN: f64 = 0.08;
const SHOOTS_MISSING: f64 = 0.15;
/// Fraction of trees that are dead snags with a broken top.
const SNAGS: f64 = 0.03;
const TRUNK_SEGMENTS: usize = 10;

/// Traits a tree draws once and all its branches share.
struct Habit {
    /// Widest crown radius in meters.
    widest: f64,
    /// How pendulous the shoots hang, and how long they grow.
    droop: f64,
    vigor: f64,
    /// How much further the crown reaches towards azimuth `favored`.
    lopsided: f64,
    favored: f64,
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// A needled shoot hanging `droop` radians, give or take, off a branch.
fn shoot(limbs: &mut Vec<Limb>, rng: &mut Rng, at: Vec3, azimuth: f64, droop: f64, length: f64) {
    let elevation = -droop * rng.range(0.5, 1.3);
    let tip = add(at, scale(direction(azimuth, elevation), length));
    limbs.push(Limb { a: at, b: tip, ra: NEEDLE_RADIUS, rb: 0.6 * NEEDLE_RADIUS });
}

/// A branch from `origin`, where `rel` runs from 0 at the crown top to
/// 1 at its base. Elevations are in degrees: upper branches rise 35°,
/// the lowest sag 30°, and their tips curl up by up to 45°.
fn branch(
    limbs: &mut Vec<Limb>,
    rng: &mut Rng,
    habit: &Habit,
    origin: Vec3,
    azimuth: f64,
    length: f64,
    rel: f64,
) {
    let rise = 35.0 - 65.0 * rel.powf(0.7) + 8.0 * rng.normal();
    let curl = 10.0 + 35.0 * rel;
    let hang = habit.droop * (15.0 + 55.0 * rel).to_radians();
    let fullness = habit.vigor * rng.range(0.5, 1.3);
    let n = (length / SHOOT_SPACING).ceil().max(1.0) as usize;
    let step = length / n as f64;
    let mut p = origin;
    for i in 0..n {
        let t = (i as f64 + 0.5) / n as f64;
        let elevation = (rise + curl * t * t).to_radians();
        let heading = azimuth + 0.15 * rng.normal();
        let q = add(p, scale(direction(heading, elevation), step));
        let r = NEEDLE_RADIUS * lerp(1.6, 1.0, t);
        limbs.push(Limb { a: p, b: q, ra: r, rb: r });
        let reach = fullness * (0.1 + 0.3 * length * (1.0 - t)).min(0.9);
        for side in [-1.0, 1.0] {
            if rng.chance(SHOOTS_MISSING) {
                continue;
            }
            let azimuth = heading + side * rng.range(0.6, 1.3);
            let length = reach * rng.range(0.3, 1.4);
            shoot(limbs, rng, q, azimuth, hang, length);
        }
        p = q;
    }
}

/// Three to six branches around the trunk at `origin`, reaching about
/// `radius`.
fn whorl(
    limbs: &mut Vec<Limb>,
    rng: &mut Rng,
    habit: &Habit,
    origin: Vec3,
    radius: f64,
    rel: f64,
) {
    let n = rng.range(3.0, 7.0) as usize;
    let phase = rng.range(0.0, TAU);
    for k in 0..n {
        if rng.chance(BRANCHES_MISSING) {
            continue;
        }
        let azimuth = phase + (k as f64 + 0.3 * rng.normal()) / n as f64 * TAU;
        let broken = if rng.chance(BRANCHES_BROKEN) { rng.range(0.2, 0.6) } else { 1.0 };
        let side = 1.0 + habit.lopsided * (azimuth - habit.favored).cos();
        let length = radius * side * rng.range(0.75, 1.15) * broken;
        branch(limbs, rng, habit, origin, azimuth, length, rel);
    }
}

/// A dead branch stub without needles.
fn stub(limbs: &mut Vec<Limb>, rng: &mut Rng, origin: Vec3, length: f64) {
    let dir = direction(rng.range(0.0, TAU), rng.range(-0.4, 0.1));
    let tip = add(origin, scale(dir, length));
    limbs.push(Limb { a: origin, b: tip, ra: 0.02, rb: 0.008 });
}

/// Crown radius `depth` meters below the top of a crown `length` long:
/// a spire that flares out to its widest some way down.
fn crown_radius(widest: f64, depth: f64, length: f64) -> f64 {
    widest * (depth / (0.7 * length)).min(1.0).powf(1.2)
}

fn trunk(limbs: &mut Vec<Limb>, axis: impl Fn(f64) -> Vec3, height: f64) {
    let girth = 0.03 + 0.011 * height;
    let radius = |t: f64| girth * (1.0 - t) + 0.8 * NEEDLE_RADIUS;
    for i in 0..TRUNK_SEGMENTS {
        let t0 = i as f64 / TRUNK_SEGMENTS as f64;
        let t1 = (i + 1) as f64 / TRUNK_SEGMENTS as f64;
        let (a, b) = (axis(t0 * height), axis(t1 * height));
        limbs.push(Limb { a, b, ra: radius(t0), rb: radius(t1) });
    }
}

pub fn grow(spruce: &Spruce) -> Vec<Limb> {
    let mut rng = Rng::new(spruce.seed);
    let snag = rng.chance(SNAGS);
    let height = spruce.height * if snag { rng.range(0.4, 0.8) } else { 1.0 };
    let crown_base = if snag { 1.0 } else { spruce.crown_base };
    let lean = [0.015 * rng.normal(), 0.015 * rng.normal()];
    let bow = 0.01 * rng.normal();
    let axis = |h: f64| add(spruce.base, [lean[0] * h + bow * h * h / height, lean[1] * h, h]);
    let habit = Habit {
        widest: rng.range(SLENDERNESS.0, SLENDERNESS.1) * height + SAPLING_RADIUS,
        droop: rng.range(0.5, 1.2),
        vigor: rng.range(0.6, 1.3),
        lopsided: rng.range(0.0, LOPSIDED),
        favored: rng.range(0.0, TAU),
    };
    let mut limbs = Vec::new();
    trunk(&mut limbs, axis, height);

    let crown = height * (1.0 - crown_base);
    let mut depth = rng.range(0.5, 1.2) * LEADER_SPACING;
    while depth < crown {
        let radius = crown_radius(habit.widest, depth, crown);
        whorl(&mut limbs, &mut rng, &habit, axis(height - depth), radius, depth / crown);
        let spacing = lerp(LEADER_SPACING, WHORL_SPACING, (depth / 4.0).min(1.0));
        let gap = if rng.chance(WHORL_GAPS) { 2.0 } else { 1.0 };
        depth += spacing * gap * rng.range(0.7, 1.3);
    }
    while depth < height - 1.5 {
        if rng.chance(0.5) {
            let length = rng.range(0.2, 0.3) * habit.widest;
            stub(&mut limbs, &mut rng, axis(height - depth), length);
        }
        depth += WHORL_SPACING * rng.range(0.7, 1.3);
    }
    // The lowest branches sag onto the ground rather than through it.
    for limb in &mut limbs {
        limb.a[2] = limb.a[2].max(spruce.base[2]);
        limb.b[2] = limb.b[2].max(spruce.base[2]);
    }
    limbs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spruce(seed: u64, crown_base: f64) -> Spruce {
        Spruce { base: [0.0; 3], height: 20.0, crown_base, seed }
    }

    /// Lowest point of needled limbs, telling them from trunk and stubs
    /// by their radius.
    fn lowest_needles(limbs: &[Limb]) -> f64 {
        let needled = |l: &&Limb| l.rb >= 0.6 * NEEDLE_RADIUS && l.ra <= 1.6 * NEEDLE_RADIUS;
        let needled = limbs.iter().filter(needled);
        needled.map(|l| l.a[2].min(l.b[2])).fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn crowns_are_spires_much_taller_than_wide() {
        for seed in 0..20 {
            let limbs = grow(&spruce(seed, 0.0));
            let top = limbs.iter().map(|l| l.b[2]).fold(0.0, f64::max);
            let reach = limbs.iter().map(|l| l.b[0].hypot(l.b[1])).fold(0.0, f64::max);
            assert!(top <= 20.0 && 2.0 * reach < 0.6 * top, "seed {seed}");
        }
    }

    #[test]
    fn shaded_trees_keep_needles_only_up_in_the_crown() {
        for seed in 0..20 {
            let open = lowest_needles(&grow(&spruce(seed, 0.0)));
            let shaded = lowest_needles(&grow(&spruce(seed, 0.5)));
            assert!(open < 1.0 && shaded > 7.0, "seed {seed}: {open} {shaded}");
        }
    }

    #[test]
    fn the_same_seed_grows_the_same_tree() {
        let (a, b) = (grow(&spruce(3, 0.2)), grow(&spruce(3, 0.2)));
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(p, q)| p.a == q.a && p.b == q.b));
    }
}
