use crate::{
    AnimationAddressability, BakeOrExtract, ConversionControl, DefaultStatus, EngineProfile,
    FactState, FactValue, ForwardAxis, Handedness, ImportHandling, LinearUnit,
    RootMotionAddressability, SettingApplicability, SettingId, SettingScope, SettingValue,
    TargetAddressability, UpAxis,
};
use animsmith_core::{InputIdentity, SourceFormatV1};
use std::collections::BTreeMap;

/// Fixed-field-order encoding whose every UTF-8 token has an eight-byte
/// big-endian length prefix. Counts are decimal UTF-8 tokens as well.
struct CanonicalEncoder(Vec<u8>);

impl CanonicalEncoder {
    fn new(version: &str) -> Self {
        let mut encoder = Self(Vec::new());
        encoder.token(version);
        encoder
    }

    fn token(&mut self, token: impl AsRef<str>) {
        let bytes = token.as_ref().as_bytes();
        self.0
            .extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        self.0.extend_from_slice(bytes);
    }

    fn count(&mut self, count: usize) {
        self.token(count.to_string());
    }

    fn identity(self) -> InputIdentity {
        InputIdentity::from_bytes(&self.0)
    }
}

pub(crate) fn facts_identity(profile: &EngineProfile) -> InputIdentity {
    let mut encoder = CanonicalEncoder::new("animsmith-engine-facts-v1");
    encode_profile_key(&mut encoder, profile);
    encoder.token("fact_bundle_urn");
    encoder.token(profile.fact_bundle_urn);

    let mut facts: Vec<_> = profile.facts.iter().collect();
    facts.sort_by_key(|fact| fact.id().as_str());
    encoder.token("facts");
    encoder.count(facts.len());
    for fact in facts {
        encoder.token(fact.id().as_str());
        encode_fact_state(&mut encoder, fact.state());
    }

    let mut settings: Vec<_> = profile.settings.iter().collect();
    settings.sort_by_key(|descriptor| descriptor.id().as_str());
    encoder.token("setting_descriptors");
    encoder.count(settings.len());
    for descriptor in settings {
        encoder.token(descriptor.id().as_str());
        encoder.token(scope_name(descriptor.scope()));
        encoder.token(domain_name(descriptor.domain()));
        encoder.token(match descriptor.applicability() {
            SettingApplicability::Applicable => "applicable",
            SettingApplicability::NotApplicable => "not_applicable",
        });
        encoder.token(match descriptor.default_status() {
            DefaultStatus::RequiredWithoutDefault => "required_without_default",
            DefaultStatus::NotApplicable => "not_applicable",
        });
    }

    let mut sources: Vec<_> = profile.sources.iter().collect();
    sources.sort_by_key(|source| source.id());
    encoder.token("sources");
    encoder.count(sources.len());
    for source in sources {
        encoder.token(source.id());
        encoder.token(source.target_version());
        encoder.token(source.url());
        encoder.token(source.verified_on());
        let mut facts = source.supported_facts().to_vec();
        facts.sort_by_key(|id| id.as_str());
        encoder.count(facts.len());
        for fact in facts {
            encoder.token(fact.as_str());
        }
        let mut settings = source.supported_settings().to_vec();
        settings.sort_by_key(|id| id.as_str());
        encoder.count(settings.len());
        for setting in settings {
            encoder.token(setting.as_str());
        }
    }
    encoder.identity()
}

pub(crate) fn settings_identity<'a>(
    profile: &EngineProfile,
    document: &BTreeMap<SettingId, SettingValue>,
    clips: impl IntoIterator<Item = (&'a str, &'a BTreeMap<SettingId, SettingValue>)>,
) -> InputIdentity {
    let mut encoder = CanonicalEncoder::new("animsmith-engine-settings-v1");
    encode_profile_key(&mut encoder, profile);
    encoder.token("fact_bundle_urn");
    encoder.token(profile.fact_bundle_urn);
    encoder.token("document_settings");
    encoder.count(document.len());
    let mut document: Vec<_> = document.iter().collect();
    document.sort_by_key(|(id, _)| id.as_str());
    for (id, value) in document {
        encoder.token(id.as_str());
        encode_setting_value(&mut encoder, value);
    }
    let clips: Vec<_> = clips.into_iter().collect();
    encoder.token("clips");
    encoder.count(clips.len());
    for (name, settings) in clips {
        encoder.token(name);
        encoder.count(settings.len());
        let mut settings: Vec<_> = settings.iter().collect();
        settings.sort_by_key(|(id, _)| id.as_str());
        for (id, value) in settings {
            encoder.token(id.as_str());
            encode_setting_value(&mut encoder, value);
        }
    }
    encoder.identity()
}

