use crate::canonical::settings_identity;
use crate::{
    DefaultStatus, EngineDeclaration, EngineProfile, InvalidSettingReason, ProfileSelection,
    ResolutionError, SettingApplicability, SettingDomain, SettingId, SettingLocation, SettingMap,
    SettingScope, SettingValue, profiles_v1,
};
use animsmith_core::{
    ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS, EngineContractError, InputIdentity, SourceFormatV1,
};
use std::collections::BTreeMap;

/// Maximum UTF-8 byte count of a V1 source-transform path.
pub const SOURCE_TRANSFORM_PATH_MAX_BYTES: usize = 4_096;

/// Maximum actual clip settings rows retained by the bounded V2 projection.
///
/// The V2 path inspects at most this many rows plus one overflow sentinel. It
/// never clones, validates, resolves, or sorts the sentinel row.
pub const RESOLVED_ENGINE_SETTINGS_V2_MAX_CLIPS: usize = ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS;

/// Find one exact registry tuple without aliases, ranges, or fallback.
///
/// # Errors
///
/// Returns [`ResolutionError::UnknownProfile`] when no tuple is exactly equal.
pub fn lookup_profile(
    selection: &ProfileSelection,
) -> Result<&'static EngineProfile, ResolutionError> {
    profiles_v1()
        .iter()
        .find(|profile| profile.selection() == selection)
        .ok_or_else(|| ResolutionError::UnknownProfile(selection.clone()))
}

/// Validate an engine declaration without inspecting an asset.
///
/// With neither selection nor settings this returns `Ok(None)`, preserving
/// engine-neutral behavior. Every declaration is otherwise validated,
/// including settings beneath selectors that may match no eventual clip.
///
/// # Errors
///
/// Returns a typed configuration error for settings without a selection, an
/// unknown tuple/key, wrong scope, not-applicable setting, invalid value, or
/// missing required document setting.
pub fn resolve_static(
    declaration: EngineDeclaration,
) -> Result<Option<StaticResolution>, ResolutionError> {
    let EngineDeclaration {
        selection,
        document_settings,
        clip_settings,
    } = declaration;
    let Some(selection) = selection else {
        if document_settings.is_some() || !clip_settings.is_empty() {
            return Err(ResolutionError::SettingsWithoutSelection);
        }
        return Ok(None);
    };
    let profile = lookup_profile(&selection)?;

    let document_settings = validate_map(
        profile,
        document_settings.unwrap_or_default(),
        SettingScope::Document,
        SettingLocation::Document,
    )?;
    for descriptor in profile.setting_descriptors() {
        if descriptor.applicability() == SettingApplicability::Applicable
            && descriptor.scope() == SettingScope::Document
            && descriptor.default_status() == DefaultStatus::RequiredWithoutDefault
            && !document_settings.contains_key(&descriptor.id())
        {
            return Err(ResolutionError::MissingRequiredSetting {
                setting: descriptor.id(),
                location: SettingLocation::Document,
            });
        }
    }

    let mut validated_clips = BTreeMap::new();
    for (selector, settings) in clip_settings {
        let values = validate_map(
            profile,
            settings,
            SettingScope::Clip,
            SettingLocation::ClipSelector(selector.clone()),
        )?;
        validated_clips.insert(selector, values);
    }

    Ok(Some(StaticResolution {
        profile,
        document_settings,
        clip_overlays: validated_clips,
    }))
}

/// Validated profile and declarations that require no asset I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticResolution {
    profile: &'static EngineProfile,
    document_settings: BTreeMap<SettingId, SettingValue>,
    clip_overlays: BTreeMap<String, BTreeMap<SettingId, SettingValue>>,
}

