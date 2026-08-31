//! Strict bounded TOML reader for foot-cycle-parameterization V1.
//!
//! Core owns the format-neutral declaration and pure map planner. This module
//! owns only the TOML wire grammar; later collection-producer work will own
//! rooted path resolution, asset loading, transformation, and publication.

use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;

use animsmith_core::{
    COLLECTION_MANIFEST_V1_ID, COLLECTION_MANIFEST_V1_SCHEMA_VERSION, CollectionIdV1,
    CollectionLogicalIdV1, DependencyResourceKeyV1, FOOT_CYCLE_PARAMETERIZATION_V1_ID,
    FOOT_CYCLE_PARAMETERIZATION_V1_MAX_BYTES, FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS,
    FOOT_CYCLE_PARAMETERIZATION_V1_SCHEMA_VERSION, FootCycleManifestBindingV1,
    FootCycleParameterizationMemberV1, FootCycleParameterizationV1, FootCycleProofPolicyV1,
    InputIdentity, ResourceKeySyntaxV1,
};
use serde::Deserialize;
use serde::de::{Deserializer, SeqAccess, Visitor};

/// Stable category for a foot-cycle parameterization control failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FootCycleParameterizationControlKind {
    Read,
    TooLarge,
    Encoding,
    Malformed,
    UnsupportedSchema,
    UnsupportedSchemaVersion,
    InvalidDeclaration,
}

impl FootCycleParameterizationControlKind {
    fn label(self) -> &'static str {
        match self {
            Self::Read => "parameterization-read",
            Self::TooLarge => "parameterization-too-large",
            Self::Encoding => "parameterization-encoding",
            Self::Malformed => "parameterization-malformed",
            Self::UnsupportedSchema => "unsupported-schema",
            Self::UnsupportedSchemaVersion => "unsupported-schema-version",
            Self::InvalidDeclaration => "invalid-declaration",
        }
    }
}

/// One validated parameterization bound to the exact bytes read once.
pub(crate) struct LoadedFootCycleParameterization {
    pub(crate) parameterization: FootCycleParameterizationV1,
    pub(crate) input: InputIdentity,
}

/// Read and strictly validate one bounded parameterization document.
pub(crate) fn load_foot_cycle_parameterization_with_identity(
    path: &Path,
) -> Result<LoadedFootCycleParameterization, FootCycleParameterizationControlError> {
    let file = fs::File::open(path).map_err(|_| {
        FootCycleParameterizationControlError::new(FootCycleParameterizationControlKind::Read)
    })?;
    let mut bytes = Vec::new();
    file.take(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| {
            FootCycleParameterizationControlError::new(FootCycleParameterizationControlKind::Read)
        })?;
    let parameterization = parse_foot_cycle_parameterization_bytes(&bytes)?;
    Ok(LoadedFootCycleParameterization {
        parameterization,
        input: InputIdentity::from_bytes(&bytes),
    })
}

/// One closed control-plane parsing failure without input detail leakage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FootCycleParameterizationControlError {
    kind: FootCycleParameterizationControlKind,
}

impl FootCycleParameterizationControlError {
    fn new(kind: FootCycleParameterizationControlKind) -> Self {
        Self { kind }
    }

    #[cfg(test)]
    fn kind(self) -> FootCycleParameterizationControlKind {
        self.kind
    }
}

impl fmt::Display for FootCycleParameterizationControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "foot-cycle parameterization control error ({})",
            self.kind.label()
        )
    }
}

impl std::error::Error for FootCycleParameterizationControlError {}

