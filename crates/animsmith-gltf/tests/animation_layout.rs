//! An animation sampler accessor addressed in a buffer layout its `gltf`
//! reader cannot walk must produce a `LoadError`, never a panic and never a
//! silently emptied track (invariant-1).
//!
//! This is the *layout* half of a sampler accessor — view extent, stride,
//! sparse count — which is the same class `vertex_encodings.rs` pins for a
//! primitive slot, one call site over. The *encoding* half (a sampler
//! accessor typed for a different element than its reader decodes) is
//! tracked separately in issue #327 and is not asserted here.
//!
//! Both halves of the reader are covered: `read_inputs` and `read_outputs`
//! each build their own `Iter`, over a dense view or over a sparse block's
//! index and value views.

use animsmith_gltf::LoadError;
use base64::Engine as _;
use serde_json::{Value, json};

const UNSIGNED_SHORT: u32 = 5123;
const FLOAT: u32 = 5126;

/// A `byteOffset`/`byteLength` pair written onto a buffer view verbatim,
/// whatever bytes it actually holds. `gltf-json` relates the two fields to
/// each other no more than it relates them to the buffer, which is the only
/// way to declare a view whose own extent has no end.
#[derive(Default, Clone, Copy)]
struct ViewExtent {
    byte_offset: Option<u64>,
    byte_length: Option<u64>,
}

impl ViewExtent {
    /// The extent named in the issue: an offset one below 2^64 against a
    /// plausible length, so `byteOffset + byteLength` has no `usize` answer.
    fn overflowing() -> Self {
        Self {
            byte_offset: Some(u64::MAX),
            byte_length: Some(12),
        }
    }

    fn apply(self, view: &mut Value) {
        if let Some(byte_offset) = self.byte_offset {
            view["byteOffset"] = json!(byte_offset);
        }
        if let Some(byte_length) = self.byte_length {
            view["byteLength"] = json!(byte_length);
        }
    }
}

/// A `sparse` block on a sampler accessor, always with `UNSIGNED_SHORT`
/// indices.
#[derive(Clone)]
struct Sparse {
    /// Written to `sparse.count` verbatim.
    declared_count: u64,
    indices: Vec<u16>,
    /// Replacement elements, in the accessor's own encoding.
    values: Vec<u8>,
    /// `byteStride` on the view holding those replacement elements.
    values_stride: Option<u64>,
    indices_view_extent: ViewExtent,
    values_view_extent: ViewExtent,
}

impl Sparse {
    /// A well-formed block replacing element 0 with `values`.
    fn replacing_first(values: Vec<u8>) -> Self {
        Self {
            declared_count: 1,
            indices: vec![0],
            values,
            values_stride: None,
            indices_view_extent: ViewExtent::default(),
            values_view_extent: ViewExtent::default(),
        }
    }

    fn declaring_count(mut self, count: u64) -> Self {
        self.declared_count = count;
        self
    }

    fn values_stride(mut self, stride: u64) -> Self {
        self.values_stride = Some(stride);
        self
    }

    fn indices_view_extent(mut self, extent: ViewExtent) -> Self {
        self.indices_view_extent = extent;
        self
    }

    fn values_view_extent(mut self, extent: ViewExtent) -> Self {
        self.values_view_extent = extent;
        self
    }
}

/// How one sampler accessor addresses its bytes, as distinct from what
/// element it declares.
#[derive(Default, Clone)]
struct Layout {
    /// `byteStride` on the buffer view this accessor owns.
    stride: Option<u64>,
    /// Overwrites the declared extent of that view.
    view_extent: ViewExtent,
    /// Written to the accessor's `count` verbatim, replacing the element
    /// count the fixture's bytes actually hold.
    declared_count: Option<u64>,
    sparse: Option<Sparse>,
}

/// One sampler accessor: `input` times or `output` values.
#[derive(Clone)]
struct Sampler {
    accessor_type: &'static str,
    count: usize,
    bytes: Vec<u8>,
    /// Emitted for the `input` accessor, which glTF requires bounds on.
    bounds: Option<(Vec<f32>, Vec<f32>)>,
    layout: Layout,
}

/// A single-channel rotation clip over one node, whose two sampler
/// accessors are individually replaceable — so one test changes exactly one
/// accessor's layout and nothing else.
struct Clip {
    input: Sampler,
    output: Sampler,
}

