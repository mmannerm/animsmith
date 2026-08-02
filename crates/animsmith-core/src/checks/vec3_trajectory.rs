//! Interpolation-aware analysis for Vec3 animation trajectories.
//!
//! This stays private to the raw-track checks. It evaluates the same curve as
//! [`crate::sample`]: STEP keys, LINEAR segments, and glTF CUBICSPLINE Hermite
//! segments with time-scaled tangents. In particular, fixed cubic key values
//! do not imply a fixed trajectory when their tangents create an excursion.

use crate::model::{Interpolation, Track, TrackValues};
use glam::Vec3;

/// Summary of a finite, structurally usable Vec3 trajectory.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Vec3Trajectory {
    min: Vec3,
    max: Vec3,
    max_pairwise_spread: f32,
    max_pairwise_time: f32,
}

impl Vec3Trajectory {
    /// Whether any component varies by more than `tolerance`.
    pub(crate) fn varies(self, tolerance: f32) -> bool {
        (self.max - self.min).max_element() > tolerance
    }

    /// Whether any two scale components differ by more than `tolerance`.
    pub(crate) fn is_non_uniform(self, tolerance: f32) -> bool {
        self.max_pairwise_spread > tolerance
    }

    /// Largest departure from unit scale anywhere on the trajectory.
    pub(crate) fn max_unit_deviation(self) -> f32 {
        (self.min - Vec3::ONE)
            .abs()
            .max((self.max - Vec3::ONE).abs())
            .max_element()
    }

    /// Time of the largest component spread, suitable for a finding.
    pub(crate) fn max_pairwise_time(self) -> f32 {
        self.max_pairwise_time
    }
}

/// Analyze a Vec3 track without sampling a fixed-rate grid.
///
/// `None` means the hand-built track is malformed or non-finite. The `nan`
/// and `time-monotonic` checks own that invalid source data; this helper must
/// neither panic nor make a scale classification from it.
pub(crate) fn analyze(track: &Track) -> Option<Vec3Trajectory> {
    let TrackValues::Vec3s(values) = &track.values else {
        return None;
    };
    let key_count = track.key_count();
    if key_count == 0
        || track.times.iter().any(|time| !time.is_finite())
        || track.times.windows(2).any(|pair| pair[1] <= pair[0])
    {
        return None;
    }

    let key = |index: usize| values.get(track.value_index(index)).copied();
    let first = key(0)?;
    if !first.is_finite() {
        return None;
    }
    let mut accumulator = Accumulator::new(first, track.times[0]);

    match track.interpolation {
        Interpolation::Step => {
            for index in 1..key_count {
                accumulator.add(key(index)?, track.times[index])?;
            }
        }
        Interpolation::Linear => {
            for index in 1..key_count {
                let time = track.times[index];
                accumulator.add(key(index)?, time)?;
            }
        }
        Interpolation::CubicSpline => {
            for index in 1..key_count {
                let t0 = track.times[index - 1];
                let t1 = track.times[index];
                let dt = t1 - t0;
                let p0 = key(index - 1)?;
                let p1 = key(index)?;
                // glTF stores tangents per second. Match sample_vec3 by
                // scaling them by this segment's duration before Hermite.
                let m0 = values.get(3 * (index - 1) + 2).copied()? * dt;
                let m1 = values.get(3 * index).copied()? * dt;
                if !p0.is_finite() || !p1.is_finite() || !m0.is_finite() || !m1.is_finite() {
                    return None;
                }
                let segment = CubicSegment::new(p0, m0, p1, m1);
                accumulator.add(p1, t1)?;

                // Component extrema determine temporal range. Pairwise
                // difference extrema determine non-uniform scale, including
                // a cubic-only interior excursion.
                for component in 0..3 {
                    for root in segment.derivative_roots_component(component) {
                        accumulator.add(segment.value(root), t0 + dt * root)?;
                    }
                }
                for (left, right) in [(0, 1), (0, 2), (1, 2)] {
                    for root in segment.derivative_roots_difference(left, right) {
                        accumulator.add(segment.value(root), t0 + dt * root)?;
                    }
                }
            }
        }
    }

    Some(accumulator.finish())
}

