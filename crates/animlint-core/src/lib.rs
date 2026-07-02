//! Engine-agnostic core of animlint: the clip/skeleton data model, the
//! game-runtime-like sampler (`PoseGrid`), measurements, and the check
//! catalog. No file-format knowledge lives here — see `animlint-gltf`
//! (and, later, `animlint-fbx`) for ingestion.

pub mod check;
pub mod checks;
pub mod finding;
pub mod measure;
pub mod model;
pub mod sample;

pub use check::{Check, mechanical_checks, run_checks};
pub use finding::{Finding, Severity, Value};
pub use model::{
    Bone, BoneId, Clip, Document, Interpolation, Property, Skeleton, SourceInfo, Track,
    TrackValues, Transform,
};
pub use sample::{PoseGrid, default_frame_count, sample_clip};