impl Clip {
    /// Three keyframes of identity rotation: the plainest well-formed clip
    /// this loader accepts.
    fn new() -> Self {
        Self {
            input: Sampler {
                accessor_type: "SCALAR",
                count: 3,
                bytes: f32s(&[0.0, 0.5, 1.0]),
                bounds: Some((vec![0.0], vec![1.0])),
                layout: Layout::default(),
            },
            output: Sampler {
                accessor_type: "VEC4",
                count: 3,
                bytes: f32s(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
                bounds: None,
                layout: Layout::default(),
            },
        }
    }

    /// Apply a layout override to the sampler's `input` accessor.
    fn input(mut self, edit: impl FnOnce(&mut Layout)) -> Self {
        edit(&mut self.input.layout);
        self
    }

    /// Apply a layout override to the sampler's `output` accessor.
    fn output(mut self, edit: impl FnOnce(&mut Layout)) -> Self {
        edit(&mut self.output.layout);
        self
    }

    fn to_json(&self) -> Vec<u8> {
        let mut blob = Vec::new();
        let mut views = Vec::new();
        let mut accessors = Vec::new();
        for sampler in [&self.input, &self.output] {
            let view = push_view(&mut blob, &mut views, &sampler.bytes, sampler.layout.stride);
            sampler.layout.view_extent.apply(&mut views[view]);
            let mut accessor = json!({
                "bufferView": view,
                "componentType": FLOAT,
                "count": sampler.layout.declared_count.unwrap_or(sampler.count as u64),
                "type": sampler.accessor_type
            });
            if let Some((min, max)) = &sampler.bounds {
                accessor["min"] = json!(min);
                accessor["max"] = json!(max);
            }
            if let Some(sparse) = &sampler.layout.sparse {
                let indices = push_view(&mut blob, &mut views, &u16s(&sparse.indices), None);
                sparse.indices_view_extent.apply(&mut views[indices]);
                let values = push_view(&mut blob, &mut views, &sparse.values, sparse.values_stride);
                sparse.values_view_extent.apply(&mut views[values]);
                accessor["sparse"] = json!({
                    "count": sparse.declared_count,
                    "indices": {
                        "bufferView": indices,
                        "byteOffset": 0,
                        "componentType": UNSIGNED_SHORT
                    },
                    "values": { "bufferView": values, "byteOffset": 0 }
                });
            }
            accessors.push(accessor);
        }
        let document = json!({
            "asset": { "version": "2.0" },
            "buffers": [{
                "uri": format!(
                    "data:application/octet-stream;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&blob)
                ),
                "byteLength": blob.len().max(1)
            }],
            "bufferViews": views,
            "accessors": accessors,
            "nodes": [{ "name": "root" }],
            "animations": [{
                "name": "poisoned",
                "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
                "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
            }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        });
        serde_json::to_vec(&document).expect("serializes synthetic glTF")
    }

    fn load(&self) -> Result<animsmith_core::model::Document, LoadError> {
        animsmith_gltf::load_bytes(std::path::Path::new("synthetic.gltf"), &self.to_json())
    }

    fn expect_layout_refusal(&self, expected: &str) {
        let error = self
            .load()
            .expect_err("a sampler accessor the reader cannot walk must be refused");
        assert!(
            matches!(error, LoadError::AnimationAccessorLayout { .. }),
            "expected an AnimationAccessorLayout refusal, got {error:?}"
        );
        assert_eq!(error.to_string(), expected);
    }
}

fn f32s(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn u16s(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Append `bytes` to the buffer as a fresh 4-byte-aligned buffer view.
/// glTF forbids a zero-length view, so an empty one still gets a byte.
fn push_view(
    blob: &mut Vec<u8>,
    views: &mut Vec<Value>,
    bytes: &[u8],
    stride: Option<u64>,
) -> usize {
    while !blob.len().is_multiple_of(4) {
        blob.push(0);
    }
    let mut view = json!({
        "buffer": 0,
        "byteOffset": blob.len(),
        "byteLength": bytes.len().max(1)
    });
    if let Some(stride) = stride {
        view["byteStride"] = json!(stride);
    }
    blob.extend_from_slice(bytes);
    if bytes.is_empty() {
        blob.push(0);
    }
    views.push(view);
    views.len() - 1
}

// --- Shape 1: a buffer view whose own extent overflows ----------------
//
// Three views reach the reader for one accessor — its own, and a sparse
// block's index and value views — and none of them is bounded by
// `gltf-json`. All three are poisoned below, on both slots.

/// The input accessor's own view has no end. `buffer_view_slice` adds
/// `byteOffset` to `byteLength` before any accessor arithmetic runs, so this
/// panics in a debug build and, worse, silently drops the whole track in a
/// release one — the sum wraps to a start past its end and the slice fails.
#[test]
fn sampler_input_over_an_unbounded_view_is_refused() {
    Clip::new()
        .input(|layout| layout.view_extent = ViewExtent::overflowing())
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler input: accessor 0 reads its elements from buffer \
             view 0, whose byteOffset 18446744073709551615 plus byteLength 12 is a byte extent \
             that overflows",
        );
}

/// The same defect on the other half of the reader: `read_outputs` builds a
/// second, independent `Iter` over the output accessor.
#[test]
fn sampler_output_over_an_unbounded_view_is_refused() {
    Clip::new()
        .output(|layout| layout.view_extent = ViewExtent::overflowing())
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler output: accessor 1 reads its elements from buffer \
             view 1, whose byteOffset 18446744073709551615 plus byteLength 12 is a byte extent \
             that overflows",
        );
}

/// A sparse block's *index* view is a third view the reader slices, and it
/// is bounded no better than the accessor's own.
#[test]
fn sampler_input_over_an_unbounded_sparse_index_view_is_refused() {
    Clip::new()
        .input(|layout| {
            layout.sparse = Some(
                Sparse::replacing_first(f32s(&[0.25]))
                    .indices_view_extent(ViewExtent::overflowing()),
            );
        })
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler input: accessor 0 reads its sparse indices from \
             buffer view 1, whose byteOffset 18446744073709551615 plus byteLength 12 is a byte \
             extent that overflows",
        );
}

/// Its *value* view is the fourth, reached here through `read_outputs`.
#[test]
fn sampler_output_over_an_unbounded_sparse_value_view_is_refused() {
    Clip::new()
        .output(|layout| {
            layout.sparse = Some(
                Sparse::replacing_first(f32s(&[0.0, 0.0, 0.0, 1.0]))
                    .values_view_extent(ViewExtent::overflowing()),
            );
        })
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler output: accessor 1 reads its sparse values from \
             buffer view 3, whose byteOffset 18446744073709551615 plus byteLength 12 is a byte \
             extent that overflows",
        );
}

