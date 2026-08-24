//! Strict bounded TOML decoding for the directional-speed policy V1 contract.
//!
//! The core crate owns the typed, format-neutral vocabulary. This module owns
//! only the TOML wire shape and its bounded strict reader; it deliberately is
//! not connected to a CLI command or collection-output reader yet.

use animsmith_core::{
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_ID, COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_MEMBERS,
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_SCHEMA_VERSION, COLLECTION_MANIFEST_V1_ID,
    COLLECTION_MANIFEST_V1_SCHEMA_VERSION, CollectionDirectionalSpeedDiagonalBehaviorV1,
    CollectionDirectionalSpeedManifestIdentityV1, CollectionDirectionalSpeedMemberV1,
    CollectionDirectionalSpeedModeV1, CollectionDirectionalSpeedPolicyV1,
    CollectionDirectionalSpeedSourceBasisV1, CollectionIdV1, CollectionLogicalIdV1,
    CollectionRuntimeSetKindV1, InputIdentity,
};
use serde::Deserialize;
use serde::de::{Deserializer, SeqAccess, Visitor};
use std::fmt;

/// Bounded policy input size for the strict V1 reader.
pub(crate) const COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectionDirectionalSpeedPolicyControlKind {
    TooLarge,
    Encoding,
    Malformed,
    UnsupportedSchema,
    UnsupportedSchemaVersion,
    InvalidDeclaration,
    InvalidBinding,
}

impl CollectionDirectionalSpeedPolicyControlKind {
    fn label(self) -> &'static str {
        match self {
            Self::TooLarge => "policy-too-large",
            Self::Encoding => "policy-encoding",
            Self::Malformed => "policy-malformed",
            Self::UnsupportedSchema => "unsupported-schema",
            Self::UnsupportedSchemaVersion => "unsupported-schema-version",
            Self::InvalidDeclaration => "invalid-declaration",
            Self::InvalidBinding => "invalid-binding",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CollectionDirectionalSpeedPolicyControlError {
    kind: CollectionDirectionalSpeedPolicyControlKind,
}

impl CollectionDirectionalSpeedPolicyControlError {
    fn new(kind: CollectionDirectionalSpeedPolicyControlKind) -> Self {
        Self { kind }
    }

    #[cfg(test)]
    fn kind(self) -> CollectionDirectionalSpeedPolicyControlKind {
        self.kind
    }
}

impl fmt::Display for CollectionDirectionalSpeedPolicyControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "directional-speed policy control error ({})",
            self.kind.label()
        )
    }
}

impl std::error::Error for CollectionDirectionalSpeedPolicyControlError {}

/// Parse one complete, bounded directional-speed policy TOML byte sequence.
#[allow(
    dead_code,
    reason = "slice 1 freezes this reader before its later CLI command"
)]
pub(crate) fn parse_collection_directional_speed_policy_bytes(
    bytes: &[u8],
) -> Result<CollectionDirectionalSpeedPolicyV1, CollectionDirectionalSpeedPolicyControlError> {
    if bytes.len() as u64 > COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES {
        return Err(CollectionDirectionalSpeedPolicyControlError::new(
            CollectionDirectionalSpeedPolicyControlKind::TooLarge,
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        CollectionDirectionalSpeedPolicyControlError::new(
            CollectionDirectionalSpeedPolicyControlKind::Encoding,
        )
    })?;
    // Classify unsupported schema/version before the full strict decode.
    let header = toml::from_str::<PolicyHeaderWire>(text).map_err(|_| {
        CollectionDirectionalSpeedPolicyControlError::new(
            CollectionDirectionalSpeedPolicyControlKind::Malformed,
        )
    })?;
    if header.schema != COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_ID {
        return Err(CollectionDirectionalSpeedPolicyControlError::new(
            CollectionDirectionalSpeedPolicyControlKind::UnsupportedSchema,
        ));
    }
    if header.schema_version != COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_SCHEMA_VERSION {
        return Err(CollectionDirectionalSpeedPolicyControlError::new(
            CollectionDirectionalSpeedPolicyControlKind::UnsupportedSchemaVersion,
        ));
    }
    let wire = toml::from_str::<PolicyWire>(text).map_err(|_| {
        CollectionDirectionalSpeedPolicyControlError::new(
            CollectionDirectionalSpeedPolicyControlKind::Malformed,
        )
    })?;
    if wire.schema != COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_ID
        || wire.schema_version != COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_SCHEMA_VERSION
    {
        return Err(CollectionDirectionalSpeedPolicyControlError::new(
            CollectionDirectionalSpeedPolicyControlKind::Malformed,
        ));
    }
    decode_policy(wire).map_err(|_| {
        CollectionDirectionalSpeedPolicyControlError::new(
            CollectionDirectionalSpeedPolicyControlKind::InvalidDeclaration,
        )
    })
}

/// Validate policy binding against an already decoded manifest runtime set.
#[allow(
    dead_code,
    reason = "slice 1 freezes binding before later evaluator wiring"
)]
pub(crate) fn validate_collection_directional_speed_policy_binding(
    policy: &CollectionDirectionalSpeedPolicyV1,
    manifest: &CollectionDirectionalSpeedManifestIdentityV1,
    runtime_set_id: &CollectionLogicalIdV1,
    kind: CollectionRuntimeSetKindV1,
    members: &[CollectionLogicalIdV1],
) -> Result<(), CollectionDirectionalSpeedPolicyControlError> {
    policy
        .validate_binding(manifest, runtime_set_id, kind, members)
        .map_err(|_| {
            CollectionDirectionalSpeedPolicyControlError::new(
                CollectionDirectionalSpeedPolicyControlKind::InvalidBinding,
            )
        })
}