impl StaticResolution {
    /// Exact immutable selected profile.
    pub const fn profile(&self) -> &'static EngineProfile {
        self.profile
    }

    /// Fully materialized required document settings.
    pub const fn document_settings(&self) -> &BTreeMap<SettingId, SettingValue> {
        &self.document_settings
    }

    /// Resolve the authoritative input format and actual clip names.
    ///
    /// Non-exact globs apply in lexical selector order, then an exact clip-name
    /// table applies last. Overlay is field-by-field. The result contains one
    /// sorted row per actual clip, including repeated clip names.
    ///
    /// # Errors
    ///
    /// Returns [`ResolutionError::UnacceptedInputFormat`] outside the profile's
    /// bounded V1 input set, [`ResolutionError::MissingRequiredSetting`] when
    /// any real clip lacks an applicable required-without-default setting, or
    /// [`ResolutionError::ResolvedSettingsContract`] when the fully
    /// materialized settings exceed the V1 row/text bounds.
    pub fn resolve_input(
        &self,
        source_format: SourceFormatV1,
        clip_names: &[String],
    ) -> Result<ResolvedProfile, ResolutionError> {
        self.resolve_input_iter(source_format, clip_names.iter().map(String::as_str))
    }

    /// Resolve input from a borrowed, exactly sized clip-name iterator.
    ///
    /// This entry point lets loaders reject an oversized actual-clip inventory
    /// before cloning or traversing any names.
    ///
    /// # Errors
    ///
    /// Returns the same typed errors as [`Self::resolve_input`]. An iterator
    /// longer than the V1 collection bound is rejected before its first item is
    /// consumed.
    pub fn resolve_input_iter<'a>(
        &self,
        source_format: SourceFormatV1,
        clip_names: impl ExactSizeIterator<Item = &'a str>,
    ) -> Result<ResolvedProfile, ResolutionError> {
        if !self.profile.accepted_inputs().contains(&source_format) {
            return Err(ResolutionError::UnacceptedInputFormat {
                selection: self.profile.selection().clone(),
                format: source_format,
            });
        }

        let clip_count = clip_names.len();
        if clip_count > ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS {
            return Err(ResolutionError::ResolvedSettingsContract(
                EngineContractError::TooManyRows {
                    field: "settings.clips",
                    found: clip_count,
                    max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                },
            ));
        }

        let mut clips = Vec::with_capacity(clip_count);
        for clip_name in clip_names {
            let mut materialized = BTreeMap::new();
            for (selector, settings) in &self.clip_overlays {
                if selector != clip_name && animsmith_core::config::glob_match(selector, clip_name)
                {
                    materialized.extend(settings.iter().map(|(id, value)| (*id, value.clone())));
                }
            }
            if let Some(exact) = self.clip_overlays.get(clip_name) {
                materialized.extend(exact.iter().map(|(id, value)| (*id, value.clone())));
            }
            for descriptor in self.profile.setting_descriptors() {
                if descriptor.applicability() == SettingApplicability::Applicable
                    && descriptor.scope() == SettingScope::Clip
                    && descriptor.default_status() == DefaultStatus::RequiredWithoutDefault
                    && !materialized.contains_key(&descriptor.id())
                {
                    return Err(ResolutionError::MissingRequiredSetting {
                        setting: descriptor.id(),
                        location: SettingLocation::ClipSelector(clip_name.to_owned()),
                    });
                }
            }
            clips.push(ResolvedClipSettings {
                clip_name: clip_name.to_owned(),
                settings: materialized,
            });
        }
        clips.sort_by(|left, right| left.clip_name.cmp(&right.clip_name));
        let identity = settings_identity(
            self.profile,
            &self.document_settings,
            clips
                .iter()
                .map(|clip| (clip.clip_name.as_str(), &clip.settings)),
        )?;
        Ok(ResolvedProfile {
            profile: self.profile,
            source_format,
            document_settings: self.document_settings.clone(),
            clips,
            settings_identity: identity,
        })
    }

    /// Resolve an input into the bounded V2 settings projection.
    ///
    /// In contrast with [`Self::resolve_input_iter`], an actual inventory
    /// larger than 4,096 is a successful, explicitly partial result. The
    /// source iterator is only consumed for the retained prefix. The exact
    /// iterator length supplies the N+1 overflow sentinel, so no tail name or
    /// settings value is cloned, validated, materialized, or canonicalized.
    ///
    /// Prefix validation deliberately happens before the overflow result: a
    /// malformed retained row is still an ordinary typed configuration error,
    /// while a defect solely in the tail cannot outrank bounded overflow.
    ///
    /// # Errors
    ///
    /// Returns the same configuration and profile errors as V1 for the
    /// document and retained source-order prefix.
    pub fn resolve_input_v2_iter<'a>(
        &self,
        source_format: SourceFormatV1,
        clip_names: impl ExactSizeIterator<Item = &'a str>,
    ) -> Result<ResolvedProfileV2, ResolutionError> {
        if !self.profile.accepted_inputs().contains(&source_format) {
            return Err(ResolutionError::UnacceptedInputFormat {
                selection: self.profile.selection().clone(),
                format: source_format,
            });
        }

        let actual_clip_rows = clip_names.len();
        let retained_clip_rows = actual_clip_rows.min(RESOLVED_ENGINE_SETTINGS_V2_MAX_CLIPS);
        let overflowed = actual_clip_rows > RESOLVED_ENGINE_SETTINGS_V2_MAX_CLIPS;
        let mut clips = Vec::with_capacity(retained_clip_rows);
        for clip_name in clip_names.take(retained_clip_rows) {
            clips.push(self.materialize_clip(clip_name)?);
        }
        clips.sort_by(|left, right| left.clip_name.cmp(&right.clip_name));

        // This is intentionally the unchanged V1 preimage for the retained
        // prefix only. The V2 wire identity additionally commits to coverage
        // and work counters in its core-owned projection.
        let settings_identity = settings_identity(
            self.profile,
            &self.document_settings,
            clips
                .iter()
                .map(|clip| (clip.clip_name.as_str(), &clip.settings)),
        )?;
        Ok(ResolvedProfileV2 {
            profile: self.profile,
            source_format,
            document_settings: self.document_settings.clone(),
            clips,
            settings_identity,
            clip_coverage: if overflowed {
                ResolvedClipCoverageV2::Partial {
                    reason: ResolvedClipCoverageReasonV2::ActualClipRowsExceeded,
                }
            } else {
                ResolvedClipCoverageV2::Complete
            },
            work: ResolvedEngineSettingsWorkV2 {
                actual_clip_rows_inspected: actual_clip_rows
                    .min(RESOLVED_ENGINE_SETTINGS_V2_MAX_CLIPS.saturating_add(1)),
                materialized_clip_rows: retained_clip_rows,
                retained_clip_rows,
            },
        })
    }

    fn materialize_clip(&self, clip_name: &str) -> Result<ResolvedClipSettings, ResolutionError> {
        let mut materialized = BTreeMap::new();
        for (selector, settings) in &self.clip_overlays {
            if selector != clip_name && animsmith_core::config::glob_match(selector, clip_name) {
                materialized.extend(settings.iter().map(|(id, value)| (*id, value.clone())));
            }
        }
        if let Some(exact) = self.clip_overlays.get(clip_name) {
            materialized.extend(exact.iter().map(|(id, value)| (*id, value.clone())));
        }
        for descriptor in self.profile.setting_descriptors() {
            if descriptor.applicability() == SettingApplicability::Applicable
                && descriptor.scope() == SettingScope::Clip
                && descriptor.default_status() == DefaultStatus::RequiredWithoutDefault
                && !materialized.contains_key(&descriptor.id())
            {
                return Err(ResolutionError::MissingRequiredSetting {
                    setting: descriptor.id(),
                    location: SettingLocation::ClipSelector(clip_name.to_owned()),
                });
            }
        }
        Ok(ResolvedClipSettings {
            clip_name: clip_name.to_owned(),
            settings: materialized,
        })
    }
}