/// Parse one complete bounded parameterization TOML byte sequence.
#[allow(
    dead_code,
    reason = "this seam freezes the reader before the collection producer"
)]
pub(crate) fn parse_foot_cycle_parameterization_bytes(
    bytes: &[u8],
) -> Result<FootCycleParameterizationV1, FootCycleParameterizationControlError> {
    if bytes.len() as u64 > FOOT_CYCLE_PARAMETERIZATION_V1_MAX_BYTES {
        return Err(FootCycleParameterizationControlError::new(
            FootCycleParameterizationControlKind::TooLarge,
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        FootCycleParameterizationControlError::new(FootCycleParameterizationControlKind::Encoding)
    })?;
    let header = toml::from_str::<HeaderWire>(text).map_err(|_| {
        FootCycleParameterizationControlError::new(FootCycleParameterizationControlKind::Malformed)
    })?;
    if header.schema != FOOT_CYCLE_PARAMETERIZATION_V1_ID {
        return Err(FootCycleParameterizationControlError::new(
            FootCycleParameterizationControlKind::UnsupportedSchema,
        ));
    }
    if header.schema_version != FOOT_CYCLE_PARAMETERIZATION_V1_SCHEMA_VERSION {
        return Err(FootCycleParameterizationControlError::new(
            FootCycleParameterizationControlKind::UnsupportedSchemaVersion,
        ));
    }
    let wire = toml::from_str::<ParameterizationWire>(text).map_err(|_| {
        FootCycleParameterizationControlError::new(FootCycleParameterizationControlKind::Malformed)
    })?;
    decode(wire).map_err(|_| {
        FootCycleParameterizationControlError::new(
            FootCycleParameterizationControlKind::InvalidDeclaration,
        )
    })
}

fn decode(wire: ParameterizationWire) -> Result<FootCycleParameterizationV1, ()> {
    if wire.schema != FOOT_CYCLE_PARAMETERIZATION_V1_ID
        || wire.schema_version != FOOT_CYCLE_PARAMETERIZATION_V1_SCHEMA_VERSION
        || wire.manifest.schema != COLLECTION_MANIFEST_V1_ID
        || wire.manifest.schema_version != COLLECTION_MANIFEST_V1_SCHEMA_VERSION
    {
        return Err(());
    }
    let manifest = FootCycleManifestBindingV1::new(
        CollectionIdV1::new(wire.manifest.collection_id).map_err(|_| ())?,
        InputIdentity::from_sha256_digest(
            decode_digest(&wire.manifest.input.sha256)?,
            wire.manifest.input.bytes,
        ),
    )
    .map_err(|_| ())?;
    let runtime_set_id = CollectionLogicalIdV1::new(wire.runtime_set_id).map_err(|_| ())?;
    let reference_member = CollectionLogicalIdV1::new(wire.reference_member).map_err(|_| ())?;
    let output_directory = safe_path(&wire.output_directory)?;
    let proof = FootCycleProofPolicyV1::new(
        wire.proof.max_gait_phase_spread,
        wire.proof.min_lr_amplitude_m,
        wire.proof.max_contact_boundary_phase_error,
    )
    .map_err(|_| ())?;
    let members = wire
        .members
        .into_iter()
        .map(|member| {
            Ok(FootCycleParameterizationMemberV1::new(
                CollectionLogicalIdV1::new(member.id).map_err(|_| ())?,
                safe_path(&member.contact_fragment)?,
            ))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    FootCycleParameterizationV1::new(
        manifest,
        runtime_set_id,
        reference_member,
        output_directory,
        wire.minimum_segment_slope,
        wire.maximum_segment_slope,
        proof,
        members,
    )
    .map_err(|_| ())
}

fn safe_path(value: &str) -> Result<DependencyResourceKeyV1, ()> {
    DependencyResourceKeyV1::from_source_str(value, ResourceKeySyntaxV1::ParserRelativePath)
        .map_err(|_| ())
}

fn decode_digest(value: &str) -> Result<[u8; 32], ()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(());
    }
    let mut digest = [0; 32];
    let bytes = value.as_bytes();
    for (index, offset) in (0..64).step_by(2).enumerate() {
        digest[index] = (hex(bytes[offset])? << 4) | hex(bytes[offset + 1])?;
    }
    Ok(digest)
}

fn hex(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(()),
    }
}

