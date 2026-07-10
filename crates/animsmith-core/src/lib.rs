//! Engine-agnostic animation linting primitives for Rust pipelines.
//!
//! This crate is the embedding boundary for animsmith. It owns the core
//! data model ([`Document`], [`Skeleton`], [`Clip`], [`Track`]), rig-role
//! resolution ([`detect_profile`], [`ResolvedRoles::from_names`]),
//! typed configuration ([`Config`]), measurement generation
//! ([`measure::measure_document`]), measurement diffs
//! ([`diff::diff_measurements`]), structured findings ([`Finding`]), and
//! check execution ([`CheckCtx`], [`all_checks`], [`run_checks`]).
//! The [`animsmith-gltf`] and [`animsmith-fbx`] loader crates translate file
//! formats into this model; their docs.rs pages continue the library path for
//! format-specific loading and writing.
//!
//! The [embedding guide] explains crate selection and integration
//! boundaries. The [pipeline scenario guide] shows where an embedded gate
//! fits in marketplace intake, mocap cleanup, outsourced acceptance, and CI.
//! A [runnable example] exercises the complete library flow.
//!
//! [embedding guide]: https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md
//! [pipeline scenario guide]: https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md
//! [runnable example]: https://github.com/mmannerm/animsmith/blob/main/crates/animsmith/examples/embed.rs
//! [`animsmith-gltf`]: https://docs.rs/animsmith-gltf
//! [`animsmith-fbx`]: https://docs.rs/animsmith-fbx
//!
//! # Quick start
//!
//! After a format crate has loaded a [`Document`], resolve rig roles, build
//! a [`Config`] from the host pipeline's contract, and share one
//! [`MetricGrids`] between measurements, checks, and optional report
//! generation:
//!
//! ```
//! use animsmith_core::{Config, CheckCtx, Document, MetricGrids, all_checks, run_checks};
//! use animsmith_core::measure::measure_document;
//! use animsmith_core::ResolvedRoles;
//!
//! let doc = Document::default();
//! let roles = ResolvedRoles::default();
//! let config = Config::default();
//! let grids = MetricGrids::new(&doc);
//!
//! let measurements = measure_document(&grids, &roles, &config);
//! let ctx = CheckCtx::new(&grids, &roles, &config);
//! let findings = run_checks(&ctx, &all_checks());
//!
//! assert!(measurements.is_empty());
//! assert!(findings.is_empty());
//! ```
//!
//! [`CheckCtx::new`] consumes already-resolved roles; it does not interpret
//! [`Config::rig`] automatically. Frontends may use [`detect_profile`],
//! [`profile::resolve_named`], or [`ResolvedRoles::from_names`], then apply
//! their own override policy before constructing the context. Checks whose
//! required roles remain unresolved emit diagnostic skip notes rather than
//! false failures.
//!
//! # API status
//!
//! The Rust API is pre-1.0 and may still change before the first stable
//! release. The intended extension points are the data model,
//! configuration types, measurement and diff APIs, rig-profile APIs, the
//! [`Check`] trait for custom checks, and the check catalog functions
//! re-exported from this crate root. Built-in check ids, CLI exit-code
//! semantics, and the CLI's versioned JSON envelope/schema id are treated
//! as the most stable automation contracts. The core [`Finding`] and
//! [`measure::ClipMeasurements`] serde shapes feed that envelope, but the
//! envelope version itself lives in the CLI crate. The scene-asset
//! structs in [`model`] and the pipeline-mechanical helpers in
//! [`transform`] are public so the loader, writer, and CLI crates can
//! share the same model, but they are less settled than the
//! measurement/check embedding flow while the crate is pre-1.0. Metric
//! formulas and individual Rust symbols are still subject to pre-1.0
//! refinement.
//!
//! Public APIs that return [`Result`] document their `# Errors` cases.
//! Index-based accessors and transform helpers that rely on
//! loader-established invariants document their `# Panics` contracts.
//! Loader-valid documents from the format crates should flow through
//! checking, sampling, and measurement without panicking on untrusted
//! input.

#![warn(missing_docs)]

pub mod check;
mod checks;
pub mod config;
pub mod diff;
pub mod finding;
#[cfg(feature = "fixtures")]
pub mod fixtures;
pub mod measure;
pub mod metrics;
pub mod model;
pub mod profile;
pub mod sample;
pub mod transform;

pub use check::{Check, CheckCtx, all_checks, mechanical_checks, run_checks};
pub use config::{ClipExpectations, Config, GaitGroup, Pinned, SeveritySetting};
pub use finding::{Finding, Severity, Value};
/// Re-export of the exact `glam` version used by animsmith's public math
/// types, so embedders can construct [`Transform`] values without a
/// cross-version type mismatch.
pub use glam;
pub use metrics::MetricGrids;
pub use model::{
    Bone, BoneId, Clip, Document, Interpolation, Property, Skeleton, SourceInfo, Track,
    TrackValues, Transform,
};
pub use profile::{ResolvedRoles, RigProfile, Role, builtin_profiles, detect_profile};
pub use sample::{PoseGrid, TrackSample, default_frame_count, sample_clip, sample_track};