/// Fully materialized settings for one real clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedClipSettings {
    clip_name: String,
    settings: BTreeMap<SettingId, SettingValue>,
}

impl ResolvedClipSettings {
    /// Actual clip name supplied by the caller.
    pub fn clip_name(&self) -> &str {
        &self.clip_name
    }

    /// Fully materialized applicable clip settings.
    pub const fn settings(&self) -> &BTreeMap<SettingId, SettingValue> {
        &self.settings
    }
}

/// Input-validated profile and fully materialized deterministic settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProfile {
    profile: &'static EngineProfile,
    source_format: SourceFormatV1,
    document_settings: BTreeMap<SettingId, SettingValue>,
    clips: Vec<ResolvedClipSettings>,
    settings_identity: InputIdentity,
}

/// Complete or explicitly partial actual-clip coverage in a V2 resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedClipCoverageV2 {
    /// Every actual clip has a retained materialized settings row.
    Complete,
    /// The retained prefix is valid but the actual inventory exceeded its cap.
    Partial {
        /// Stable reason for partial settings coverage.
        reason: ResolvedClipCoverageReasonV2,
    },
}

/// Stable reason why V2 settings do not cover every actual clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedClipCoverageReasonV2 {
    /// The source reported more actual clips than the retained V2 prefix.
    ActualClipRowsExceeded,
}

/// Bounded work accounting for one V2 settings resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedEngineSettingsWorkV2 {
    actual_clip_rows_inspected: usize,
    materialized_clip_rows: usize,
    retained_clip_rows: usize,
}

impl ResolvedEngineSettingsWorkV2 {
    /// Actual source rows inspected, capped at the retained limit plus one.
    pub const fn actual_clip_rows_inspected(&self) -> usize {
        self.actual_clip_rows_inspected
    }

    /// Rows whose settings were materialized.
    pub const fn materialized_clip_rows(&self) -> usize {
        self.materialized_clip_rows
    }

    /// Rows retained in canonical lexical-name order.
    pub const fn retained_clip_rows(&self) -> usize {
        self.retained_clip_rows
    }
}

/// V2 bounded engine resolution retaining a valid canonical prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProfileV2 {
    profile: &'static EngineProfile,
    source_format: SourceFormatV1,
    document_settings: BTreeMap<SettingId, SettingValue>,
    clips: Vec<ResolvedClipSettings>,
    settings_identity: InputIdentity,
    clip_coverage: ResolvedClipCoverageV2,
    work: ResolvedEngineSettingsWorkV2,
}

