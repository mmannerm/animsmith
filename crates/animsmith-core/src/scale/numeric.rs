//! Policy-neutral matrix rewrites and arithmetic provenance primitives.
//!
//! These leaves describe arithmetic that is shared by the reference writer
//! and independent proof without carrying a scale operation, expected value,
//! tolerance decision, connector product, or proof result.

use glam::{Mat4, Vec4};

/// `B' = U B U^-1` for a uniform `U = scale(q)`: the translation column
/// scales by `q`; the linear part is unchanged.
pub(in crate::scale) fn scale_translation_only(matrix: Mat4, q: f32) -> Mat4 {
    let mut matrix = matrix;
    matrix.w_axis.x *= q;
    matrix.w_axis.y *= q;
    matrix.w_axis.z *= q;
    matrix
}

/// `scale(k) * M`: every output row (x/y/z, not the homogeneous row) scales
/// by `k`, which is what left-multiplying by a uniform scale does to both
/// the linear part and the translation column.
pub(in crate::scale) fn scale_rows(matrix: Mat4, k: f32) -> Mat4 {
    let scale_column = |c: Vec4| Vec4::new(c.x * k, c.y * k, c.z * k, c.w);
    Mat4::from_cols(
        scale_column(matrix.x_axis),
        scale_column(matrix.y_axis),
        scale_column(matrix.z_axis),
        scale_column(matrix.w_axis),
    )
}

/// The largest absolute component difference between two matrices.
pub(in crate::scale) fn matrix_residual(before: Mat4, after: Mat4) -> f64 {
    before
        .to_cols_array()
        .into_iter()
        .zip(after.to_cols_array())
        .map(|(b, a)| (b as f64 - a as f64).abs())
        .fold(0.0, f64::max)
}

/// The largest magnitude any entry of `a * b` is summed from:
/// `max over (i, j) of sum over k of abs(a_ik) * abs(b_kj)`.
///
/// This is the magnitude an `f32` `a * b` rounds against, and it is what
/// [`super::ScaleTolerancePolicy::f32_rounding_ulps`] is counted in for the
/// obligations that compare such a product. [`matrix_magnitude`] of the
/// product itself is not: a rotation makes `W * B` near-identity — max entry
/// `1.0` — while its translation column was the difference of two entries of
/// magnitude `abs(W)`, and the error that cancellation leaves behind is
/// `abs(W)`'s ulp, not `1.0`'s.
///
/// Nor is `matrix_magnitude(a) * matrix_magnitude(b)`, which is the same
/// quantity with the sum over `k` replaced by a product of two independent
/// maxima. On a `W * B` whose largest entries are both in the translation
/// column that overstates by the ratio between them: on a rotating rig at
/// factor `3190` it reads `7.6e6` where the arithmetic ran on `6.4e3`, and a
/// tolerance derived from it would accept a matrix that is entirely wrong.
pub(in crate::scale) fn product_operand_magnitude(a: Mat4, b: Mat4) -> f64 {
    // `abs(a) * abs(b)` *is* the matrix of those sums, so one matrix
    // multiply computes all sixteen of them. This runs once per skin slot
    // per document side per sample time — the same order as the `W * B`
    // composition it describes — so the fast path stays in `f32` lane
    // operations rather than becoming a scalar `f64` fold over sixteen
    // entries.
    //
    // Every term of every sum is non-negative, so no entry can cancel and a
    // finite maximum means no entry overflowed: the `f32` result is exact
    // enough to use whenever it is finite.
    let lanes = largest_entry(mat4_abs(a) * mat4_abs(b));
    if lanes.is_finite() {
        return lanes;
    }
    product_operand_magnitude_f64(a, b)
}

/// [`product_operand_magnitude`] recomputed as a scalar `f64` fold, for the
/// operands whose `f32` sums overflow.
///
/// These sums are the *operands'* magnitudes, not the product's, so they run
/// past `f32::MAX` while `a * b` is still finite — the cancellation that
/// makes `W * B` near-identity is exactly what removes the magnitude from the
/// result. Sweeping 2_000_000 random rig-shaped `W` / `B` pairs found 87 such
/// pairs, the smallest with an operand entry of `7.04e37`. Without this
/// fallback each one made [`super::SkinSlot::rounding_magnitude`] infinite,
/// which makes the tolerance derived from it infinite, which
/// [`super::check_residual`] refuses — a *correct* candidate rejected with
/// `tolerance: inf`. `SkinMatrix` reaches it from the joint transforms
/// alone, with no unusual geometry involved.
///
/// The fold cannot overflow in turn, for any `a` and `b` this proof can
/// reach. Each term is a product of two `f32` magnitudes, at most
/// `f32::MAX^2 = 1.16e77`, and each sum has four of them: `4.63e77`, a
/// hundred and fifty decades below `f64::MAX`. So the case is removed rather
/// than moved, and it needs no domain caveat of its own.
#[cold]
#[inline(never)]
fn product_operand_magnitude_f64(a: Mat4, b: Mat4) -> f64 {
    let mut largest = 0.0f64;
    for column in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0f64;
            for inner in 0..4 {
                sum += f64::from(a.col(inner)[row].abs()) * f64::from(b.col(column)[inner].abs());
            }
            largest = largest.max(sum);
        }
    }
    largest
}