/// The same index view on the output half.
#[test]
fn sampler_output_over_an_unbounded_sparse_index_view_is_refused() {
    Clip::new()
        .output(|layout| {
            layout.sparse = Some(
                Sparse::replacing_first(f32s(&[0.0, 0.0, 0.0, 1.0]))
                    .indices_view_extent(ViewExtent::overflowing()),
            );
        })
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler output: accessor 1 reads its sparse indices from \
             buffer view 2, whose byteOffset 18446744073709551615 plus byteLength 12 is a byte \
             extent that overflows",
        );
}

/// And a `sparse` block's *value* view on the input half.
#[test]
fn sampler_input_over_an_unbounded_sparse_value_view_is_refused() {
    Clip::new()
        .input(|layout| {
            layout.sparse = Some(
                Sparse::replacing_first(f32s(&[0.25]))
                    .values_view_extent(ViewExtent::overflowing()),
            );
        })
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler input: accessor 0 reads its sparse values from \
             buffer view 2, whose byteOffset 18446744073709551615 plus byteLength 12 is a byte \
             extent that overflows",
        );
}

// --- Shape 2: a `byteStride` shorter than the element -----------------
//
// The one shape here that panics in a **release** build too: the
// `debug_assert!(stride >= size_of::<T>())` is compiled out, but the
// truncated slice then reaches `Item::from_slice`'s hard `assert!`.
//
// It is expressible only where the element is wider than `Stride`'s own
// floor of 4, which the `output` accessor is (`VEC4` of `FLOAT`, 16 bytes)
// and a `SCALAR`-of-`FLOAT` `input` never is — see
// `sampler_input_on_the_smallest_legal_stride_still_loads`.

