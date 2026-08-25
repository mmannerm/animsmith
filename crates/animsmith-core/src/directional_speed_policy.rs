//! Format-neutral directional-speed policy V1 values.
//!
//! The CLI owns bounded TOML decoding. This module owns the immutable policy
//! vocabulary and its finite, mode-specific, manifest-binding invariants.
//! The evaluator compares normalized raw collection-output V3 endpoint
//! displacement for heading, uses the published `speed_mps` field for speed
//! magnitude (not travel distance), and binds both policy and evidence by
//! their raw [`InputIdentity`] values. A zero net displacement is typed
//! complete/not-evaluated rather than a false speed finding. An unrepresentable
//! ratio comparison is likewise a typed numeric-range/not-evaluated outcome.

use serde::{Deserialize, Serialize};

use crate::{CollectionIdV1, CollectionLogicalIdV1, CollectionRuntimeSetKindV1, InputIdentity};

/// Schema identity for a directional-speed policy declaration.
pub const COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_ID: &str =
    "urn:animsmith:schema:collection-directional-speed-policy:1";
/// Schema version for a directional-speed policy declaration.
pub const COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_SCHEMA_VERSION: u32 = 1;
/// Maximum raw directional-speed policy TOML byte identity.
pub const COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum raw collection-output JSON byte identity consumed by evaluation.
pub const COLLECTION_DIRECTIONAL_SPEED_EVIDENCE_V1_MAX_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum policy members retained by the V1 reader.
pub const COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_MEMBERS: usize = 4_096;
/// Maximum absolute source-basis or semantic-coordinate component.
pub const COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_COMPONENT: f64 = 1_000_000.0;
/// Maximum finite speed, ratio, or tolerance value.
pub const COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_SCALAR: f64 = 1_000_000.0;
/// Maximum angular direction tolerance, in degrees, accepted by V1.
pub const COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_DIRECTION_TOLERANCE_DEG: f64 = 180.0;
/// Maximum absolute cosine allowed between the declared source axes.
pub const COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_AXIS_COSINE: f64 = 1e-9;

/// Closed diagonal-input handling declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CollectionDirectionalSpeedDiagonalBehaviorV1 {
    /// Preserve the authored diagonal magnitude. The declared speed fields
    /// remain unit-input/base targets; a coordinate `c` contributes gain
    /// `g(c) = hypot(c)` to the later speed expectation.
    Preserve,
    /// Normalize diagonal input magnitude before policy comparison. The
    /// later speed gain for a coordinate is `g(c) = 1`.
    Normalize,
}

/// Explicit source X/Z orientation witnesses in semantic 2-D policy
/// coordinates.
///
/// `x` and `z` witness the raw collection-output V3 +X/+Z endpoint
/// displacement directions. Their magnitudes are nonsemantic; the
/// evaluator uses unit axes for heading comparison.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CollectionDirectionalSpeedSourceBasisV1 {
    x: [f64; 2],
    z: [f64; 2],
}

impl CollectionDirectionalSpeedSourceBasisV1 {
    /// Construct a finite, bounded, nonzero, perpendicular X/Z orientation
    /// witness for raw collection-output V3 endpoint displacement.
    pub fn new(x: [f64; 2], z: [f64; 2]) -> Result<Self, CollectionDirectionalSpeedPolicyError> {
        for component in x.into_iter().chain(z) {
            if !bounded_component(component) {
                return Err(CollectionDirectionalSpeedPolicyError::InvalidNumber {
                    field: "source_basis",
                });
            }
        }
        let x_norm = x[0].hypot(x[1]);
        let z_norm = z[0].hypot(z[1]);
        if x_norm == 0.0 || z_norm == 0.0 {
            return Err(CollectionDirectionalSpeedPolicyError::InvalidBasis);
        }
        let normalized_dot = (x[0] / x_norm) * (z[0] / z_norm) + (x[1] / x_norm) * (z[1] / z_norm);
        if normalized_dot.abs() > COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_AXIS_COSINE {
            return Err(CollectionDirectionalSpeedPolicyError::InvalidBasis);
        }
        Ok(Self { x, z })
    }

