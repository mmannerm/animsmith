//! Dense-`f32` accessor read/modify/write over resolved source buffers.
//!
//! Every accessor this module touches was vouched for by #280's preflight:
//! dense `f32`, non-normalized, non-sparse, `stride == element_size`,
//! 4-byte aligned, and byte-disjoint from every other used accessor. The
//! range is nevertheless re-resolved here through the very same
//! [`crate::capability::dense_f32_accessor_range`] the preflight used, so a
//! source that somehow reached the rewriter without that guarantee fails
//! closed instead of writing into an unrelated byte.

use super::GltfScaleRewriteError;
use super::rules::{AccessorRule, components_per_element};
use crate::capability::dense_f32_accessor_range;
use serde_json::{Map, Value};

/// One accessor's resolved byte span inside one resolved source buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AccessorSpan {
    /// Source accessor index.
    pub(crate) accessor_index: usize,
    /// Resolved buffer index.
    pub(crate) buffer: usize,
    /// Inclusive start byte offset into that buffer.
    pub(crate) start: usize,
    /// Exclusive end byte offset into that buffer.
    pub(crate) end: usize,
    /// Components per element, from the accessor's `type`.
    pub(crate) components: usize,
}

impl AccessorSpan {
    /// Number of `f32` values in the span.
    pub(crate) fn float_count(self) -> usize {
        (self.end - self.start) / 4
    }
}

/// Resolve the dense `f32` byte span of `accessor_index`, checking that the
/// span really is a whole number of elements of the rule's required type.
pub(crate) fn accessor_span(
    root: &Map<String, Value>,
    buffers: &[Vec<u8>],
    accessor_index: usize,
    rule: AccessorRule,
) -> Result<AccessorSpan, GltfScaleRewriteError> {
    let location = format!("/accessors/{accessor_index}");
    let unsupported = || GltfScaleRewriteError::UnrewritableAccessor {
        accessor_index,
        location: location.clone(),
    };
    let accessor = root
        .get("accessors")
        .and_then(Value::as_array)
        .and_then(|accessors| accessors.get(accessor_index))
        .and_then(Value::as_object)
        .ok_or_else(unsupported)?;
    let accessor_type = accessor
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(unsupported)?;
    if rule
        .required_accessor_type()
        .is_some_and(|required| required != accessor_type)
    {
        return Err(unsupported());
    }
    let components = components_per_element(accessor_type).ok_or_else(unsupported)?;
    let count: usize = accessor
        .get("count")
        .and_then(Value::as_u64)
        .and_then(|count| count.try_into().ok())
        .ok_or_else(unsupported)?;
    let (buffer, start, end) =
        dense_f32_accessor_range(root, buffers, accessor_index).ok_or_else(unsupported)?;
    // A dense f32 accessor stores exactly `count * components` floats with no
    // matrix column padding, so a span that disagrees means the range and the
    // declared shape describe different data. Refuse rather than stride over
    // whatever is actually there.
    if end.checked_sub(start) != count.checked_mul(components).and_then(|f| f.checked_mul(4)) {
        return Err(unsupported());
    }
    Ok(AccessorSpan {
        accessor_index,
        buffer,
        start,
        end,
        components,
    })
}

/// Per-component extrema observed in one rewritten accessor's payload.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ComponentExtrema {
    /// Smallest value seen for each element component.
    pub(crate) min: Vec<f32>,
    /// Largest value seen for each element component.
    pub(crate) max: Vec<f32>,
}

/// Multiply every component `rule` selects by `factor`, in place, and report
/// the per-component extrema of the rewritten payload.
///
/// Every element is computed in `f64` and narrowed exactly once. A result
/// that is not finite, or that flushes a nonzero product to zero, is a
/// located rejection: [`animsmith_core::scale::ScaleError::FactorNotRepresentable`]
/// guards the *factor*, never `coordinate * factor`, and the raw route has no
/// `validate_document_shape` net underneath it to catch an annihilated or
/// infinite coordinate afterwards.
pub(crate) fn scale_span(
    buffers: &mut [Vec<u8>],
    span: AccessorSpan,
    rule: AccessorRule,
    factor: f64,
) -> Result<ComponentExtrema, GltfScaleRewriteError> {
    let buffer = buffers
        .get_mut(span.buffer)
        .filter(|buffer| span.end <= buffer.len())
        .ok_or_else(|| GltfScaleRewriteError::UnrewritableAccessor {
            accessor_index: span.accessor_index,
            location: format!("/accessors/{}", span.accessor_index),
        })?;
    let mut extrema = ComponentExtrema {
        min: vec![f32::INFINITY; span.components],
        max: vec![f32::NEG_INFINITY; span.components],
    };
    for float_index in 0..span.float_count() {
        let offset = span.start + float_index * 4;
        let bytes: [u8; 4] = buffer[offset..offset + 4]
            .try_into()
            .expect("slice has four bytes");
        let before = f32::from_le_bytes(bytes);
        let component = float_index % span.components;
        let after = if rule.scales_component(component) {
            narrow(
                f64::from(before) * factor,
                &format!("/accessors/{}[{float_index}]", span.accessor_index),
            )?
        } else {
            before
        };
        buffer[offset..offset + 4].copy_from_slice(&after.to_le_bytes());
        extrema.min[component] = extrema.min[component].min(after);
        extrema.max[component] = extrema.max[component].max(after);
    }
    Ok(extrema)
}

/// Narrow one `f64` product to the `f32` the glTF model stores, rejecting a
/// non-finite result and a nonzero product that flushed to zero.
pub(crate) fn narrow(value: f64, location: &str) -> Result<f32, GltfScaleRewriteError> {
    let narrowed = value as f32;
    if !value.is_finite() || !narrowed.is_finite() || (narrowed == 0.0 && value != 0.0) {
        return Err(GltfScaleRewriteError::ValueNotRepresentable {
            location: location.to_owned(),
            value,
        });
    }
    Ok(narrowed)
}