/// A `byteStride` shorter than the element it strides over; `Stride`'s own
/// validator accepts any `4..=252` without ever seeing the element.
#[test]
fn sampler_output_on_a_short_stride_is_refused() {
    Clip::new()
        .output(|layout| layout.stride = Some(4))
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler output: accessor 1 reads its elements from buffer \
             view 1 at byteStride 4, shorter than the 16-byte element it strides over",
        );
}

/// The same shortfall on the sparse route, where the reader compiles no
/// stride assertion at all and goes straight to `Item::from_slice`.
#[test]
fn sampler_output_on_short_strided_sparse_values_is_refused() {
    Clip::new()
        .output(|layout| {
            layout.sparse =
                Some(Sparse::replacing_first(f32s(&[0.0, 0.0, 0.0, 1.0])).values_stride(4));
        })
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler output: accessor 1 reads its sparse values from \
             buffer view 3 at byteStride 4, shorter than the 16-byte element it strides over",
        );
}

// --- Shape 3: an extent that overflows `usize` ------------------------

/// A `count` large enough that stepping to its last element overflows
/// `usize`, which nothing in `gltf-json` bounds.
#[test]
fn sampler_input_whose_extent_overflows_is_refused() {
    Clip::new()
        .input(|layout| layout.declared_count = Some(u64::MAX))
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler input: accessor 0 walks 18446744073709551615 \
             elements of 4 bytes at byteStride 4 from byteOffset 0, a byte extent that overflows",
        );
}

/// The same on the output half, where the element is four times as wide.
#[test]
fn sampler_output_whose_extent_overflows_is_refused() {
    Clip::new()
        .output(|layout| layout.declared_count = Some(u64::MAX))
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler output: accessor 1 walks 18446744073709551615 \
             elements of 16 bytes at byteStride 16 from byteOffset 0, a byte extent that \
             overflows",
        );
}

// --- Shape 4: a `sparse` block of count 0 -----------------------------

/// `sparse.count` of 0 underflows in `SparseIter`. Every count-zero guard in
/// the loader reads `Accessor::count()`; none of them sees this one.
#[test]
fn sampler_input_with_a_zero_count_sparse_block_is_refused() {
    Clip::new()
        .input(|layout| {
            layout.sparse = Some(Sparse::replacing_first(f32s(&[0.25])).declaring_count(0));
        })
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler input: accessor 0 declares a sparse block of count \
             0, which its reader cannot walk",
        );
}

/// And on the output half, which reaches `SparseIter` through its own
/// reader.
#[test]
fn sampler_output_with_a_zero_count_sparse_block_is_refused() {
    Clip::new()
        .output(|layout| {
            layout.sparse =
                Some(Sparse::replacing_first(f32s(&[0.0, 0.0, 0.0, 1.0])).declaring_count(0));
        })
        .expect_layout_refusal(
            "clip 'poisoned' node 0 sampler output: accessor 1 declares a sparse block of count \
             0, which its reader cannot walk",
        );
}

// --- Layouts that must keep loading -----------------------------------

/// The over-rejection boundary of the stride check, and the reason there is
/// no short-stride fixture for a sampler `input`: `Stride`'s validator
/// floors `byteStride` at 4, and a `SCALAR` of `FLOAT` is 4 bytes, so an
/// input stride can never fall *below* its element — only meet it exactly.
/// The check is `stride < size`, and this pins that it is not `<=`.
#[test]
fn sampler_input_on_the_smallest_legal_stride_still_loads() {
    let document = Clip::new()
        .input(|layout| layout.stride = Some(4))
        .load()
        .expect("a stride equal to the element must load");
    assert_eq!(document.clips[0].tracks[0].times, vec![0.0, 0.5, 1.0]);
}

/// The other half of the guard: a sparse layout glTF legitimately permits
/// must still load, and load the values the sparse block substitutes — the
/// refusals above must not cost a valid sampler its clip.
#[test]
fn a_well_formed_sparse_sampler_still_loads() {
    let document = Clip::new()
        .input(|layout| layout.sparse = Some(Sparse::replacing_first(f32s(&[0.25]))))
        .output(|layout| layout.sparse = Some(Sparse::replacing_first(f32s(&[0.0, 0.0, 0.0, 1.0]))))
        .load()
        .expect("a well-formed sparse sampler must load");
    let track = &document.clips[0].tracks[0];
    // Element 0 of the input comes from the sparse block, elements 1 and 2
    // from the base view.
    assert_eq!(track.times, vec![0.25, 0.5, 1.0]);
}