    /// Semantic coordinates of the source X axis.
    pub const fn x(self) -> [f64; 2] {
        self.x
    }

    /// Semantic coordinates of the source Z axis.
    pub const fn z(self) -> [f64; 2] {
        self.z
    }
}

/// Exact manifest identity carried by a directional-speed policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollectionDirectionalSpeedManifestIdentityV1 {
    collection_id: CollectionIdV1,
    input: InputIdentity,
}

impl CollectionDirectionalSpeedManifestIdentityV1 {
    /// Construct an identity from the exact collection id and manifest bytes.
    pub fn new(
        collection_id: CollectionIdV1,
        input: InputIdentity,
    ) -> Result<Self, CollectionDirectionalSpeedPolicyError> {
        if input.bytes() > crate::COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES {
            return Err(CollectionDirectionalSpeedPolicyError::ManifestTooLarge);
        }
        Ok(Self {
            collection_id,
            input,
        })
    }

    /// Collection namespace token.
    pub fn collection_id(&self) -> &CollectionIdV1 {
        &self.collection_id
    }

    /// Exact manifest-byte identity.
    pub const fn input(&self) -> &InputIdentity {
        &self.input
    }
}

/// One ordered semantic coordinate and its mode-specific authored value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectionDirectionalSpeedMemberV1 {
    id: CollectionLogicalIdV1,
    coordinate: [f64; 2],
    speed_mps: Option<f64>,
    expected_ratio: Option<f64>,
}

impl CollectionDirectionalSpeedMemberV1 {
    /// Construct one member declaration. Mode-specific values are validated by
    /// [`CollectionDirectionalSpeedPolicyV1::new`].
    pub fn new(
        id: CollectionLogicalIdV1,
        coordinate: [f64; 2],
        speed_mps: Option<f64>,
        expected_ratio: Option<f64>,
    ) -> Self {
        Self {
            id,
            coordinate,
            speed_mps,
            expected_ratio,
        }
    }

    /// Logical member id.
    pub fn id(&self) -> &CollectionLogicalIdV1 {
        &self.id
    }

    /// Semantic 2-D coordinate.
    pub const fn coordinate(&self) -> [f64; 2] {
        self.coordinate
    }

    /// Authored member speed, present only in authored mode.
    pub const fn speed_mps(&self) -> Option<f64> {
        self.speed_mps
    }

    /// Expected reference ratio, present only in ratios mode.
    pub const fn expected_ratio(&self) -> Option<f64> {
        self.expected_ratio
    }
}

/// Closed speed-policy mode and its explicit mode-level fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum CollectionDirectionalSpeedModeV1 {
    /// Every member is compared with one declared unit-input/base speed.
    Uniform {
        /// Unit-input/base expected speed in metres per second.
        speed_mps: f64,
        /// Allowed speed deviation in metres per second.
        speed_tolerance_mps: f64,
    },
    /// Every member carries one declared authored unit-input/base speed.
    Authored {
        /// Allowed speed deviation in metres per second.
        speed_tolerance_mps: f64,
    },
    /// Every member carries one declared unit-input/base ratio to a reference
    /// member. The later derived target is
    /// `expected_measured_ratio_i_to_ref = declared_expected_ratio_i *
    /// g(c_i) / g(c_ref)`; diagonal gains do not affect direction.
    Ratios {
        /// Member whose measured speed is the ratio denominator.
        reference_member: CollectionLogicalIdV1,
        /// Allowed dimensionless ratio deviation.
        ratio_tolerance: f64,
    },
}

/// Fully validated directional-speed policy V1.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectionDirectionalSpeedPolicyV1 {
    schema: &'static str,
    schema_version: u32,
    manifest: CollectionDirectionalSpeedManifestIdentityV1,
    runtime_set_id: CollectionLogicalIdV1,
    source_basis: CollectionDirectionalSpeedSourceBasisV1,
    diagonal_behavior: CollectionDirectionalSpeedDiagonalBehaviorV1,
    direction_tolerance_deg: f64,
    mode: CollectionDirectionalSpeedModeV1,
    members: Vec<CollectionDirectionalSpeedMemberV1>,
}