fn decode_policy(wire: PolicyWire) -> Result<CollectionDirectionalSpeedPolicyV1, ()> {
    let collection_id = CollectionIdV1::new(wire.manifest.collection_id).map_err(|_| ())?;
    let input = InputIdentity::from_sha256_digest(
        decode_digest(&wire.manifest.input.sha256)?,
        wire.manifest.input.bytes,
    );
    if wire.manifest.schema != COLLECTION_MANIFEST_V1_ID
        || wire.manifest.schema_version != COLLECTION_MANIFEST_V1_SCHEMA_VERSION
    {
        return Err(());
    }
    let manifest = CollectionDirectionalSpeedManifestIdentityV1::new(collection_id, input);
    let runtime_set_id = CollectionLogicalIdV1::new(wire.runtime_set_id).map_err(|_| ())?;
    let basis =
        CollectionDirectionalSpeedSourceBasisV1::new(wire.source_basis.x, wire.source_basis.z)
            .map_err(|_| ())?;
    let diagonal_behavior = match wire.diagonal_behavior.as_str() {
        "preserve" => CollectionDirectionalSpeedDiagonalBehaviorV1::Preserve,
        "normalize" => CollectionDirectionalSpeedDiagonalBehaviorV1::Normalize,
        _ => return Err(()),
    };
    let mode = match wire.mode.as_str() {
        "uniform" => {
            if wire.uniform_speed_mps.is_none()
                || wire.speed_tolerance_mps.is_none()
                || wire.reference_member.is_some()
                || wire.ratio_tolerance.is_some()
            {
                return Err(());
            }
            CollectionDirectionalSpeedModeV1::Uniform {
                speed_mps: wire.uniform_speed_mps.ok_or(())?,
                speed_tolerance_mps: wire.speed_tolerance_mps.ok_or(())?,
            }
        }
        "authored" => {
            if wire.uniform_speed_mps.is_some()
                || wire.speed_tolerance_mps.is_none()
                || wire.reference_member.is_some()
                || wire.ratio_tolerance.is_some()
            {
                return Err(());
            }
            CollectionDirectionalSpeedModeV1::Authored {
                speed_tolerance_mps: wire.speed_tolerance_mps.ok_or(())?,
            }
        }
        "ratios" => {
            if wire.uniform_speed_mps.is_some()
                || wire.speed_tolerance_mps.is_some()
                || wire.reference_member.is_none()
                || wire.ratio_tolerance.is_none()
            {
                return Err(());
            }
            CollectionDirectionalSpeedModeV1::Ratios {
                reference_member: CollectionLogicalIdV1::new(wire.reference_member.ok_or(())?)
                    .map_err(|_| ())?,
                ratio_tolerance: wire.ratio_tolerance.ok_or(())?,
            }
        }
        _ => return Err(()),
    };
    let members = wire
        .members
        .into_iter()
        .map(|member| {
            Ok(CollectionDirectionalSpeedMemberV1::new(
                CollectionLogicalIdV1::new(member.id).map_err(|_| ())?,
                member.coordinate,
                member.speed_mps,
                member.expected_ratio,
            ))
        })
        .collect::<Result<Vec<_>, ()>>()?;
    CollectionDirectionalSpeedPolicyV1::new(
        manifest,
        runtime_set_id,
        basis,
        diagonal_behavior,
        wire.direction_tolerance_deg,
        mode,
        members,
    )
    .map_err(|_| ())
}