#[derive(Debug, Deserialize)]
struct HeaderWire {
    schema: String,
    schema_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParameterizationWire {
    schema: String,
    schema_version: u32,
    runtime_set_id: String,
    reference_member: String,
    output_directory: String,
    minimum_segment_slope: f64,
    maximum_segment_slope: f64,
    manifest: ManifestWire,
    proof: ProofWire,
    #[serde(deserialize_with = "deserialize_members")]
    members: Vec<MemberWire>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestWire {
    schema: String,
    schema_version: u32,
    collection_id: String,
    input: InputWire,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofWire {
    max_gait_phase_spread: f64,
    min_lr_amplitude_m: f64,
    max_contact_boundary_phase_error: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputWire {
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberWire {
    id: String,
    contact_fragment: String,
}

fn deserialize_members<'de, D>(deserializer: D) -> Result<Vec<MemberWire>, D::Error>
where
    D: Deserializer<'de>,
{
    struct MembersVisitor;

    impl<'de> Visitor<'de> for MembersVisitor {
        type Value = Vec<MemberWire>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded sequence of foot-cycle members")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut members = Vec::new();
            while let Some(member) = sequence.next_element()? {
                if members.len() == FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS {
                    return Err(serde::de::Error::custom(
                        "too many foot-cycle parameterization members",
                    ));
                }
                members.push(member);
            }
            Ok(members)
        }
    }

    deserializer.deserialize_seq(MembersVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn valid() -> String {
        format!(
            r#"schema = "{FOOT_CYCLE_PARAMETERIZATION_V1_ID}"
schema_version = 1
runtime_set_id = "com.example/sets/walk"
reference_member = "com.example/walk-forward"
output_directory = "generated/walk-aligned"
minimum_segment_slope = 0.5
maximum_segment_slope = 2.0

[proof]
max_gait_phase_spread = 0.08
min_lr_amplitude_m = 0.05
max_contact_boundary_phase_error = 0.01

[manifest]
schema = "{COLLECTION_MANIFEST_V1_ID}"
schema_version = 1
collection_id = "com.example"

[manifest.input]
sha256 = "{DIGEST}"
bytes = 1024

[[members]]
id = "com.example/walk-forward"
contact_fragment = "contacts/walk-forward.json"

[[members]]
id = "com.example/walk-right"
contact_fragment = "contacts/walk-right.json"
"#
        )
    }

    #[test]
    fn strict_reader_preserves_declared_order_and_bindings() {
        assert_eq!(
            FOOT_CYCLE_PARAMETERIZATION_V1_ID,
            "urn:animsmith:schema:foot-cycle-parameterization:1"
        );
        let parsed = parse_foot_cycle_parameterization_bytes(valid().as_bytes()).unwrap();
        assert_eq!(parsed.schema(), FOOT_CYCLE_PARAMETERIZATION_V1_ID);
        assert_eq!(parsed.manifest().input().bytes(), 1024);
        assert_eq!(parsed.runtime_set_id().as_str(), "com.example/sets/walk");
        assert_eq!(
            parsed.reference_member().as_str(),
            "com.example/walk-forward"
        );
        assert_eq!(parsed.output_directory().as_str(), "generated/walk-aligned");
        assert_eq!(parsed.proof().max_gait_phase_spread(), 0.08);
        assert_eq!(parsed.proof().min_lr_amplitude_m(), 0.05);
        assert_eq!(parsed.proof().max_contact_boundary_phase_error(), 0.01);
        assert_eq!(
            parsed
                .members()
                .iter()
                .map(|member| member.id().as_str())
                .collect::<Vec<_>>(),
            ["com.example/walk-forward", "com.example/walk-right"]
        );
        assert_eq!(
            parsed
                .members()
                .iter()
                .map(|member| member.contact_fragment().as_str())
                .collect::<Vec<_>>(),
            ["contacts/walk-forward.json", "contacts/walk-right.json"]
        );
    }

    #[test]
    fn reader_classifies_header_and_closed_shape_failures() {
        let unsupported = valid().replace(FOOT_CYCLE_PARAMETERIZATION_V1_ID, "urn:other:1");
        assert_eq!(
            parse_foot_cycle_parameterization_bytes(unsupported.as_bytes())
                .unwrap_err()
                .kind(),
            FootCycleParameterizationControlKind::UnsupportedSchema
        );
        let unsupported = valid().replacen("schema_version = 1", "schema_version = 2", 1);
        assert_eq!(
            parse_foot_cycle_parameterization_bytes(unsupported.as_bytes())
                .unwrap_err()
                .kind(),
            FootCycleParameterizationControlKind::UnsupportedSchemaVersion
        );
        for unsupported in [
            valid().replace(COLLECTION_MANIFEST_V1_ID, "urn:other:manifest:1"),
            valid().replace(
                &format!(
                    "[manifest]\nschema = \"{COLLECTION_MANIFEST_V1_ID}\"\nschema_version = 1"
                ),
                &format!(
                    "[manifest]\nschema = \"{COLLECTION_MANIFEST_V1_ID}\"\nschema_version = 2"
                ),
            ),
        ] {
            assert_eq!(
                parse_foot_cycle_parameterization_bytes(unsupported.as_bytes())
                    .unwrap_err()
                    .kind(),
                FootCycleParameterizationControlKind::InvalidDeclaration
            );
        }
        for unknown in [
            valid().replace(
                "maximum_segment_slope = 2.0",
                "maximum_segment_slope = 2.0\nunknown = true",
            ),
            valid().replace(
                "max_contact_boundary_phase_error = 0.01",
                "max_contact_boundary_phase_error = 0.01\nunknown = true",
            ),
            valid().replace(
                "collection_id = \"com.example\"",
                "collection_id = \"com.example\"\nunknown = true",
            ),
            valid().replace("bytes = 1024", "bytes = 1024\nunknown = true"),
            valid().replace(
                "contact_fragment = \"contacts/walk-right.json\"",
                "contact_fragment = \"contacts/walk-right.json\"\nunknown = true",
            ),
        ] {
            assert_eq!(
                parse_foot_cycle_parameterization_bytes(unknown.as_bytes())
                    .unwrap_err()
                    .kind(),
                FootCycleParameterizationControlKind::Malformed
            );
        }
    }

    #[test]
    fn reader_rejects_bad_encoding_unsafe_paths_and_bad_digest() {
        assert_eq!(
            parse_foot_cycle_parameterization_bytes(&[0xff])
                .unwrap_err()
                .kind(),
            FootCycleParameterizationControlKind::Encoding
        );
        for source in [
            valid().replace("contacts/walk-right.json", "../walk-right.json"),
            valid().replace("generated/walk-aligned", "../generated/walk-aligned"),
            valid().replace(DIGEST, &"A".repeat(64)),
        ] {
            assert_eq!(
                parse_foot_cycle_parameterization_bytes(source.as_bytes())
                    .unwrap_err()
                    .kind(),
                FootCycleParameterizationControlKind::InvalidDeclaration
            );
        }
    }

    #[test]
    fn proof_table_is_required_closed_and_range_checked() {
        let proof = "[proof]\nmax_gait_phase_spread = 0.08\nmin_lr_amplitude_m = 0.05\nmax_contact_boundary_phase_error = 0.01\n\n";
        for malformed in [
            valid().replace(proof, ""),
            valid().replace("max_gait_phase_spread = 0.08\n", ""),
            valid().replace("min_lr_amplitude_m = 0.05\n", ""),
            valid().replace("max_contact_boundary_phase_error = 0.01\n", ""),
        ] {
            assert_eq!(
                parse_foot_cycle_parameterization_bytes(malformed.as_bytes())
                    .unwrap_err()
                    .kind(),
                FootCycleParameterizationControlKind::Malformed
            );
        }

        for invalid in [
            valid().replace(
                "max_gait_phase_spread = 0.08",
                "max_gait_phase_spread = -0.01",
            ),
            valid().replace(
                "max_gait_phase_spread = 0.08",
                "max_gait_phase_spread = 0.5000000000000001",
            ),
            valid().replace(
                "min_lr_amplitude_m = 0.05",
                "min_lr_amplitude_m = -0.0000000000000001",
            ),
            valid().replace(
                "max_contact_boundary_phase_error = 0.01",
                "max_contact_boundary_phase_error = 0.5000000000000001",
            ),
            valid().replace(
                "max_gait_phase_spread = 0.08",
                "max_gait_phase_spread = nan",
            ),
            valid().replace("min_lr_amplitude_m = 0.05", "min_lr_amplitude_m = inf"),
            valid().replace(
                "max_contact_boundary_phase_error = 0.01",
                "max_contact_boundary_phase_error = -inf",
            ),
        ] {
            assert_eq!(
                parse_foot_cycle_parameterization_bytes(invalid.as_bytes())
                    .unwrap_err()
                    .kind(),
                FootCycleParameterizationControlKind::InvalidDeclaration
            );
        }

        let exact = valid()
            .replace(
                "max_gait_phase_spread = 0.08",
                "max_gait_phase_spread = 0.5",
            )
            .replace("min_lr_amplitude_m = 0.05", "min_lr_amplitude_m = 0.0")
            .replace(
                "max_contact_boundary_phase_error = 0.01",
                "max_contact_boundary_phase_error = 0.5",
            );
        let parsed = parse_foot_cycle_parameterization_bytes(exact.as_bytes()).unwrap();
        assert_eq!(parsed.proof().max_gait_phase_spread(), 0.5);
        assert_eq!(parsed.proof().min_lr_amplitude_m(), 0.0);
        assert_eq!(parsed.proof().max_contact_boundary_phase_error(), 0.5);
    }

    #[test]
    fn exact_parameterization_bytes_bind_the_proof_policy() {
        let directory = tempfile::tempdir().unwrap();
        let first_path = directory.path().join("first.toml");
        let second_path = directory.path().join("second.toml");
        let first = valid();
        let second = first.replace(
            "max_gait_phase_spread = 0.08",
            "max_gait_phase_spread = 0.09",
        );
        std::fs::write(&first_path, &first).unwrap();
        std::fs::write(&second_path, &second).unwrap();
        let first = load_foot_cycle_parameterization_with_identity(&first_path).unwrap();
        let second = load_foot_cycle_parameterization_with_identity(&second_path).unwrap();
        assert_eq!(first.parameterization.proof().max_gait_phase_spread(), 0.08);
        assert_eq!(
            second.parameterization.proof().max_gait_phase_spread(),
            0.09
        );
        assert_ne!(first.input, second.input);
    }

    #[test]
    fn reader_accepts_exact_byte_limit_and_rejects_n_plus_one() {
        assert_eq!(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_BYTES, 8 * 1024 * 1024);
        let mut exact = valid().into_bytes();
        exact.resize(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_BYTES as usize, b' ');
        assert!(parse_foot_cycle_parameterization_bytes(&exact).is_ok());
        exact.push(b' ');
        assert_eq!(
            parse_foot_cycle_parameterization_bytes(&exact)
                .unwrap_err()
                .kind(),
            FootCycleParameterizationControlKind::TooLarge
        );
    }

    #[test]
    fn member_deserializer_refuses_n_plus_one_without_retaining_it() {
        assert_eq!(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS, 4_096);
        let prefix = valid()
            .split("[[members]]")
            .next()
            .expect("prefix")
            .to_owned();
        let mut exact = prefix;
        for index in 0..FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS {
            let id = if index == 0 {
                "com.example/walk-forward".to_owned()
            } else {
                format!("com.example/member-{index}")
            };
            exact.push_str(&format!(
                "[[members]]\nid = \"{id}\"\ncontact_fragment = \"contacts/member-{index}.json\"\n"
            ));
        }
        assert!(parse_foot_cycle_parameterization_bytes(exact.as_bytes()).is_ok());

        let mut source = exact;
        let index = FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS;
        source.push_str(&format!(
            "[[members]]\nid = \"com.example/member-{index}\"\ncontact_fragment = \"contacts/member-{index}.json\"\n"
        ));
        assert_eq!(
            parse_foot_cycle_parameterization_bytes(source.as_bytes())
                .unwrap_err()
                .kind(),
            FootCycleParameterizationControlKind::Malformed
        );
    }
}
