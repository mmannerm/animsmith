//! Engine-agnostic core of animsmith: the clip/skeleton data model, the
//! game-runtime-like sampler ([`PoseGrid`]), measurements, rig profiles,
//! configuration, and the check catalog. No file-format knowledge lives
//! here; pair it with `animsmith-gltf` or `animsmith-fbx` at the edge of
//! a pipeline.

pub mod check;
pub mod checks;
pub mod config;
pub mod finding;
pub mod measure;
pub mod metrics;
pub mod model;
pub mod profile;
pub mod sample;
pub mod transform;

pub use check::{Check, CheckCtx, all_checks, mechanical_checks, run_checks};
pub use config::{ClipExpectations, Config, GaitGroup, Pinned, SeveritySetting};
pub use finding::{Finding, Severity, Value};
pub use model::{
    Bone, BoneId, Clip, Document, Interpolation, Property, Skeleton, SourceInfo, Track,
    TrackValues, Transform,
};
pub use profile::{ResolvedRoles, RigProfile, Role, builtin_profiles, detect_profile};
pub use sample::{PoseGrid, TrackSample, default_frame_count, sample_clip, sample_track};