/// The rounding base of the translation column in `parent_world * local`.
///
/// For each spatial row, `s` is the binary64 absolute sum of the three new
/// linear/local-translation products and `p` is the absolute carried parent
/// translation. The row contributes
/// `s + min(max(p, MIN_POSITIVE), s / EPSILON)`: `EPSILON * s` provisions the
/// local dot product, while the capped second term provisions the smaller of
/// one parent-scale ulp and losing the whole new contribution. The minimum
/// normal floor also covers subnormal product rounding. Zero links still
/// contribute zero, and underflowed links carry only their vanishing `s`
/// rather than charging the translated parent again.
///
/// [`product_operand_magnitude`] for one column, and it exists for the same
/// reason one column further up the chain. That function reads the *already
/// composed* `W`, whose translation column has already lost whatever its own
/// parent chain cancelled: a joint whose local offset points back along its
/// parent's world translation leaves `W` with a small translation that was
/// summed from two large terms, and `abs(W) * abs(B)` cannot see terms that
/// are no longer in `W`. Composing `W * B` then carries that lost magnitude's
/// rounding error into a near-identity product, and a tolerance derived from
/// `abs(W) * abs(B)` alone refuses the correct candidate — measured at up to
/// `41` binary32 ulps of that base over a million correct candidates, against
/// `2.4` once this term is included.
///
/// The homogeneous output row is deliberately excluded because it is not a
/// spatial translation component. For validated affine operands its linear
/// entries are zero, so the shipped `contribution / EPSILON` cap would already
/// make its contribution zero. Keeping the spatial range explicit preserves
/// the quantity's unit semantics and prevents a non-affine bottom row from
/// entering provenance if upstream validation regresses.
///
/// Only the translation column needs it. A world linear part is a product of
/// rotations and uniform scales, and while an individual entry of that
/// product can cancel to near zero, `product_operand_magnitude` already sums
/// over the inner index — so the terms that cancelled are still in its sum.
/// The translation column is the one place a *previous* composition's
/// cancellation is carried forward as an operand.
pub(in crate::scale) fn translation_composition_rounding_base(
    parent_world: Mat4,
    local: Mat4,
) -> f64 {
    let epsilon = f64::from(f32::EPSILON);
    let minimum_normal = f64::from(f32::MIN_POSITIVE);
    let mut largest = 0.0f64;
    for row in 0..3 {
        let mut contribution = 0.0f64;
        for inner in 0..3 {
            contribution += f64::from(parent_world.col(inner)[row].abs())
                * f64::from(local.w_axis[inner].abs());
        }
        let parent = f64::from(parent_world.w_axis[row].abs());
        let addition = parent.max(minimum_normal).min(contribution / epsilon);
        largest = largest.max(contribution + addition);
    }
    largest
}

/// The magnitude `matrix * column` is summed from:
/// `max over i of sum over k of abs(matrix_ik) * abs(column_k)`.
///
/// [`product_operand_magnitude`] for one column, and the quantity the bounds
/// path needs when a composed `W * B` transforms a vertex position.
///
/// `absolute` is [`mat4_abs`] of the matrix, taken by the caller rather than
/// here. The skinned-bounds caller runs this once per weighted vertex per slot
/// — the hottest loop in this proof — against a matrix that is constant across
/// the whole primitive, so recomputing sixteen `abs` per vertex would be work
/// the slot already did once.
pub(in crate::scale) fn column_operand_magnitude(absolute: Mat4, column: Vec4) -> f64 {
    // Every term is non-negative, so a finite maximum means no term
    // overflowed and the `f32` lane result is exact enough to use — the same
    // argument [`product_operand_magnitude`] makes, and it needs the same
    // fallback for the operands whose sums leave the `f32` range while the
    // transformed result stays inside it.
    let lanes = f64::from((absolute * column.abs()).max_element());
    if lanes.is_finite() {
        return lanes;
    }
    column_operand_magnitude_f64(absolute, column)
}

/// [`column_operand_magnitude`] recomputed as a scalar `f64` fold, for the
/// operands whose `f32` sums overflow.
///
/// Cannot overflow in turn, for [`product_operand_magnitude_f64`]'s reason:
/// four terms, each a product of two `f32` magnitudes, is at most `4.63e77`.
#[cold]
#[inline(never)]
fn column_operand_magnitude_f64(absolute: Mat4, column: Vec4) -> f64 {
    let column = column.abs();
    let mut largest = 0.0f64;
    for row in 0..4 {
        let mut sum = 0.0f64;
        for inner in 0..4 {
            sum += f64::from(absolute.col(inner)[row]) * f64::from(column[inner]);
        }
        largest = largest.max(sum);
    }
    largest
}

/// `abs` applied to every component.
pub(in crate::scale) fn mat4_abs(matrix: Mat4) -> Mat4 {
    Mat4::from_cols(
        matrix.x_axis.abs(),
        matrix.y_axis.abs(),
        matrix.z_axis.abs(),
        matrix.w_axis.abs(),
    )
}

/// The largest entry of an already non-negative matrix.
pub(in crate::scale) fn largest_entry(nonnegative: Mat4) -> f64 {
    f64::from(
        nonnegative
            .x_axis
            .max(nonnegative.y_axis)
            .max(nonnegative.z_axis.max(nonnegative.w_axis))
            .max_element(),
    )
}

/// The largest absolute component of a matrix.
pub(in crate::scale) fn matrix_magnitude(matrix: Mat4) -> f64 {
    largest_entry(mat4_abs(matrix))
}