impl CollectionDirectionalSpeedPolicyV1 {
    /// Construct and validate one policy in declared member order.
    pub fn new(
        manifest: CollectionDirectionalSpeedManifestIdentityV1,
        runtime_set_id: CollectionLogicalIdV1,
        source_basis: CollectionDirectionalSpeedSourceBasisV1,
        diagonal_behavior: CollectionDirectionalSpeedDiagonalBehaviorV1,
        direction_tolerance_deg: f64,
        mode: CollectionDirectionalSpeedModeV1,
        members: Vec<CollectionDirectionalSpeedMemberV1>,
    ) -> Result<Self, CollectionDirectionalSpeedPolicyError> {
        if members.len() < 2 {
            return Err(CollectionDirectionalSpeedPolicyError::TooFewMembers {
                found: members.len(),
            });
        }
        if members.len() > COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_MEMBERS {
            return Err(CollectionDirectionalSpeedPolicyError::TooManyMembers {
                found: members.len(),
                max: COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_MEMBERS,
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut seen_coordinates = std::collections::HashSet::new();
        for member in &members {
            if !seen.insert(member.id.clone()) {
                return Err(CollectionDirectionalSpeedPolicyError::DuplicateMember {
                    value: member.id.as_str().to_owned(),
                });
            }
            if member
                .coordinate
                .iter()
                .any(|value| !bounded_component(*value))
                || member.coordinate[0] == 0.0 && member.coordinate[1] == 0.0
            {
                return Err(CollectionDirectionalSpeedPolicyError::InvalidCoordinate {
                    member: member.id.as_str().to_owned(),
                });
            }
            let coordinate_key = member.coordinate.map(canonical_coordinate_bits);
            if !seen_coordinates.insert(coordinate_key) {
                return Err(CollectionDirectionalSpeedPolicyError::DuplicateCoordinate {
                    member: member.id.as_str().to_owned(),
                });
            }
        }
        if !bounded_direction_tolerance(direction_tolerance_deg) {
            return Err(CollectionDirectionalSpeedPolicyError::InvalidNumber {
                field: "direction_tolerance_deg",
            });
        }
        match &mode {
            CollectionDirectionalSpeedModeV1::Uniform {
                speed_mps,
                speed_tolerance_mps,
            } => {
                if !bounded_scalar(*speed_mps)
                    || !bounded_scalar(*speed_tolerance_mps)
                    || members
                        .iter()
                        .any(|member| member.speed_mps.is_some() || member.expected_ratio.is_some())
                {
                    return Err(CollectionDirectionalSpeedPolicyError::InvalidModeFields);
                }
            }
            CollectionDirectionalSpeedModeV1::Authored {
                speed_tolerance_mps,
            } => {
                if !bounded_scalar(*speed_tolerance_mps)
                    || members.iter().any(|member| {
                        member.speed_mps.is_none()
                            || member.expected_ratio.is_some()
                            || !member.speed_mps.is_some_and(bounded_scalar)
                    })
                {
                    return Err(CollectionDirectionalSpeedPolicyError::InvalidModeFields);
                }
            }
            CollectionDirectionalSpeedModeV1::Ratios {
                reference_member,
                ratio_tolerance,
            } => {
                if !bounded_scalar(*ratio_tolerance)
                    || !members.iter().any(|member| {
                        member.id == *reference_member && member.expected_ratio == Some(1.0)
                    })
                    || members.iter().any(|member| {
                        member.expected_ratio.is_none()
                            || member.speed_mps.is_some()
                            || !member.expected_ratio.is_some_and(bounded_scalar)
                    })
                {
                    return Err(CollectionDirectionalSpeedPolicyError::InvalidModeFields);
                }
            }
        }
        Ok(Self {
            schema: COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_ID,
            schema_version: COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_SCHEMA_VERSION,
            manifest,
            runtime_set_id,
            source_basis,
            diagonal_behavior,
            direction_tolerance_deg,
            mode,
            members,
        })
    }

    /// Maximum angular deviation, in degrees, accepted by a later evaluator.
    pub const fn direction_tolerance_deg(&self) -> f64 {
        self.direction_tolerance_deg
    }

    /// Immutable policy schema identity.
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    /// Immutable policy schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Exact manifest identity bound by this policy.
    pub fn manifest(&self) -> &CollectionDirectionalSpeedManifestIdentityV1 {
        &self.manifest
    }

    /// Declared directional-blend runtime-set id.
    pub fn runtime_set_id(&self) -> &CollectionLogicalIdV1 {
        &self.runtime_set_id
    }

    /// Explicit source X/Z basis.
    pub const fn source_basis(&self) -> CollectionDirectionalSpeedSourceBasisV1 {
        self.source_basis
    }

    /// Explicit diagonal handling.
    pub const fn diagonal_behavior(&self) -> CollectionDirectionalSpeedDiagonalBehaviorV1 {
        self.diagonal_behavior
    }

    /// Closed speed mode.
    pub fn mode(&self) -> &CollectionDirectionalSpeedModeV1 {
        &self.mode
    }

    /// Members in declared policy order.
    pub fn members(&self) -> &[CollectionDirectionalSpeedMemberV1] {
        &self.members
    }

    /// Bind this policy to one exact manifest identity and directional set.
    pub fn validate_binding(
        &self,
        manifest: &CollectionDirectionalSpeedManifestIdentityV1,
        runtime_set_id: &CollectionLogicalIdV1,
        kind: CollectionRuntimeSetKindV1,
        members: &[CollectionLogicalIdV1],
    ) -> Result<(), CollectionDirectionalSpeedPolicyError> {
        if self.manifest != *manifest {
            return Err(CollectionDirectionalSpeedPolicyError::ManifestMismatch);
        }
        if kind != CollectionRuntimeSetKindV1::DirectionalBlend {
            return Err(CollectionDirectionalSpeedPolicyError::WrongRuntimeSetKind);
        }
        if self.runtime_set_id != *runtime_set_id {
            return Err(CollectionDirectionalSpeedPolicyError::RuntimeSetMismatch);
        }
        if self
            .members
            .iter()
            .map(|member| member.id.clone())
            .collect::<Vec<_>>()
            != members
        {
            return Err(CollectionDirectionalSpeedPolicyError::MemberOrderMismatch);
        }
        Ok(())
    }
}

/// A directional-speed policy was malformed or violated a frozen V1 bound.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CollectionDirectionalSpeedPolicyError {
    /// A scalar or component was non-finite or outside its V1 bound.
    #[error("invalid finite bounded number in {field}")]
    InvalidNumber {
        /// Stable field name.
        field: &'static str,
    },
    /// The source X/Z basis was zero or not perpendicular.
    #[error("source X/Z basis must be nonzero and perpendicular")]
    InvalidBasis,
    /// Fewer than two directional members were declared.
    #[error("directional policy needs at least two members, found {found}")]
    TooFewMembers {
        /// Number supplied.
        found: usize,
    },
    /// The policy exceeded its member bound.
    #[error("directional policy has {found} members, exceeding V1 limit {max}")]
    TooManyMembers {
        /// Number supplied.
        found: usize,
        /// V1 maximum.
        max: usize,
    },
    /// A member id was repeated.
    #[error("duplicate policy member {value:?}")]
    DuplicateMember {
        /// Repeated id.
        value: String,
    },
    /// A member coordinate was zero, non-finite, or out of range.
    #[error("invalid coordinate for policy member {member:?}")]
    InvalidCoordinate {
        /// Affected member id.
        member: String,
    },
    /// Two members used one exact semantic coordinate.
    #[error("duplicate semantic coordinate for policy member {member:?}")]
    DuplicateCoordinate {
        /// Affected member id.
        member: String,
    },
    /// Mode-specific fields were missing, extra, or invalid.
    #[error("invalid mode-specific policy fields")]
    InvalidModeFields,
    /// The policy did not carry the exact manifest identity.
    #[error("policy manifest identity does not match evidence manifest")]
    ManifestMismatch,
    /// The manifest byte identity exceeds the V1 bounded reader limit.
    #[error("manifest identity exceeds the V1 byte limit")]
    ManifestTooLarge,
    /// The referenced runtime set was not directional-blend.
    #[error("policy runtime set is not directional-blend")]
    WrongRuntimeSetKind,
    /// The policy runtime-set id differed from the evidence set id.
    #[error("policy runtime-set id does not match evidence")]
    RuntimeSetMismatch,
    /// Policy member order or membership differed from the evidence set.
    #[error("policy members do not exactly preserve evidence member order")]
    MemberOrderMismatch,
}

fn bounded_component(value: f64) -> bool {
    value.is_finite() && value.abs() <= COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_COMPONENT
}

fn bounded_scalar(value: f64) -> bool {
    value.is_finite() && (0.0..=COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_SCALAR).contains(&value)
}

fn bounded_direction_tolerance(value: f64) -> bool {
    value.is_finite()
        && (0.0..=COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_DIRECTION_TOLERANCE_DEG)
            .contains(&value)
}

fn canonical_coordinate_bits(value: f64) -> u64 {
    if value == 0.0 { 0 } else { value.to_bits() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(mode: CollectionDirectionalSpeedModeV1) -> CollectionDirectionalSpeedPolicyV1 {
        let collection_id = CollectionIdV1::new("com.example").unwrap();
        let members = vec![
            CollectionDirectionalSpeedMemberV1::new(
                CollectionLogicalIdV1::new("com.example/left").unwrap(),
                [-1.0, 0.0],
                match &mode {
                    CollectionDirectionalSpeedModeV1::Authored { .. } => Some(1.0),
                    _ => None,
                },
                match &mode {
                    CollectionDirectionalSpeedModeV1::Ratios { .. } => Some(1.0),
                    _ => None,
                },
            ),
            CollectionDirectionalSpeedMemberV1::new(
                CollectionLogicalIdV1::new("com.example/right").unwrap(),
                [1.0, 0.0],
                match &mode {
                    CollectionDirectionalSpeedModeV1::Authored { .. } => Some(1.0),
                    _ => None,
                },
                match &mode {
                    CollectionDirectionalSpeedModeV1::Ratios { .. } => Some(1.0),
                    _ => None,
                },
            ),
        ];
        CollectionDirectionalSpeedPolicyV1::new(
            CollectionDirectionalSpeedManifestIdentityV1::new(
                collection_id,
                InputIdentity::from_bytes(b"manifest"),
            )
            .unwrap(),
            CollectionLogicalIdV1::new("com.example/directional").unwrap(),
            CollectionDirectionalSpeedSourceBasisV1::new([1.0, 0.0], [0.0, 1.0]).unwrap(),
            CollectionDirectionalSpeedDiagonalBehaviorV1::Normalize,
            1.0,
            mode,
            members,
        )
        .unwrap()
    }

    #[test]
    fn all_closed_modes_validate_and_retain_typed_values() {
        let uniform = fixture(CollectionDirectionalSpeedModeV1::Uniform {
            speed_mps: 1.0,
            speed_tolerance_mps: 0.1,
        });
        let authored = fixture(CollectionDirectionalSpeedModeV1::Authored {
            speed_tolerance_mps: 0.1,
        });
        let ratios = fixture(CollectionDirectionalSpeedModeV1::Ratios {
            reference_member: CollectionLogicalIdV1::new("com.example/left").unwrap(),
            ratio_tolerance: 0.1,
        });
        assert_eq!(uniform.members().len(), 2);
        assert_eq!(authored.members()[0].speed_mps(), Some(1.0));
        assert_eq!(ratios.members()[1].expected_ratio(), Some(1.0));
        assert_eq!(uniform.schema(), COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_ID);
    }

    #[test]
    fn direction_tolerance_uses_the_distinct_inclusive_degree_bound() {
        let valid = fixture(CollectionDirectionalSpeedModeV1::Uniform {
            speed_mps: 1.0,
            speed_tolerance_mps: 0.1,
        });
        let rebuild = |direction_tolerance_deg| {
            CollectionDirectionalSpeedPolicyV1::new(
                valid.manifest.clone(),
                valid.runtime_set_id.clone(),
                valid.source_basis,
                valid.diagonal_behavior,
                direction_tolerance_deg,
                valid.mode.clone(),
                valid.members.clone(),
            )
        };
        assert_eq!(rebuild(0.0).unwrap().direction_tolerance_deg(), 0.0);
        assert_eq!(
            rebuild(COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_DIRECTION_TOLERANCE_DEG)
                .unwrap()
                .direction_tolerance_deg(),
            180.0
        );
        assert!(matches!(
            rebuild(180.00000000000003),
            Err(CollectionDirectionalSpeedPolicyError::InvalidNumber {
                field: "direction_tolerance_deg"
            })
        ));
    }

    #[test]
    fn basis_perpendicularity_is_scale_independent_and_accepts_near_threshold() {
        let tiny = CollectionDirectionalSpeedSourceBasisV1::new([1e-200, 0.0], [0.0, 1e-200]);
        assert!(tiny.is_ok());
        let diagonal = CollectionDirectionalSpeedSourceBasisV1::new([1e-6, 0.0], [1e-6, 1e-6]);
        assert!(matches!(
            diagonal,
            Err(CollectionDirectionalSpeedPolicyError::InvalidBasis)
        ));
        let near_parallel =
            CollectionDirectionalSpeedSourceBasisV1::new([1e-6, 0.0], [1e-6, 1e-12]);
        assert!(matches!(
            near_parallel,
            Err(CollectionDirectionalSpeedPolicyError::InvalidBasis)
        ));
        let below = COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_AXIS_COSINE * 0.5;
        let above = COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_AXIS_COSINE * 2.0;
        assert!(
            CollectionDirectionalSpeedSourceBasisV1::new(
                [1.0, 0.0],
                [below, (1.0 - below * below).sqrt()]
            )
            .is_ok()
        );
        assert!(matches!(
            CollectionDirectionalSpeedSourceBasisV1::new(
                [1.0, 0.0],
                [above, (1.0 - above * above).sqrt()]
            ),
            Err(CollectionDirectionalSpeedPolicyError::InvalidBasis)
        ));
    }

    #[test]
    fn ratios_require_a_unit_reference_ratio() {
        let manifest = CollectionDirectionalSpeedManifestIdentityV1::new(
            CollectionIdV1::new("com.example").unwrap(),
            InputIdentity::from_bytes(b"manifest"),
        )
        .unwrap();
        let members = vec![
            CollectionDirectionalSpeedMemberV1::new(
                CollectionLogicalIdV1::new("com.example/left").unwrap(),
                [-1.0, 0.0],
                None,
                Some(0.9),
            ),
            CollectionDirectionalSpeedMemberV1::new(
                CollectionLogicalIdV1::new("com.example/right").unwrap(),
                [1.0, 0.0],
                None,
                Some(1.1),
            ),
        ];
        assert!(matches!(
            CollectionDirectionalSpeedPolicyV1::new(
                manifest,
                CollectionLogicalIdV1::new("com.example/directional").unwrap(),
                CollectionDirectionalSpeedSourceBasisV1::new([1.0, 0.0], [0.0, 1.0]).unwrap(),
                CollectionDirectionalSpeedDiagonalBehaviorV1::Normalize,
                1.0,
                CollectionDirectionalSpeedModeV1::Ratios {
                    reference_member: CollectionLogicalIdV1::new("com.example/left").unwrap(),
                    ratio_tolerance: 0.1,
                },
                members,
            ),
            Err(CollectionDirectionalSpeedPolicyError::InvalidModeFields)
        ));
    }

    #[test]
    fn rejects_duplicate_coordinates_and_invalid_basis() {
        assert!(matches!(
            CollectionDirectionalSpeedSourceBasisV1::new([1.0, 0.0], [1.0, 0.0]),
            Err(CollectionDirectionalSpeedPolicyError::InvalidBasis)
        ));
        assert!(matches!(
            CollectionDirectionalSpeedSourceBasisV1::new([f64::NAN, 0.0], [0.0, 1.0]),
            Err(CollectionDirectionalSpeedPolicyError::InvalidNumber { .. })
        ));
        let mut policy = fixture(CollectionDirectionalSpeedModeV1::Uniform {
            speed_mps: 1.0,
            speed_tolerance_mps: 0.1,
        });
        policy.members[1].coordinate = policy.members[0].coordinate;
        assert!(matches!(
            CollectionDirectionalSpeedPolicyV1::new(
                policy.manifest,
                policy.runtime_set_id,
                policy.source_basis,
                policy.diagonal_behavior,
                1.0,
                policy.mode,
                policy.members,
            ),
            Err(CollectionDirectionalSpeedPolicyError::DuplicateCoordinate { .. })
        ));
        let mut signed_zero = fixture(CollectionDirectionalSpeedModeV1::Uniform {
            speed_mps: 1.0,
            speed_tolerance_mps: 0.1,
        });
        signed_zero.members[0].coordinate = [1.0, -0.0];
        signed_zero.members[1].coordinate = [1.0, 0.0];
        assert!(matches!(
            CollectionDirectionalSpeedPolicyV1::new(
                signed_zero.manifest,
                signed_zero.runtime_set_id,
                signed_zero.source_basis,
                signed_zero.diagonal_behavior,
                1.0,
                signed_zero.mode,
                signed_zero.members,
            ),
            Err(CollectionDirectionalSpeedPolicyError::DuplicateCoordinate { .. })
        ));
    }

    #[test]
    fn binding_requires_exact_identity_kind_id_and_member_order() {
        let policy = fixture(CollectionDirectionalSpeedModeV1::Uniform {
            speed_mps: 1.0,
            speed_tolerance_mps: 0.1,
        });
        let manifest = policy.manifest.clone();
        let set = policy.runtime_set_id.clone();
        let members = policy
            .members
            .iter()
            .map(|member| member.id.clone())
            .collect::<Vec<_>>();
        assert!(
            policy
                .validate_binding(
                    &manifest,
                    &set,
                    CollectionRuntimeSetKindV1::DirectionalBlend,
                    &members,
                )
                .is_ok()
        );
        assert!(matches!(
            policy.validate_binding(
                &manifest,
                &set,
                CollectionRuntimeSetKindV1::GaitGroup,
                &members
            ),
            Err(CollectionDirectionalSpeedPolicyError::WrongRuntimeSetKind)
        ));
        let stale_manifest = CollectionDirectionalSpeedManifestIdentityV1::new(
            CollectionIdV1::new("com.other").unwrap(),
            InputIdentity::from_bytes(b"manifest"),
        )
        .unwrap();
        assert!(matches!(
            policy.validate_binding(
                &stale_manifest,
                &set,
                CollectionRuntimeSetKindV1::DirectionalBlend,
                &members
            ),
            Err(CollectionDirectionalSpeedPolicyError::ManifestMismatch)
        ));
        let other_set = CollectionLogicalIdV1::new("com.example/other").unwrap();
        assert!(matches!(
            policy.validate_binding(
                &manifest,
                &other_set,
                CollectionRuntimeSetKindV1::DirectionalBlend,
                &members
            ),
            Err(CollectionDirectionalSpeedPolicyError::RuntimeSetMismatch)
        ));
        assert!(matches!(
            policy.validate_binding(
                &manifest,
                &set,
                CollectionRuntimeSetKindV1::DirectionalBlend,
                &members[1..]
            ),
            Err(CollectionDirectionalSpeedPolicyError::MemberOrderMismatch)
        ));
    }

    #[test]
    fn manifest_identity_enforces_exact_v1_byte_limit() {
        let collection = CollectionIdV1::new("com.example.collection").unwrap();
        assert!(
            CollectionDirectionalSpeedManifestIdentityV1::new(
                collection.clone(),
                InputIdentity::from_sha256_digest(
                    [0; 32],
                    crate::COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES
                )
            )
            .is_ok()
        );
        assert_eq!(
            CollectionDirectionalSpeedManifestIdentityV1::new(
                collection,
                InputIdentity::from_sha256_digest(
                    [0; 32],
                    crate::COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES + 1
                )
            ),
            Err(CollectionDirectionalSpeedPolicyError::ManifestTooLarge)
        );
    }
}