/// Read every `f32` of a span out of `buffers`.
pub(crate) fn read_span(buffers: &[Vec<u8>], span: AccessorSpan) -> Vec<f32> {
    let buffer = &buffers[span.buffer];
    (0..span.float_count())
        .map(|index| {
            let offset = span.start + index * 4;
            f32::from_le_bytes(
                buffer[offset..offset + 4]
                    .try_into()
                    .expect("slice has four bytes"),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn floats(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn vec3_root(count: usize) -> Value {
        json!({
            "bufferViews": [{ "buffer": 0, "byteLength": count * 12 }],
            "accessors": [{ "bufferView": 0, "componentType": 5126, "count": count, "type": "VEC3" }]
        })
    }

    #[test]
    fn scaling_by_one_is_byte_identity() {
        let root = vec3_root(2);
        let root = root.as_object().expect("object");
        let original = floats(&[1.5, -2.25, 0.0, 3.5, 1e-30, -0.0]);
        let mut buffers = vec![original.clone()];
        let span = accessor_span(root, &buffers, 0, AccessorRule::AllComponents).expect("span");
        scale_span(&mut buffers, span, AccessorRule::AllComponents, 1.0).expect("identity rewrite");
        assert_eq!(buffers[0], original);
    }

    #[test]
    fn all_components_scale_and_extrema_are_per_component() {
        let root = vec3_root(2);
        let root = root.as_object().expect("object");
        let mut buffers = vec![floats(&[1.0, -2.0, 4.0, 3.0, -6.0, 8.0])];
        let span = accessor_span(root, &buffers, 0, AccessorRule::AllComponents).expect("span");
        let extrema =
            scale_span(&mut buffers, span, AccessorRule::AllComponents, 0.5).expect("rewrite");
        assert_eq!(
            read_span(&buffers, span),
            vec![0.5, -1.0, 2.0, 1.5, -3.0, 4.0]
        );
        assert_eq!(extrema.min, vec![0.5, -3.0, 2.0]);
        assert_eq!(extrema.max, vec![1.5, -1.0, 4.0]);
    }

    #[test]
    fn the_mat4_rule_touches_only_the_translation_column() {
        let root = json!({
            "bufferViews": [{ "buffer": 0, "byteLength": 128 }],
            "accessors": [{ "bufferView": 0, "componentType": 5126, "count": 2, "type": "MAT4" }]
        });
        let root = root.as_object().expect("object");
        let mut source: Vec<f32> = (0..32).map(|i| (i + 1) as f32).collect();
        let mut buffers = vec![floats(&source)];
        let span =
            accessor_span(root, &buffers, 0, AccessorRule::Mat4TranslationColumn).expect("span");
        scale_span(&mut buffers, span, AccessorRule::Mat4TranslationColumn, 4.0).expect("rewrite");
        for element in 0..2 {
            for component in [12usize, 13, 14] {
                source[element * 16 + component] *= 4.0;
            }
        }
        assert_eq!(read_span(&buffers, span), source);
    }

    #[test]
    fn a_mat4_rule_on_a_vec3_accessor_is_refused() {
        let root = vec3_root(2);
        let root = root.as_object().expect("object");
        let buffers = vec![floats(&[0.0; 6])];
        let error = accessor_span(root, &buffers, 0, AccessorRule::Mat4TranslationColumn)
            .expect_err("MAT4 rule on VEC3 storage");
        assert!(matches!(
            error,
            GltfScaleRewriteError::UnrewritableAccessor {
                accessor_index: 0,
                ..
            }
        ));
    }

    #[test]
    fn an_overflowing_element_is_located_and_rejected() {
        let root = vec3_root(1);
        let root = root.as_object().expect("object");
        let mut buffers = vec![floats(&[1.0, 3.0e38, 1.0])];
        let span = accessor_span(root, &buffers, 0, AccessorRule::AllComponents).expect("span");
        let error = scale_span(&mut buffers, span, AccessorRule::AllComponents, 10.0)
            .expect_err("3e38 * 10 overflows f32");
        match error {
            GltfScaleRewriteError::ValueNotRepresentable { location, value } => {
                assert_eq!(location, "/accessors/0[1]");
                assert_eq!(value, 3.0e38f32 as f64 * 10.0);
            }
            other => panic!("expected a located ValueNotRepresentable, got {other:?}"),
        }
    }

    #[test]
    fn an_annihilated_element_is_located_and_rejected() {
        let root = vec3_root(1);
        let root = root.as_object().expect("object");
        let mut buffers = vec![floats(&[0.0, 0.0, 1.0e-30])];
        let span = accessor_span(root, &buffers, 0, AccessorRule::AllComponents).expect("span");
        let error = scale_span(&mut buffers, span, AccessorRule::AllComponents, 1.0e-20)
            .expect_err("1e-30 * 1e-20 flushes to zero in f32");
        match error {
            GltfScaleRewriteError::ValueNotRepresentable { location, value } => {
                assert_eq!(location, "/accessors/0[2]");
                assert_eq!(value, 1.0e-30f32 as f64 * 1.0e-20);
            }
            other => panic!("expected a located ValueNotRepresentable, got {other:?}"),
        }
    }

    #[test]
    fn a_zero_element_is_not_mistaken_for_an_annihilated_one() {
        assert_eq!(narrow(0.0, "/x").expect("zero stays zero"), 0.0);
        assert_eq!(narrow(-0.0, "/x").expect("negative zero stays"), 0.0);
    }
}