impl ResolvedProfileV2 {
    /// Exact immutable selected profile.
    pub const fn profile(&self) -> &'static EngineProfile {
        self.profile
    }

    /// Authoritative accepted source format.
    pub const fn source_format(&self) -> SourceFormatV1 {
        self.source_format
    }

    /// Fully materialized document settings.
    pub const fn document_settings(&self) -> &BTreeMap<SettingId, SettingValue> {
        &self.document_settings
    }

    /// Retained clip settings rows in canonical lexical-name order.
    pub fn clip_settings(&self) -> &[ResolvedClipSettings] {
        &self.clips
    }

    /// V1-prefix identity, retained only as an input to the V2 projection.
    pub const fn settings_identity(&self) -> &InputIdentity {
        &self.settings_identity
    }

    /// Coverage state for the actual source clip inventory.
    pub const fn clip_coverage(&self) -> ResolvedClipCoverageV2 {
        self.clip_coverage
    }

    /// Bounded work counters.
    pub const fn work(&self) -> ResolvedEngineSettingsWorkV2 {
        self.work
    }
}

impl ResolvedProfile {
    /// Exact immutable selected profile.
    pub const fn profile(&self) -> &'static EngineProfile {
        self.profile
    }

    /// Authoritative accepted source format.
    pub const fn source_format(&self) -> SourceFormatV1 {
        self.source_format
    }

    /// Fully materialized document settings.
    pub const fn document_settings(&self) -> &BTreeMap<SettingId, SettingValue> {
        &self.document_settings
    }

    /// Fully materialized clip rows in lexical actual-name order.
    pub fn clip_settings(&self) -> &[ResolvedClipSettings] {
        &self.clips
    }

    /// Canonical settings SHA-256 plus byte count.
    pub const fn settings_identity(&self) -> &InputIdentity {
        &self.settings_identity
    }
}

fn validate_map(
    profile: &EngineProfile,
    settings: SettingMap,
    supplied_scope: SettingScope,
    location: SettingLocation,
) -> Result<BTreeMap<SettingId, SettingValue>, ResolutionError> {
    let mut validated = BTreeMap::new();
    for (key, value) in settings {
        let Some(id) = SettingId::from_str(&key) else {
            return Err(ResolutionError::UnknownSetting {
                key,
                location: location.clone(),
            });
        };
        let Some(descriptor) = profile.setting_descriptor(id) else {
            return Err(ResolutionError::UnknownSetting {
                key,
                location: location.clone(),
            });
        };
        if descriptor.applicability() == SettingApplicability::NotApplicable {
            return Err(ResolutionError::NotApplicable {
                setting: id,
                location: location.clone(),
            });
        }
        if descriptor.scope() != supplied_scope {
            return Err(ResolutionError::WrongScope {
                setting: id,
                expected: descriptor.scope(),
                found: supplied_scope,
                location: location.clone(),
            });
        }
        validate_value(id, descriptor.domain(), &value, location.clone())?;
        validated.insert(id, value);
    }
    Ok(validated)
}

fn validate_value(
    setting: SettingId,
    domain: SettingDomain,
    value: &SettingValue,
    location: SettingLocation,
) -> Result<(), ResolutionError> {
    let domain_matches = matches!(
        (domain, value),
        (SettingDomain::Boolean, SettingValue::Boolean(_))
            | (SettingDomain::BakeOrExtract, SettingValue::BakeOrExtract(_))
            | (
                SettingDomain::SourceTransformPath,
                SettingValue::SourceTransformPath(_)
            )
    );
    if !domain_matches {
        return Err(ResolutionError::InvalidSettingValue {
            setting,
            location,
            reason: InvalidSettingReason::WrongDomain {
                expected: domain,
                found: value.kind(),
            },
        });
    }
    if let SettingValue::SourceTransformPath(path) = value {
        let reason = if path.is_empty() {
            Some(InvalidSettingReason::EmptyPath)
        } else if path.len() > SOURCE_TRANSFORM_PATH_MAX_BYTES {
            Some(InvalidSettingReason::PathTooLong {
                bytes: path.len(),
                limit: SOURCE_TRANSFORM_PATH_MAX_BYTES,
            })
        } else if path.starts_with('/') {
            Some(InvalidSettingReason::AbsolutePath)
        } else if path.chars().any(char::is_control) {
            Some(InvalidSettingReason::ControlCharacter)
        } else if path.split('/').any(str::is_empty) {
            Some(InvalidSettingReason::EmptyPathSegment)
        } else if path.split('/').any(|segment| matches!(segment, "." | "..")) {
            Some(InvalidSettingReason::DotPathSegment)
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(ResolutionError::InvalidSettingValue {
                setting,
                location,
                reason,
            });
        }
    }
    Ok(())
}
