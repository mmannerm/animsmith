//! Docs.rs API map: [`Document`] holds skeletons, clips, and scene
//! assets; [`measure::measure_document`] computes measurements;
//! [`CheckCtx::new`] prepares a check run; [`run_checks`] executes
//! [`all_checks`]. Use [`detect_profile`] or
//! [`ResolvedRoles::from_names`] to resolve rig roles. [`PoseGrid`] is
//! the sampled pose cache shared by semantic checks.
//!
//! # API status
//!
//! The Rust API is pre-1.0 and intentionally marked experimental while
//! animsmith is still settling its catalog, JSON contract, and loader
//! boundaries. The intended embedding surface is the data model,
//! configuration types, measurement APIs, rig-profile APIs, and the check
//! catalog functions re-exported from this crate root. Internal built-in
//! check modules are private implementation details.

#![doc = "\n\n"]
#![doc = include_str!("../README.md")]

pub mod check;
mod checks;
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
pub use glam;
pub use model::{
    Bone, BoneId, Clip, Document, Interpolation, Property, Skeleton, SourceInfo, Track,
    TrackValues, Transform,
};
pub use profile::{ResolvedRoles, RigProfile, Role, builtin_profiles, detect_profile};
pub use sample::{PoseGrid, TrackSample, default_frame_count, sample_clip, sample_track};