fn encode_profile_key(encoder: &mut CanonicalEncoder, profile: &EngineProfile) {
    encoder.token("selection");
    encoder.token(profile.selection.family());
    encoder.token(profile.selection.profile_revision().to_string());
    encoder.token(profile.selection.engine_version());
    encoder.token(profile.selection.importer());
}

fn encode_fact_state(encoder: &mut CanonicalEncoder, state: &FactState) {
    match state {
        FactState::Unknown => encoder.token("unknown"),
        FactState::NotApplicable => encoder.token("not_applicable"),
        FactState::Known(value) => {
            encoder.token("known");
            match value {
                FactValue::AcceptedFormats(formats) => {
                    encoder.token("accepted_formats");
                    let mut formats = formats.clone();
                    formats.sort_by_key(|format| format_name(*format));
                    encoder.count(formats.len());
                    for format in formats {
                        encoder.token(format_name(format));
                    }
                }
                FactValue::AnimationAddressability(value) => {
                    encoder.token("animation_addressability");
                    encoder.token(match value {
                        AnimationAddressability::GltfAssetLabel => "gltf_asset_label",
                    });
                }
                FactValue::CoordinateBasis(value) => {
                    encoder.token("coordinate_basis");
                    encoder.token(match value.handedness {
                        Handedness::Left => "left",
                        Handedness::Right => "right",
                    });
                    encoder.token(match value.up_axis {
                        UpAxis::X => "x",
                        UpAxis::Y => "y",
                        UpAxis::Z => "z",
                    });
                    encoder.token(match value.forward_axis {
                        ForwardAxis::PositiveX => "+x",
                        ForwardAxis::NegativeX => "-x",
                        ForwardAxis::PositiveY => "+y",
                        ForwardAxis::NegativeY => "-y",
                        ForwardAxis::PositiveZ => "+z",
                        ForwardAxis::NegativeZ => "-z",
                    });
                }
                FactValue::LinearUnit(value) => {
                    encoder.token("linear_unit");
                    encoder.token(match value {
                        LinearUnit::Metre => "metre",
                        LinearUnit::Centimetre => "centimetre",
                    });
                }
                FactValue::ConversionControl(value) => {
                    encoder.token("conversion_control");
                    match value {
                        ConversionControl::ProfileSetting(setting) => {
                            encoder.token("profile_setting");
                            encoder.token(setting.as_str());
                        }
                        ConversionControl::ImporterOption => encoder.token("importer_option"),
                    }
                }
                FactValue::Boolean(value) => {
                    encoder.token("boolean");
                    encoder.token(if *value { "true" } else { "false" });
                }
                FactValue::ImportHandling(value) => {
                    encoder.token("import_handling");
                    encoder.token(match value {
                        ImportHandling::Preserved => "preserved",
                        ImportHandling::Converted => "converted",
                        ImportHandling::Discarded => "discarded",
                        ImportHandling::Unsupported => "unsupported",
                    });
                }
                FactValue::TargetAddressability(value) => {
                    encoder.token("target_addressability");
                    encoder.token(match value {
                        TargetAddressability::NamePathDerivedId => "name_path_derived_id",
                    });
                }
                FactValue::RootMotionAddressability(value) => {
                    encoder.token("root_motion_addressability");
                    encoder.token(match value {
                        RootMotionAddressability::ExactSourceTransformPath => {
                            "exact_source_transform_path"
                        }
                        RootMotionAddressability::HumanoidAvatarBody => "humanoid_avatar_body",
                    });
                }
            }
        }
    }
}