fn decode_digest(value: &str) -> Result<[u8; 32], ()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(());
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks(2).enumerate() {
        digest[index] = (hex(pair[0])? << 4) | hex(pair[1])?;
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
struct PolicyHeaderWire {
    schema: String,
    schema_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyWire {
    schema: String,
    schema_version: u32,
    manifest: ManifestWire,
    runtime_set_id: String,
    source_basis: BasisWire,
    diagonal_behavior: String,
    direction_tolerance_deg: f64,
    mode: String,
    #[serde(default)]
    uniform_speed_mps: Option<f64>,
    #[serde(default)]
    speed_tolerance_mps: Option<f64>,
    #[serde(default)]
    reference_member: Option<String>,
    #[serde(default)]
    ratio_tolerance: Option<f64>,
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
struct InputWire {
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BasisWire {
    x: [f64; 2],
    z: [f64; 2],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberWire {
    id: String,
    coordinate: [f64; 2],
    #[serde(default)]
    speed_mps: Option<f64>,
    #[serde(default)]
    expected_ratio: Option<f64>,
}

fn deserialize_members<'de, D>(deserializer: D) -> Result<Vec<MemberWire>, D::Error>
where
    D: Deserializer<'de>,
{
    struct MembersVisitor;
    impl<'de> Visitor<'de> for MembersVisitor {
        type Value = Vec<MemberWire>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded sequence of directional policy members")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut members = Vec::new();
            while let Some(member) = sequence.next_element()? {
                if members.len() >= COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_MEMBERS {
                    return Err(serde::de::Error::custom(
                        "too many directional policy members",
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
    use animsmith_core::{
        COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_DIRECTION_TOLERANCE_DEG, CollectionIdV1,
        CollectionRuntimeSetKindV1, InputIdentity,
    };

    const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn policy(mode: &str, fields: &str) -> String {
        format!(
            r#"schema = "{COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_ID}"
schema_version = 1
runtime_set_id = "com.example/directional"
diagonal_behavior = "normalize"
direction_tolerance_deg = 2.0
mode = "{mode}"
{fields}
speed_tolerance_mps = 0.1

[manifest]
schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example"
[manifest.input]
sha256 = "{DIGEST}"
bytes = 7

[source_basis]
x = [1.0, 0.0]
z = [0.0, 1.0]

[[members]]
id = "com.example/left"
coordinate = [-1.0, 0.0]
{fields_left}

[[members]]
id = "com.example/right"
coordinate = [1.0, 0.0]
{fields_right}
"#,
            fields_left = if mode == "authored" {
                "speed_mps = 1.0"
            } else if mode == "ratios" {
                "expected_ratio = 1.0"
            } else {
                ""
            },
            fields_right = if mode == "authored" {
                "speed_mps = 1.1"
            } else if mode == "ratios" {
                "expected_ratio = 1.1"
            } else {
                ""
            },
        )
    }

    #[test]
    fn parses_each_closed_mode_and_preserves_order() {
        let uniform = parse_collection_directional_speed_policy_bytes(
            policy("uniform", "uniform_speed_mps = 1.0").as_bytes(),
        )
        .unwrap();
        let authored =
            parse_collection_directional_speed_policy_bytes(policy("authored", "").as_bytes())
                .unwrap();
        let ratios = parse_collection_directional_speed_policy_bytes(
            policy(
                "ratios",
                "reference_member = \"com.example/left\"\nratio_tolerance = 0.1",
            )
            .replace("speed_tolerance_mps = 0.1\n", "")
            .as_bytes(),
        )
        .unwrap();
        assert_eq!(uniform.members()[0].id().as_str(), "com.example/left");
        assert!(authored.members()[1].speed_mps().is_some());
        assert!(ratios.members()[1].expected_ratio().is_some());
    }

    #[test]
    fn rejects_unknown_duplicate_missing_and_invalid_wire_values() {
        let valid = policy("uniform", "uniform_speed_mps = 1.0");
        for bad in [
            format!("{valid}\nunknown = true\n"),
            valid.replacen(
                "uniform_speed_mps = 1.0",
                "uniform_speed_mps = 1.0\nuniform_speed_mps = 2.0",
                1,
            ),
            valid.replace("mode = \"uniform\"", "mode = \"unknown\""),
            valid.replace(
                "diagonal_behavior = \"normalize\"",
                "diagonal_behavior = \"unknown\"",
            ),
            valid.replace("x = [1.0, 0.0]", "x = [0.0, 0.0]"),
            valid.replace("uniform_speed_mps = 1.0", "uniform_speed_mps = 1000001.0"),
            valid.replace(
                "direction_tolerance_deg = 2.0",
                "direction_tolerance_deg = nan",
            ),
            valid.replace(
                "direction_tolerance_deg = 2.0",
                "direction_tolerance_deg = 180.00000000000003",
            ),
            valid.replace("x = [1.0, 0.0]", "x = [1000001.0, 0.0]"),
            valid.replace("coordinate = [-1.0, 0.0]", "coordinate = [nan, 0.0]"),
            valid.replace("speed_tolerance_mps = 0.1", "speed_tolerance_mps = inf"),
            valid.replace(
                "speed_tolerance_mps = 0.1",
                "speed_tolerance_mps = 0.1\nratio_tolerance = 0.1",
            ),
            valid.replace("id = \"com.example/right\"", "id = \"com.example/left\""),
            valid.replace("id = \"com.example/right\"", "id = \"invalid\""),
            valid.replace("coordinate = [1.0, 0.0]", "coordinate = [-1.0, 0.0]"),
            valid.replace(
                &format!("sha256 = \"{DIGEST}\""),
                "sha256 = \"not-a-digest\"",
            ),
            valid.replace(
                "schema = \"urn:animsmith:schema:collection-manifest:1\"",
                "schema = \"urn:animsmith:schema:other:1\"",
            ),
            valid.replace(
                "[manifest]\nschema = \"urn:animsmith:schema:collection-manifest:1\"\nschema_version = 1",
                "[manifest]\nschema = \"urn:animsmith:schema:collection-manifest:1\"\nschema_version = 2",
            ),
            valid.replace("bytes = 7", "bytes = -1"),
            valid.replace("bytes = 7", "bytes = 7\n[manifest.input]\nextra = true"),
        ] {
            assert!(parse_collection_directional_speed_policy_bytes(bad.as_bytes()).is_err());
        }
    }

    #[test]
    fn rejects_mode_forbidden_fields_and_out_of_range_scalars() {
        let uniform = policy("uniform", "uniform_speed_mps = 1.0");
        let authored = policy("authored", "");
        let ratios = policy(
            "ratios",
            "reference_member = \"com.example/left\"\nratio_tolerance = 0.1",
        )
        .replace("speed_tolerance_mps = 0.1\n", "");

        let cases = [
            // Uniform mode forbids all member-level authored fields.
            uniform.replace(
                "coordinate = [-1.0, 0.0]",
                "coordinate = [-1.0, 0.0]\nspeed_mps = 1.0",
            ),
            uniform.replace(
                "coordinate = [-1.0, 0.0]",
                "coordinate = [-1.0, 0.0]\nexpected_ratio = 1.0",
            ),
            // Authored mode forbids ratios and the uniform speed field.
            authored.replace("speed_mps = 1.1", "speed_mps = 1.1\nexpected_ratio = 1.1"),
            authored.replace(
                "speed_tolerance_mps = 0.1",
                "speed_tolerance_mps = 0.1\nuniform_speed_mps = 1.0",
            ),
            // Ratios mode forbids authored speeds and the uniform speed field.
            ratios.replace(
                "coordinate = [-1.0, 0.0]",
                "coordinate = [-1.0, 0.0]\nspeed_mps = 1.0",
            ),
            ratios.replace(
                "ratio_tolerance = 0.1",
                "ratio_tolerance = 0.1\nuniform_speed_mps = 1.0",
            ),
            // Every mode-specific scalar has a bounded negative case.
            uniform.replace("uniform_speed_mps = 1.0", "uniform_speed_mps = 1000001.0"),
            authored.replace("speed_mps = 1.1", "speed_mps = 1000001.0"),
            uniform.replace("uniform_speed_mps = 1.0", "uniform_speed_mps = -0.1"),
            authored.replace("speed_mps = 1.1", "speed_mps = -0.1"),
            uniform.replace(
                "speed_tolerance_mps = 0.1",
                "speed_tolerance_mps = 1000001.0",
            ),
            authored.replace(
                "speed_tolerance_mps = 0.1",
                "speed_tolerance_mps = 1000001.0",
            ),
            uniform.replace("speed_tolerance_mps = 0.1", "speed_tolerance_mps = -0.1"),
            authored.replace("speed_tolerance_mps = 0.1", "speed_tolerance_mps = -0.1"),
            ratios.replace("ratio_tolerance = 0.1", "ratio_tolerance = 1000001.0"),
            ratios.replace("expected_ratio = 1.1", "expected_ratio = 1000001.0"),
            ratios.replace("ratio_tolerance = 0.1", "ratio_tolerance = -0.1"),
            ratios.replace("expected_ratio = 1.1", "expected_ratio = -0.1"),
        ];
        for case in cases {
            assert!(
                parse_collection_directional_speed_policy_bytes(case.as_bytes()).is_err(),
                "invalid policy unexpectedly parsed:\n{case}"
            );
        }
    }

    #[test]
    fn direction_tolerance_bounds_are_inclusive_in_the_toml_reader() {
        let zero = parse_collection_directional_speed_policy_bytes(
            policy("uniform", "uniform_speed_mps = 1.0")
                .replace(
                    "direction_tolerance_deg = 2.0",
                    "direction_tolerance_deg = 0.0",
                )
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(zero.direction_tolerance_deg(), 0.0);

        let maximum = parse_collection_directional_speed_policy_bytes(
            policy("uniform", "uniform_speed_mps = 1.0")
                .replace(
                    "direction_tolerance_deg = 2.0",
                    &format!(
                        "direction_tolerance_deg = {COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_DIRECTION_TOLERANCE_DEG}"
                    ),
                )
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(maximum.direction_tolerance_deg(), 180.0);

        let above = policy("uniform", "uniform_speed_mps = 1.0").replace(
            "direction_tolerance_deg = 2.0",
            "direction_tolerance_deg = 180.00000000000003",
        );
        assert!(parse_collection_directional_speed_policy_bytes(above.as_bytes()).is_err());
    }

    #[test]
    fn mode_fields_and_reference_member_form_a_strict_behavior_matrix() {
        let authored = policy("authored", "");
        assert!(
            parse_collection_directional_speed_policy_bytes(
                authored.replace("speed_mps = 1.1", "").as_bytes()
            )
            .is_err()
        );
        assert!(
            parse_collection_directional_speed_policy_bytes(
                authored
                    .replace(
                        "speed_tolerance_mps = 0.1",
                        "speed_tolerance_mps = 0.1\nratio_tolerance = 0.1"
                    )
                    .as_bytes()
            )
            .is_err()
        );

        let ratios = policy(
            "ratios",
            "reference_member = \"com.example/left\"\nratio_tolerance = 0.1",
        )
        .replace("speed_tolerance_mps = 0.1\n", "");
        assert!(
            parse_collection_directional_speed_policy_bytes(
                ratios.replace("expected_ratio = 1.1", "").as_bytes()
            )
            .is_err()
        );
        assert!(
            parse_collection_directional_speed_policy_bytes(
                ratios
                    .replace(
                        "ratio_tolerance = 0.1",
                        "ratio_tolerance = 0.1\nspeed_tolerance_mps = 0.1"
                    )
                    .as_bytes()
            )
            .is_err()
        );
        assert!(
            parse_collection_directional_speed_policy_bytes(
                ratios
                    .replace(
                        "reference_member = \"com.example/left\"",
                        "reference_member = \"com.example/missing\""
                    )
                    .as_bytes()
            )
            .is_err()
        );
        assert!(
            parse_collection_directional_speed_policy_bytes(
                ratios
                    .replace("expected_ratio = 1.0", "expected_ratio = 0.9")
                    .as_bytes()
            )
            .is_err()
        );
        assert!(
            parse_collection_directional_speed_policy_bytes(
                ratios
                    .replace("reference_member = \"com.example/left\"\n", "")
                    .as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn header_classification_precedes_full_decode() {
        let valid = policy("uniform", "uniform_speed_mps = 1.0");
        let unsupported_schema = valid.replace(
            &format!("schema = \"{COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_ID}\""),
            "schema = \"urn:animsmith:schema:other:1\"",
        );
        assert_eq!(
            parse_collection_directional_speed_policy_bytes(unsupported_schema.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionDirectionalSpeedPolicyControlKind::UnsupportedSchema
        );
        let unsupported_version = valid.replace("schema_version = 1", "schema_version = 2");
        assert_eq!(
            parse_collection_directional_speed_policy_bytes(unsupported_version.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionDirectionalSpeedPolicyControlKind::UnsupportedSchemaVersion
        );
    }

    #[test]
    fn bounded_reader_classifies_oversize_and_invalid_utf8_before_toml() {
        let oversized = vec![0_u8; COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES as usize + 1];
        assert_eq!(
            parse_collection_directional_speed_policy_bytes(&oversized)
                .unwrap_err()
                .kind(),
            CollectionDirectionalSpeedPolicyControlKind::TooLarge
        );
        assert_eq!(
            parse_collection_directional_speed_policy_bytes(&[0xff, 0xfe])
                .unwrap_err()
                .kind(),
            CollectionDirectionalSpeedPolicyControlKind::Encoding
        );
    }

    #[test]
    fn bounded_reader_accepts_exact_maximum_input_bytes_with_toml_padding() {
        let mut exact = policy("uniform", "uniform_speed_mps = 1.0");
        exact.push_str("\n#");
        while exact.len() < COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES as usize {
            exact.push('x');
        }
        assert_eq!(
            exact.len(),
            COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES as usize
        );
        assert!(parse_collection_directional_speed_policy_bytes(exact.as_bytes()).is_ok());
    }

    #[test]
    fn binds_exact_manifest_and_directional_members() {
        let parsed = parse_collection_directional_speed_policy_bytes(
            policy("uniform", "uniform_speed_mps = 1.0").as_bytes(),
        )
        .unwrap();
        let manifest = parsed.manifest().clone();
        let ids = parsed
            .members()
            .iter()
            .map(|member| member.id().clone())
            .collect::<Vec<_>>();
        assert!(
            validate_collection_directional_speed_policy_binding(
                &parsed,
                &manifest,
                parsed.runtime_set_id(),
                CollectionRuntimeSetKindV1::DirectionalBlend,
                &ids
            )
            .is_ok()
        );
        let stale_manifest = CollectionDirectionalSpeedManifestIdentityV1::new(
            CollectionIdV1::new("com.other").unwrap(),
            InputIdentity::from_bytes(b"manifest"),
        );
        let other_set = CollectionLogicalIdV1::new("com.example/other").unwrap();
        let extra = CollectionLogicalIdV1::new("com.example/extra").unwrap();
        for (candidate_manifest, candidate_set, candidate_kind, candidate_members) in [
            (
                stale_manifest,
                parsed.runtime_set_id().clone(),
                CollectionRuntimeSetKindV1::DirectionalBlend,
                ids.clone(),
            ),
            (
                manifest.clone(),
                other_set,
                CollectionRuntimeSetKindV1::DirectionalBlend,
                ids.clone(),
            ),
            (
                manifest.clone(),
                parsed.runtime_set_id().clone(),
                CollectionRuntimeSetKindV1::GaitGroup,
                ids.clone(),
            ),
            (
                manifest.clone(),
                parsed.runtime_set_id().clone(),
                CollectionRuntimeSetKindV1::DirectionalBlend,
                ids[..1].to_vec(),
            ),
            (
                manifest.clone(),
                parsed.runtime_set_id().clone(),
                CollectionRuntimeSetKindV1::DirectionalBlend,
                vec![ids[0].clone(), ids[1].clone(), extra.clone()],
            ),
            (
                manifest.clone(),
                parsed.runtime_set_id().clone(),
                CollectionRuntimeSetKindV1::DirectionalBlend,
                vec![ids[1].clone(), ids[0].clone()],
            ),
        ] {
            assert_eq!(
                validate_collection_directional_speed_policy_binding(
                    &parsed,
                    &candidate_manifest,
                    &candidate_set,
                    candidate_kind,
                    &candidate_members
                )
                .unwrap_err()
                .kind(),
                CollectionDirectionalSpeedPolicyControlKind::InvalidBinding
            );
        }
    }

    #[test]
    fn accepts_member_limit_and_rejects_n_plus_one_before_retention() {
        let mut exact = policy("uniform", "uniform_speed_mps = 1.0");
        for index in 2..COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_MEMBERS {
            exact.push_str(&format!(
                "\n[[members]]\nid = \"com.example/member-{index}\"\ncoordinate = [{index}.0, 1.0]\n"
            ));
        }
        assert_eq!(
            parse_collection_directional_speed_policy_bytes(exact.as_bytes())
                .unwrap()
                .members()
                .len(),
            COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_MEMBERS
        );
        exact.push_str(&format!(
            "\n[[members]]\nid = \"com.example/member-{}\"\ncoordinate = [{0}.0, 1.0]\n",
            COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_MEMBERS
        ));
        assert!(parse_collection_directional_speed_policy_bytes(exact.as_bytes()).is_err());
    }
}
