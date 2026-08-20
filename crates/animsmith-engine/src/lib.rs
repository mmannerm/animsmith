//! Strict, immutable engine-import profile facts and deterministic settings
//! resolution for AnimSmith.
//!
//! The V1 registry has exactly five version-pinned profiles. [`resolve_static`]
//! validates a profile tuple and every declared setting before asset I/O;
//! [`StaticResolution::resolve_input`] then checks the authoritative
//! [`animsmith_core::SourceFormatV1`] and materializes per-clip settings.
//! Unknown engine behavior remains [`FactState::Unknown`]. This crate performs
//! no filesystem access, parses no TOML, imports no format crate, and produces
//! no predictions.
//!
//! # Example
//!
//! ```
//! use animsmith_core::SourceFormatV1;
//! use animsmith_engine::{
//!     BakeOrExtract, EngineDeclaration, ProfileSelection, SettingValue,
//!     resolve_static,
//! };
//!
//! let mut declaration = EngineDeclaration {
//!     selection: Some(ProfileSelection::new(
//!         "unity-generic", 1, "6000.3", "fbx-model-importer",
//!     )),
//!     document_settings: Some(Default::default()),
//!     ..EngineDeclaration::default()
//! };
//! let document = declaration.document_settings.as_mut().unwrap();
//! document.insert("convert_units".into(), SettingValue::Boolean(true));
//! document.insert("bake_axis_conversion".into(), SettingValue::Boolean(true));
//! document.insert(
//!     "root_motion_source".into(),
//!     SettingValue::SourceTransformPath("Reference/Root".into()),
//! );
//! declaration.clip_settings.insert(
//!     "walk_*".into(),
//!     [
//!         ("root_rotation".into(), SettingValue::BakeOrExtract(BakeOrExtract::Extract)),
//!         ("root_position_y".into(), SettingValue::BakeOrExtract(BakeOrExtract::Bake)),
//!         ("root_position_xz".into(), SettingValue::BakeOrExtract(BakeOrExtract::Extract)),
//!     ].into(),
//! );
//! let static_profile = resolve_static(declaration)?.unwrap();
//! let resolved = static_profile.resolve_input(SourceFormatV1::Fbx, &["walk_forward".into()])?;
//! assert_eq!(resolved.clip_settings().len(), 1);
//! # Ok::<(), animsmith_engine::ResolutionError>(())
//! ```
//!
//! The crate has no public feature flags and supports the workspace MSRV,
//! Rust 1.88. Its Rust API is pre-1.0.

#![warn(missing_docs)]

mod canonical;
mod error;
mod registry;
mod resolver;
mod types;

pub use error::{InvalidSettingReason, RegistryValidationError, ResolutionError, SettingLocation};
pub use registry::{profiles_v1, validate_registry_v1};
pub use resolver::{
    ResolvedClipSettings, ResolvedProfile, SOURCE_TRANSFORM_PATH_MAX_BYTES, StaticResolution,
    lookup_profile, resolve_static,
};
pub use types::{
    AnimationAddressability, BakeOrExtract, ConversionControl, CoordinateBasis, DefaultStatus,
    EngineDeclaration, EngineProfile, FactId, FactState, FactValue, ForwardAxis, Handedness,
    ImportHandling, LinearUnit, PrimarySource, ProfileFact, ProfileSelection,
    RootMotionAddressability, SettingApplicability, SettingDescriptor, SettingDomain, SettingId,
    SettingMap, SettingScope, SettingValue, TargetAddressability, UpAxis,
};

/// Runtime kind of one closed [`SettingValue`] variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingValueKind {
    /// Boolean value.
    Boolean,
    /// Bake-or-extract value.
    BakeOrExtract,
    /// Source-transform path value.
    SourceTransformPath,
}