#[derive(Debug, Clone, Copy)]
struct Accumulator {
    min: Vec3,
    max: Vec3,
    max_pairwise_spread: f32,
    max_pairwise_time: f32,
}

impl Accumulator {
    fn new(value: Vec3, time: f32) -> Self {
        let spread = pairwise_spread(value);
        Self {
            min: value,
            max: value,
            max_pairwise_spread: spread,
            max_pairwise_time: time,
        }
    }

    fn add(&mut self, value: Vec3, time: f32) -> Option<()> {
        if !value.is_finite() || !time.is_finite() {
            return None;
        }
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        let spread = pairwise_spread(value);
        if spread > self.max_pairwise_spread {
            self.max_pairwise_spread = spread;
            self.max_pairwise_time = time;
        }
        Some(())
    }

    fn finish(self) -> Vec3Trajectory {
        Vec3Trajectory {
            min: self.min,
            max: self.max,
            max_pairwise_spread: self.max_pairwise_spread,
            max_pairwise_time: self.max_pairwise_time,
        }
    }
}

fn pairwise_spread(value: Vec3) -> f32 {
    (value.x - value.y)
        .abs()
        .max((value.x - value.z).abs())
        .max((value.y - value.z).abs())
}

/// One cubic Hermite Vec3 segment, represented as `a*u^3 + b*u^2 + c*u + d`.
#[derive(Debug, Clone, Copy)]
struct CubicSegment {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    d: Vec3,
}

impl CubicSegment {
    fn new(p0: Vec3, m0: Vec3, p1: Vec3, m1: Vec3) -> Self {
        Self {
            a: 2.0 * p0 + m0 - 2.0 * p1 + m1,
            b: -3.0 * p0 - 2.0 * m0 + 3.0 * p1 - m1,
            c: m0,
            d: p0,
        }
    }

    fn value(self, u: f32) -> Vec3 {
        ((self.a * u + self.b) * u + self.c) * u + self.d
    }

    fn component(value: Vec3, index: usize) -> f64 {
        match index {
            0 => value.x as f64,
            1 => value.y as f64,
            _ => value.z as f64,
        }
    }

    /// Interior roots of one component's derivative.
    fn derivative_roots_component(self, index: usize) -> Vec<f32> {
        let a = 3.0 * Self::component(self.a, index);
        let b = 2.0 * Self::component(self.b, index);
        let c = Self::component(self.c, index);
        quadratic_roots(a, b, c)
    }

    /// Interior roots of the derivative of a component difference.
    fn derivative_roots_difference(self, left: usize, right: usize) -> Vec<f32> {
        let difference = |value: Vec3| Self::component(value, left) - Self::component(value, right);
        let a = 3.0 * difference(self.a);
        let b = 2.0 * difference(self.b);
        let c = difference(self.c);
        quadratic_roots(a, b, c)
    }
}

/// Solve `a*u² + b*u + c = 0`, retaining only finite roots inside `(0, 1)`.
fn quadratic_roots(a: f64, b: f64, c: f64) -> Vec<f32> {
    const COEFFICIENT_EPSILON: f64 = 1e-12;
    let mut roots: Vec<f32> = Vec::with_capacity(2);
    let mut push = |root: f64| {
        if root.is_finite() && root > 0.0 && root < 1.0 {
            let root = root as f32;
            if !roots.iter().any(|other| (*other - root).abs() <= 1e-6) {
                roots.push(root);
            }
        }
    };
    if a.abs() <= COEFFICIENT_EPSILON {
        if b.abs() > COEFFICIENT_EPSILON {
            push(-c / b);
        }
        return roots;
    }

    let discriminant = b.mul_add(b, -4.0 * a * c);
    if discriminant < 0.0 {
        return roots;
    }
    let square_root = discriminant.sqrt();
    // Avoid cancellation for one root, then recover the other through the
    // product of roots. The direct fallback covers a zero `q` (double root).
    let q = -0.5 * (b + b.signum() * square_root);
    if q.abs() > COEFFICIENT_EPSILON {
        push(q / a);
        push(c / q);
    } else {
        push((-b - square_root) / (2.0 * a));
        push((-b + square_root) / (2.0 * a));
    }
    roots
}