fn encode_setting_value(encoder: &mut CanonicalEncoder, value: &SettingValue) {
    match value {
        SettingValue::Boolean(value) => {
            encoder.token("boolean");
            encoder.token(if *value { "true" } else { "false" });
        }
        SettingValue::BakeOrExtract(value) => {
            encoder.token("bake_or_extract");
            encoder.token(match value {
                BakeOrExtract::Bake => "bake",
                BakeOrExtract::Extract => "extract",
            });
        }
        SettingValue::SourceTransformPath(value) => {
            encoder.token("source_transform_path");
            encoder.token(value);
        }
    }
}

pub(crate) const fn format_name(format: SourceFormatV1) -> &'static str {
    match format {
        SourceFormatV1::GltfJson => "gltf_json",
        SourceFormatV1::Glb => "glb",
        SourceFormatV1::Fbx => "fbx",
    }
}

pub(crate) const fn scope_name(scope: SettingScope) -> &'static str {
    match scope {
        SettingScope::Document => "document",
        SettingScope::Clip => "clip",
    }
}

pub(crate) const fn domain_name(domain: crate::SettingDomain) -> &'static str {
    match domain {
        crate::SettingDomain::Boolean => "boolean",
        crate::SettingDomain::BakeOrExtract => "bake_or_extract",
        crate::SettingDomain::SourceTransformPath => "source_transform_path",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FactId, ProfileFact, SettingDescriptor};

    #[test]
    fn facts_encoding_sorts_every_repeated_stable_id() {
        for original in crate::profiles_v1() {
            let expected = facts_identity(original);
            let mut reordered = original.clone();
            let accepted = reordered
                .facts
                .iter_mut()
                .find(|fact| fact.id() == FactId::AcceptedInputs)
                .unwrap();
            if let FactState::Known(FactValue::AcceptedFormats(formats)) = accepted.state() {
                let mut formats = formats.clone();
                formats.reverse();
                *accepted = ProfileFact::new(
                    FactId::AcceptedInputs,
                    FactState::Known(FactValue::AcceptedFormats(formats)),
                );
            }
            reordered.facts.reverse();
            reordered.settings.reverse();
            reordered.sources.reverse();
            for source in &mut reordered.sources {
                source.reverse_supported_ids_for_test();
            }
            assert_eq!(
                facts_identity(&reordered),
                expected,
                "{:?}",
                original.selection()
            );
        }
    }

    #[test]
    fn facts_encoding_is_sensitive_to_each_record_domain() {
        let original = crate::profiles_v1()
            .iter()
            .find(|profile| profile.selection.family() == "unity-generic")
            .unwrap()
            .clone();
        let expected = facts_identity(&original);
        let changed = |profile: &EngineProfile| assert_ne!(facts_identity(profile), expected);

        let mut profile = original.clone();
        profile.selection = crate::ProfileSelection::new(
            "unity-generic-changed",
            1,
            "6000.3",
            "fbx-model-importer",
        );
        changed(&profile);

        let mut profile = original.clone();
        profile.selection =
            crate::ProfileSelection::new("unity-generic", 2, "6000.3", "fbx-model-importer");
        changed(&profile);

        let mut profile = original.clone();
        profile.selection =
            crate::ProfileSelection::new("unity-generic", 1, "6000.4", "fbx-model-importer");
        changed(&profile);

        let mut profile = original.clone();
        profile.selection =
            crate::ProfileSelection::new("unity-generic", 1, "6000.3", "other-importer");
        changed(&profile);

        let mut profile = original.clone();
        profile.fact_bundle_urn = "urn:animsmith:engine-profile:unity-generic:changed";
        changed(&profile);

        let mut profile = original.clone();
        let fact = profile
            .facts
            .iter_mut()
            .find(|fact| fact.id() == FactId::ExactAxisConversion)
            .unwrap();
        *fact = ProfileFact::new(FactId::ExactAxisConversion, FactState::NotApplicable);
        changed(&profile);

        let mut profile = original.clone();
        let fact = profile
            .facts
            .iter_mut()
            .find(|fact| fact.id() == FactId::AcceptedInputs)
            .unwrap();
        *fact = ProfileFact::new(
            FactId::AcceptedInputs,
            FactState::Known(FactValue::AcceptedFormats(vec![
                SourceFormatV1::Fbx,
                SourceFormatV1::Glb,
            ])),
        );
        changed(&profile);

        let mut profile = original.clone();
        let descriptor = profile.settings[0].clone();
        profile.settings[0] = SettingDescriptor::new(
            descriptor.id(),
            descriptor.scope(),
            crate::SettingDomain::BakeOrExtract,
            descriptor.applicability(),
            descriptor.default_status(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let descriptor = profile.settings[0].clone();
        profile.settings[0] = SettingDescriptor::new(
            descriptor.id(),
            crate::SettingScope::Clip,
            descriptor.domain(),
            descriptor.applicability(),
            descriptor.default_status(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let descriptor = profile.settings[0].clone();
        profile.settings[0] = SettingDescriptor::new(
            descriptor.id(),
            descriptor.scope(),
            descriptor.domain(),
            crate::SettingApplicability::NotApplicable,
            descriptor.default_status(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let descriptor = profile.settings[0].clone();
        profile.settings[0] = SettingDescriptor::new(
            descriptor.id(),
            descriptor.scope(),
            descriptor.domain(),
            descriptor.applicability(),
            crate::DefaultStatus::NotApplicable,
        );
        changed(&profile);

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        profile.sources[0] = crate::PrimarySource::new(
            "changed-source-id",
            source.target_version(),
            source.url(),
            source.verified_on(),
            source.supported_facts().to_vec(),
            source.supported_settings().to_vec(),
        );
        changed(&profile);

        let replace_source = |profile: &mut EngineProfile,
                              target_version,
                              url,
                              verified_on,
                              facts: Vec<_>,
                              settings: Vec<_>| {
            let source = profile.sources[0].clone();
            profile.sources[0] = crate::PrimarySource::new(
                source.id(),
                target_version,
                url,
                verified_on,
                facts,
                settings,
            );
        };

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        replace_source(
            &mut profile,
            "changed-version",
            source.url(),
            source.verified_on(),
            source.supported_facts().to_vec(),
            source.supported_settings().to_vec(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        replace_source(
            &mut profile,
            source.target_version(),
            "https://example.invalid/changed",
            source.verified_on(),
            source.supported_facts().to_vec(),
            source.supported_settings().to_vec(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        replace_source(
            &mut profile,
            source.target_version(),
            source.url(),
            "2099-01-01",
            source.supported_facts().to_vec(),
            source.supported_settings().to_vec(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        let mut facts = source.supported_facts().to_vec();
        facts.push(FactId::AcceptedInputs);
        replace_source(
            &mut profile,
            source.target_version(),
            source.url(),
            source.verified_on(),
            facts,
            source.supported_settings().to_vec(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        let mut settings = source.supported_settings().to_vec();
        settings.pop();
        replace_source(
            &mut profile,
            source.target_version(),
            source.url(),
            source.verified_on(),
            source.supported_facts().to_vec(),
            settings,
        );
        changed(&profile);

        let unreal = crate::profiles_v1()
            .iter()
            .find(|profile| profile.selection.family() == "unreal")
            .unwrap();
        let expected = facts_identity(unreal);
        let mut profile = unreal.clone();
        let fact = profile
            .facts
            .iter_mut()
            .find(|fact| fact.id() == FactId::TargetCoordinateBasis)
            .unwrap();
        *fact = ProfileFact::new(
            FactId::TargetCoordinateBasis,
            FactState::Known(FactValue::CoordinateBasis(crate::CoordinateBasis {
                handedness: crate::Handedness::Left,
                up_axis: crate::UpAxis::Z,
                forward_axis: crate::ForwardAxis::NegativeX,
            })),
        );
        assert_ne!(facts_identity(&profile), expected);
    }
}
