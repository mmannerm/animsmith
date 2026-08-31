//! Strict contact-fragment transform result V1.
//!
//! This module implements the format-neutral Appendix F.10 mapping and result
//! contract. It transforms only contact facts; asset mutation, dependency
//! capture, filesystem publication, and engine policy remain frontend work.

use std::collections::BTreeSet;
use std::io::{self, Write};

use serde::de::{Error as _, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::contact_fragment::StrictJsonValue;
use crate::{
    CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER, ContactEventKindV1, ContactEventV1, ContactEventWindowV1,
    ContactFragmentError, ContactFragmentV1, ContactProducerV1, DependencyClosureIdentityV1,
    InputIdentity,
};

/// Immutable contact-transform-result V1 schema identity.
pub const CONTACT_TRANSFORM_RESULT_V1_ID: &str = "urn:animsmith:schema:contact-transform-result:1";
/// Immutable contact-transform-result V1 schema version.
pub const CONTACT_TRANSFORM_RESULT_V1_SCHEMA_VERSION: u32 = 1;
/// Maximum accepted UTF-8 result bytes.
pub const CONTACT_TRANSFORM_RESULT_V1_MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum canonical RFC 8785 result bytes.
pub const CONTACT_TRANSFORM_RESULT_V1_MAX_CANONICAL_BYTES: usize = 16 * 1024 * 1024;
/// Maximum time-warp control points.
pub const CONTACT_TRANSFORM_RESULT_V1_MAX_CONTROL_POINTS: usize = 4_096;
/// Maximum event outcomes.
pub const CONTACT_TRANSFORM_RESULT_V1_MAX_EVENT_OUTCOMES: usize = 4_096;
/// Maximum UTF-8 bytes in a display refusal message.
pub const CONTACT_TRANSFORM_RESULT_V1_MAX_MESSAGE_BYTES: usize = 4_096;

/// A strict result could not be decoded, represented, or verified.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ContactTransformError {
    /// Input exceeded the immutable source-byte cap.
    #[error("contact transform result source has {bytes} bytes, exceeding V1 limit {limit}")]
    SourceTooLarge {
        /// Observed bytes.
        bytes: usize,
        /// Frozen limit.
        limit: usize,
    },
    /// JSON or a strict wire invariant was invalid.
    #[error("invalid contact transform result: {message}")]
    Invalid {
        /// Decoder or verification detail.
        message: String,
    },
    /// A row cap was exceeded.
    #[error("contact transform result {field} has {found} rows, exceeding V1 limit {limit}")]
    LimitExceeded {
        /// Stable field.
        field: &'static str,
        /// Observed rows, including the N+1 witness.
        found: usize,
        /// Frozen limit.
        limit: usize,
    },
    /// Canonical output exceeded its immutable cap.
    #[error("contact transform result canonical JSON exceeds V1 limit {limit}")]
    CanonicalTooLarge {
        /// Frozen limit.
        limit: usize,
    },
    /// The embedded contact fragment was invalid.
    #[error("invalid transformed contact fragment: {0}")]
    Fragment(#[from] ContactFragmentError),
}

/// One normalized retained interval for trim or slice.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactTransformIntervalV1 {
    start: f64,
    end: f64,
}

impl ContactTransformIntervalV1 {
    /// Construct a structurally representable interval.
    ///
    /// Domain and ordering are deliberately checked by operation validation so
    /// a known operation can produce a typed `invalid_mapping` refusal.
    pub const fn new(start: f64, end: f64) -> Self {
        Self { start, end }
    }
    /// Declared normalized start.
    pub const fn start(self) -> f64 {
        self.start
    }
    /// Declared normalized end.
    pub const fn end(self) -> f64 {
        self.end
    }
}

/// One source-to-output normalized time-warp knot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactTimeWarpControlPointV1 {
    input_time: f64,
    output_time: f64,
}

impl ContactTimeWarpControlPointV1 {
    /// Construct one structurally representable control point.
    pub const fn new(input_time: f64, output_time: f64) -> Self {
        Self {
            input_time,
            output_time,
        }
    }
    /// Source normalized time.
    pub const fn input_time(self) -> f64 {
        self.input_time
    }
    /// Output normalized time.
    pub const fn output_time(self) -> f64 {
        self.output_time
    }
}

/// The closed V1 contact operation vocabulary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContactTransformOperationV1 {
    /// Retain and normalize one interval.
    Trim {
        /// Operation version.
        version: u32,
        /// Retained normalized interval.
        interval: ContactTransformIntervalV1,
    },
    /// Retain and normalize one interval with slice semantics.
    Slice {
        /// Operation version.
        version: u32,
        /// Retained normalized interval.
        interval: ContactTransformIntervalV1,
    },
    /// Identity normalized-time mapping.
    Resample {
        /// Operation version.
        version: u32,
        /// Must be `identity`.
        mapping: String,
    },
    /// Strictly monotone piecewise-linear source-to-output mapping.
    TimeWarp {
        /// Operation version.
        version: u32,
        /// Exact output clip duration.
        output_duration_s: f64,
        /// Ordered normalized control points.
        #[serde(deserialize_with = "deserialize_control_points")]
        control_points: Vec<ContactTimeWarpControlPointV1>,
    },
}

impl ContactTransformOperationV1 {
    /// Construct a V1 trim request.
    pub const fn trim(interval: ContactTransformIntervalV1) -> Self {
        Self::Trim {
            version: 1,
            interval,
        }
    }
    /// Construct a V1 slice request.
    pub const fn slice(interval: ContactTransformIntervalV1) -> Self {
        Self::Slice {
            version: 1,
            interval,
        }
    }
    /// Construct a V1 identity resample request.
    pub fn resample() -> Self {
        Self::Resample {
            version: 1,
            mapping: "identity".into(),
        }
    }
    /// Construct a V1 time-warp request.
    ///
    /// Mapping validity is evaluated into a typed result rather than rejected
    /// by this structural constructor.
    pub fn time_warp(
        output_duration_s: f64,
        control_points: Vec<ContactTimeWarpControlPointV1>,
    ) -> Self {
        Self::TimeWarp {
            version: 1,
            output_duration_s,
            control_points,
        }
    }
    /// Ordered time-warp knots, or `None` for another operation.
    pub fn control_points(&self) -> Option<&[ContactTimeWarpControlPointV1]> {
        match self {
            Self::TimeWarp { control_points, .. } => Some(control_points),
            _ => None,
        }
    }
    /// Exact output duration declared by time-warp.
    pub const fn output_duration_s(&self) -> Option<f64> {
        match self {
            Self::TimeWarp {
                output_duration_s, ..
            } => Some(*output_duration_s),
            _ => None,
        }
    }
}

/// Exact three-way input binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactTransformBindingV1 {
    artifact: InputIdentity,
    dependency_closure_identity: DependencyClosureIdentityV1,
    fragment: InputIdentity,
}

impl ContactTransformBindingV1 {
    /// Construct one exact input binding.
    pub fn new(
        artifact: InputIdentity,
        dependency_closure_identity: DependencyClosureIdentityV1,
        fragment: InputIdentity,
    ) -> Self {
        Self {
            artifact,
            dependency_closure_identity,
            fragment,
        }
    }
    /// Source artifact identity.
    pub const fn artifact(&self) -> &InputIdentity {
        &self.artifact
    }
    /// Source dependency-closure identity.
    pub const fn dependency_closure_identity(&self) -> &DependencyClosureIdentityV1 {
        &self.dependency_closure_identity
    }
    /// Canonical source fragment identity.
    pub const fn fragment(&self) -> &InputIdentity {
        &self.fragment
    }
}

/// Same-operation input/output identity and extension-support context.
#[derive(Debug, Clone)]
pub struct ContactTransformContextV1 {
    current_input_artifact: InputIdentity,
    current_input_dependency_closure_identity: DependencyClosureIdentityV1,
    output_artifact: InputIdentity,
    output_dependency_closure_identity: DependencyClosureIdentityV1,
    output_producer: ContactProducerV1,
    supported_extensions: BTreeSet<(String, u32)>,
}

impl ContactTransformContextV1 {
    /// Construct the exact context captured by one artifact operation.
    pub fn new(
        current_input_artifact: InputIdentity,
        current_input_dependency_closure_identity: DependencyClosureIdentityV1,
        output_artifact: InputIdentity,
        output_dependency_closure_identity: DependencyClosureIdentityV1,
        output_producer: ContactProducerV1,
        supported_extensions: BTreeSet<(String, u32)>,
    ) -> Self {
        Self {
            current_input_artifact,
            current_input_dependency_closure_identity,
            output_artifact,
            output_dependency_closure_identity,
            output_producer,
            supported_extensions,
        }
    }
}

/// Exact transformed point or window value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ContactTransformedValueV1 {
    /// Mapped point.
    Point {
        /// Normalized output time.
        time: f64,
    },
    /// Mapped inclusive window.
    Window {
        /// Inclusive normalized output window.
        window: ContactEventWindowV1,
    },
}

/// One input event's operation outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContactEventOutcomeV1 {
    /// Event was mapped and retained.
    Transformed {
        /// Stable input event id.
        event_id: String,
        /// Exact mapped value.
        value: ContactTransformedValueV1,
    },
    /// Event lay wholly outside a trim/slice interval.
    Outside {
        /// Stable input event id.
        event_id: String,
    },
    /// Event made the whole operation refuse.
    Refused {
        /// Stable input event id.
        event_id: String,
        /// Stable refusal code.
        code: ContactTransformRefusalCodeV1,
    },
}

impl ContactEventOutcomeV1 {
    /// Stable input event id.
    pub fn event_id(&self) -> &str {
        match self {
            Self::Transformed { event_id, .. }
            | Self::Outside { event_id }
            | Self::Refused { event_id, .. } => event_id,
        }
    }
}

/// Closed V1 refusal vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactTransformRefusalCodeV1 {
    /// A window crossed a retained interval boundary.
    PartialWindow,
    /// A known operation's numeric domain or ordering was invalid.
    InvalidMapping,
    /// Input identities did not match the supplied fragment.
    InvalidBinding,
    /// Finite inputs produced an invalid derived value.
    InvalidValue,
    /// An extension has no explicit operation-specific support.
    UnsupportedExtension,
}

/// Stable refusal record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactTransformRefusalV1 {
    code: ContactTransformRefusalCodeV1,
    message: String,
}

impl ContactTransformRefusalV1 {
    /// Stable refusal code.
    pub const fn code(&self) -> ContactTransformRefusalCodeV1 {
        self.code
    }
    /// Display-only detail.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Successful output binding plus the complete transformed fragment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContactTransformOutputV1 {
    artifact: InputIdentity,
    dependency_closure_identity: DependencyClosureIdentityV1,
    fragment: InputIdentity,
    contact_fragment: ContactFragmentV1,
}

impl ContactTransformOutputV1 {
    /// Output artifact identity.
    pub const fn artifact(&self) -> &InputIdentity {
        &self.artifact
    }
    /// Fresh output dependency-closure identity.
    pub const fn dependency_closure_identity(&self) -> &DependencyClosureIdentityV1 {
        &self.dependency_closure_identity
    }
    /// Canonical output-fragment identity.
    pub const fn fragment(&self) -> &InputIdentity {
        &self.fragment
    }
    /// Complete transformed fragment.
    pub const fn contact_fragment(&self) -> &ContactFragmentV1 {
        &self.contact_fragment
    }
}

/// Strict V1 transform result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContactTransformResultV1 {
    schema: &'static str,
    schema_version: u32,
    operation: ContactTransformOperationV1,
    input: ContactTransformBindingV1,
    outcome: ContactTransformOutcomeV1,
    event_outcomes: Vec<ContactEventOutcomeV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<ContactTransformOutputV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refusal: Option<ContactTransformRefusalV1>,
}

/// Global transform outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactTransformOutcomeV1 {
    /// Complete transformed output exists.
    Transformed,
    /// No output exists and a refusal is present.
    Refused,
}

impl ContactTransformResultV1 {
    /// Strictly decode and verify one bounded result against its separately
    /// supplied input fragment.
    pub fn read_json(
        bytes: &[u8],
        input_fragment: &ContactFragmentV1,
    ) -> Result<Self, ContactTransformError> {
        if bytes.len() > CONTACT_TRANSFORM_RESULT_V1_MAX_SOURCE_BYTES {
            return Err(ContactTransformError::SourceTooLarge {
                bytes: bytes.len(),
                limit: CONTACT_TRANSFORM_RESULT_V1_MAX_SOURCE_BYTES,
            });
        }
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);
        let wire = ResultWire::deserialize(&mut deserializer).map_err(invalid_decode)?;
        deserializer.end().map_err(invalid_decode)?;
        let result = wire.into_result()?;
        result.verify_structure(input_fragment)?;
        result.verify_semantics(input_fragment)?;
        Ok(result)
    }

    /// Canonical RFC 8785 result bytes.
    pub fn canonical_json(&self) -> Result<Vec<u8>, ContactTransformError> {
        let mut writer = CappedWriter {
            bytes: Vec::new(),
            overflowed: false,
        };
        serde_jcs::to_writer(&mut writer, self).map_err(|error| {
            if writer.overflowed {
                ContactTransformError::CanonicalTooLarge {
                    limit: CONTACT_TRANSFORM_RESULT_V1_MAX_CANONICAL_BYTES,
                }
            } else {
                ContactTransformError::Invalid {
                    message: error.to_string(),
                }
            }
        })?;
        Ok(writer.bytes)
    }

    /// Operation echoed exactly in the result.
    pub const fn operation(&self) -> &ContactTransformOperationV1 {
        &self.operation
    }
    /// Exact source bindings.
    pub const fn input(&self) -> &ContactTransformBindingV1 {
        &self.input
    }
    /// Global outcome.
    pub const fn outcome(&self) -> ContactTransformOutcomeV1 {
        self.outcome
    }
    /// Outcomes in canonical input event order.
    pub fn event_outcomes(&self) -> &[ContactEventOutcomeV1] {
        &self.event_outcomes
    }
    /// Successful output, when transformed.
    pub const fn output(&self) -> Option<&ContactTransformOutputV1> {
        self.output.as_ref()
    }
    /// Refusal, when refused.
    pub const fn refusal(&self) -> Option<&ContactTransformRefusalV1> {
        self.refusal.as_ref()
    }

    fn verify_structure(
        &self,
        input_fragment: &ContactFragmentV1,
    ) -> Result<(), ContactTransformError> {
        if self.schema != CONTACT_TRANSFORM_RESULT_V1_ID
            || self.schema_version != CONTACT_TRANSFORM_RESULT_V1_SCHEMA_VERSION
        {
            return invalid("schema identity/version mismatch");
        }
        validate_operation_structure(&self.operation)?;
        let input_fragment_identity = input_fragment.canonical_identity()?;
        validate_identity(&self.input.artifact)?;
        validate_identity(self.input.dependency_closure_identity.input_identity())?;
        validate_identity(&self.input.fragment)?;
        let binding_matches = self.input.artifact == *input_fragment.artifact()
            && self.input.dependency_closure_identity
                == *input_fragment.dependency_closure_identity()
            && self.input.fragment == input_fragment_identity;
        if self.event_outcomes.len() > CONTACT_TRANSFORM_RESULT_V1_MAX_EVENT_OUTCOMES {
            return limit(
                "event_outcomes",
                self.event_outcomes.len(),
                CONTACT_TRANSFORM_RESULT_V1_MAX_EVENT_OUTCOMES,
            );
        }
        match self.outcome {
            ContactTransformOutcomeV1::Transformed => {
                if !binding_matches {
                    return invalid("transformed input binding does not match supplied fragment");
                }
                if self.refusal.is_some() || self.output.is_none() {
                    return invalid("transformed outcome requires output and forbids refusal");
                }
                if self
                    .event_outcomes
                    .iter()
                    .any(|row| matches!(row, ContactEventOutcomeV1::Refused { .. }))
                {
                    return invalid("transformed outcome contains a refused event");
                }
                let output = self.output.as_ref().expect("checked");
                validate_identity(&output.artifact)?;
                validate_identity(output.dependency_closure_identity.input_identity())?;
                validate_identity(&output.fragment)?;
                if output.artifact != *output.contact_fragment.artifact()
                    || output.dependency_closure_identity
                        != *output.contact_fragment.dependency_closure_identity()
                    || output.fragment != output.contact_fragment.canonical_identity()?
                {
                    return invalid("output identities do not match inline contact fragment");
                }
            }
            ContactTransformOutcomeV1::Refused => {
                if self.output.is_some() || self.refusal.is_none() {
                    return invalid("refused outcome requires refusal and forbids output");
                }
                let refusal = self.refusal.as_ref().expect("checked");
                if refusal.code == ContactTransformRefusalCodeV1::InvalidBinding {
                    if binding_matches || !self.event_outcomes.is_empty() {
                        return invalid(
                            "invalid_binding requires a mismatched input and empty event outcomes",
                        );
                    }
                    return Ok(());
                }
                if !binding_matches {
                    return invalid("non-binding refusal carries a mismatched input binding");
                }
            }
        }
        if self.event_outcomes.is_empty() {
            return Ok(());
        }
        if self.event_outcomes.len() != input_fragment.events().len()
            || self
                .event_outcomes
                .iter()
                .zip(input_fragment.events())
                .any(|(outcome, event)| outcome.event_id() != event.event_id())
        {
            return invalid("event outcomes do not exactly cover canonical input event order");
        }
        Ok(())
    }

    fn verify_semantics(
        &self,
        input_fragment: &ContactFragmentV1,
    ) -> Result<(), ContactTransformError> {
        if self
            .refusal
            .as_ref()
            .is_some_and(|refusal| refusal.code == ContactTransformRefusalCodeV1::InvalidBinding)
        {
            return Ok(());
        }
        let supported_extensions = if self.refusal.as_ref().is_some_and(|refusal| {
            refusal.code == ContactTransformRefusalCodeV1::UnsupportedExtension
        }) {
            BTreeSet::new()
        } else {
            input_fragment
                .extensions()
                .iter()
                .map(|extension| (extension.schema().to_owned(), extension.schema_version()))
                .collect()
        };
        let (output_artifact, output_closure, output_producer) = match &self.output {
            Some(output) => (
                output.artifact.clone(),
                output.dependency_closure_identity.clone(),
                output.contact_fragment.producer().clone(),
            ),
            None => (
                input_fragment.artifact().clone(),
                input_fragment.dependency_closure_identity().clone(),
                input_fragment.producer().clone(),
            ),
        };
        let context = ContactTransformContextV1::new(
            self.input.artifact.clone(),
            self.input.dependency_closure_identity.clone(),
            output_artifact,
            output_closure,
            output_producer,
            supported_extensions,
        );
        let mut expected =
            transform_contact_fragment_v1(self.operation.clone(), input_fragment, context)?;
        if let (Some(expected), Some(actual)) = (&mut expected.refusal, &self.refusal) {
            expected.message.clone_from(&actual.message);
        }
        if *self != expected {
            return invalid("result does not equal independent V1 operation rederivation");
        }
        Ok(())
    }
}

/// Transform a contact fragment with explicit output identities and explicit
/// operation-specific extension support.
///
/// The context's extension inventory contains exact `(schema, version)` pairs.
/// Every input extension must appear there or the whole operation refuses.
pub fn transform_contact_fragment_v1(
    operation: ContactTransformOperationV1,
    input_fragment: &ContactFragmentV1,
    context: ContactTransformContextV1,
) -> Result<ContactTransformResultV1, ContactTransformError> {
    validate_operation_structure(&operation)?;
    let input = ContactTransformBindingV1::new(
        context.current_input_artifact,
        context.current_input_dependency_closure_identity,
        input_fragment.canonical_identity()?,
    );
    if input.artifact != *input_fragment.artifact()
        || input.dependency_closure_identity != *input_fragment.dependency_closure_identity()
    {
        return refusal(
            operation,
            input,
            ContactTransformRefusalCodeV1::InvalidBinding,
            "contact fragment does not bind the current source artifact and dependency closure",
            Vec::new(),
        );
    }
    if operation_invalid(&operation) {
        return refusal(
            operation,
            input,
            ContactTransformRefusalCodeV1::InvalidMapping,
            "operation mapping is outside the V1 domain",
            Vec::new(),
        );
    }
    if input_fragment.extensions().iter().any(|extension| {
        !context
            .supported_extensions
            .contains(&(extension.schema().to_owned(), extension.schema_version()))
    }) {
        return refusal(
            operation,
            input,
            ContactTransformRefusalCodeV1::UnsupportedExtension,
            "contact extension has no operation-specific transform support",
            Vec::new(),
        );
    }
    let output_duration_s =
        mapped_duration(&operation, input_fragment.duration_s()).ok_or_else(|| {
            ContactTransformError::Invalid {
                message: "valid mapping produced an invalid duration".into(),
            }
        })?;
    if !output_duration_s.is_finite() || output_duration_s <= 0.0 {
        return refusal(
            operation,
            input,
            ContactTransformRefusalCodeV1::InvalidValue,
            "mapped duration is not finite and positive",
            Vec::new(),
        );
    }
    let mut outcomes = Vec::with_capacity(input_fragment.events().len());
    let mut transformed_events = Vec::with_capacity(input_fragment.events().len());
    let mut partial = false;
    for event in input_fragment.events() {
        match map_event(&operation, event)? {
            MappedEvent::Transformed { outcome, event } => {
                outcomes.push(ContactEventOutcomeV1::Transformed {
                    event_id: event.event_id().to_owned(),
                    value: outcome,
                });
                transformed_events.push(event);
            }
            MappedEvent::Outside => outcomes.push(ContactEventOutcomeV1::Outside {
                event_id: event.event_id().to_owned(),
            }),
            MappedEvent::PartialWindow => {
                partial = true;
                outcomes.push(ContactEventOutcomeV1::Refused {
                    event_id: event.event_id().to_owned(),
                    code: ContactTransformRefusalCodeV1::PartialWindow,
                });
            }
        }
    }
    if partial {
        return refusal(
            operation,
            input,
            ContactTransformRefusalCodeV1::PartialWindow,
            "one or more contact windows cross a retained interval boundary",
            outcomes,
        );
    }
    let output_fragment = ContactFragmentV1::new(
        context.output_producer,
        context.output_artifact.clone(),
        context.output_dependency_closure_identity.clone(),
        input_fragment.clip().clone(),
        output_duration_s,
        transformed_events,
        input_fragment.extensions().to_vec(),
    )?;
    let output = ContactTransformOutputV1 {
        artifact: context.output_artifact,
        dependency_closure_identity: context.output_dependency_closure_identity,
        fragment: output_fragment.canonical_identity()?,
        contact_fragment: output_fragment,
    };
    let result = ContactTransformResultV1 {
        schema: CONTACT_TRANSFORM_RESULT_V1_ID,
        schema_version: CONTACT_TRANSFORM_RESULT_V1_SCHEMA_VERSION,
        operation,
        input,
        outcome: ContactTransformOutcomeV1::Transformed,
        event_outcomes: outcomes,
        output: Some(output),
        refusal: None,
    };
    result.verify_structure(input_fragment)?;
    let _ = result.canonical_json()?;
    Ok(result)
}

enum MappedEvent {
    Transformed {
        outcome: ContactTransformedValueV1,
        event: ContactEventV1,
    },
    Outside,
    PartialWindow,
}

fn map_event(
    operation: &ContactTransformOperationV1,
    event: &ContactEventV1,
) -> Result<MappedEvent, ContactTransformError> {
    match event.kind() {
        ContactEventKindV1::Point(time) => match map_time(operation, time)? {
            Some(time) => Ok(MappedEvent::Transformed {
                outcome: ContactTransformedValueV1::Point { time },
                event: ContactEventV1::point(
                    event.event_id(),
                    event.role(),
                    event.phase(),
                    time,
                    event.confidence(),
                )?,
            }),
            None => Ok(MappedEvent::Outside),
        },
        ContactEventKindV1::Window(window) => {
            if let Some(interval) = retained_interval(operation) {
                if window.end() < interval.start || window.start() > interval.end {
                    return Ok(MappedEvent::Outside);
                }
                if !(interval.start <= window.start() && window.end() <= interval.end) {
                    return Ok(MappedEvent::PartialWindow);
                }
            }
            let start = map_time(operation, window.start())?.ok_or_else(|| {
                ContactTransformError::Invalid {
                    message: "contained window start mapped outside".into(),
                }
            })?;
            let end = map_time(operation, window.end())?.ok_or_else(|| {
                ContactTransformError::Invalid {
                    message: "contained window end mapped outside".into(),
                }
            })?;
            let mapped = ContactEventWindowV1::new(start, end)?;
            Ok(MappedEvent::Transformed {
                outcome: ContactTransformedValueV1::Window { window: mapped },
                event: ContactEventV1::window(
                    event.event_id(),
                    event.role(),
                    event.phase(),
                    mapped,
                    event.confidence(),
                )?,
            })
        }
    }
}

fn map_time(
    operation: &ContactTransformOperationV1,
    time: f64,
) -> Result<Option<f64>, ContactTransformError> {
    let value = match operation {
        ContactTransformOperationV1::Trim { interval, .. }
        | ContactTransformOperationV1::Slice { interval, .. } => {
            if time < interval.start || time > interval.end {
                return Ok(None);
            }
            let span = interval.end - interval.start;
            (time - interval.start) / span
        }
        ContactTransformOperationV1::Resample { .. } => time,
        ContactTransformOperationV1::TimeWarp { control_points, .. } => {
            if let Some(point) = control_points.iter().find(|point| point.input_time == time) {
                point.output_time
            } else {
                let pair = control_points
                    .windows(2)
                    .find(|pair| pair[0].input_time < time && time < pair[1].input_time)
                    .ok_or_else(|| ContactTransformError::Invalid {
                        message: "valid mapping does not cover normalized input time".into(),
                    })?;
                let dx = pair[1].input_time - pair[0].input_time;
                let alpha = (time - pair[0].input_time) / dx;
                let dy = pair[1].output_time - pair[0].output_time;
                let product = alpha * dy;
                pair[0].output_time + product
            }
        }
    };
    if value.is_finite() {
        Ok(Some(if value == 0.0 { 0.0 } else { value }))
    } else {
        Err(ContactTransformError::Invalid {
            message: "mapping produced a non-finite value".into(),
        })
    }
}

fn retained_interval(
    operation: &ContactTransformOperationV1,
) -> Option<ContactTransformIntervalV1> {
    match operation {
        ContactTransformOperationV1::Trim { interval, .. }
        | ContactTransformOperationV1::Slice { interval, .. } => Some(*interval),
        _ => None,
    }
}

fn mapped_duration(operation: &ContactTransformOperationV1, input: f64) -> Option<f64> {
    let value = match operation {
        ContactTransformOperationV1::Trim { interval, .. }
        | ContactTransformOperationV1::Slice { interval, .. } => {
            let span = interval.end - interval.start;
            input * span
        }
        ContactTransformOperationV1::Resample { .. } => input,
        ContactTransformOperationV1::TimeWarp {
            output_duration_s, ..
        } => *output_duration_s,
    };
    value.is_finite().then_some(value)
}

fn operation_invalid(operation: &ContactTransformOperationV1) -> bool {
    match operation {
        ContactTransformOperationV1::Trim { version, interval }
        | ContactTransformOperationV1::Slice { version, interval } => {
            debug_assert_eq!(*version, 1);
            !(0.0 <= interval.start && interval.start < interval.end && interval.end <= 1.0)
        }
        ContactTransformOperationV1::Resample { version, mapping } => {
            debug_assert_eq!(*version, 1);
            debug_assert_eq!(mapping, "identity");
            false
        }
        ContactTransformOperationV1::TimeWarp {
            version,
            output_duration_s,
            control_points,
        } => {
            debug_assert_eq!(*version, 1);
            *output_duration_s <= 0.0
                || control_points.len() < 2
                || control_points
                    .first()
                    .is_none_or(|point| point.input_time != 0.0 || point.output_time != 0.0)
                || control_points
                    .last()
                    .is_none_or(|point| point.input_time != 1.0 || point.output_time != 1.0)
                || control_points.iter().any(|point| {
                    !(0.0..=1.0).contains(&point.input_time)
                        || !(0.0..=1.0).contains(&point.output_time)
                })
                || control_points.windows(2).any(|pair| {
                    pair[0].input_time >= pair[1].input_time
                        || pair[0].output_time >= pair[1].output_time
                })
        }
    }
}

fn validate_operation_structure(
    operation: &ContactTransformOperationV1,
) -> Result<(), ContactTransformError> {
    match operation {
        ContactTransformOperationV1::Trim { version, interval }
        | ContactTransformOperationV1::Slice { version, interval } => {
            if *version != 1 {
                return invalid("unknown contact operation version");
            }
            if !safe_number(interval.start) || !safe_number(interval.end) {
                return invalid("contact interval contains an RFC 8785 unsafe number");
            }
        }
        ContactTransformOperationV1::Resample { version, mapping } => {
            if *version != 1 || mapping != "identity" {
                return invalid("unknown contact resample version or mapping token");
            }
        }
        ContactTransformOperationV1::TimeWarp {
            version,
            output_duration_s,
            control_points,
        } => {
            if *version != 1 {
                return invalid("unknown contact operation version");
            }
            if control_points.len() > CONTACT_TRANSFORM_RESULT_V1_MAX_CONTROL_POINTS {
                return limit(
                    "control_points",
                    control_points.len(),
                    CONTACT_TRANSFORM_RESULT_V1_MAX_CONTROL_POINTS,
                );
            }
            if !safe_number(*output_duration_s)
                || control_points
                    .iter()
                    .any(|point| !safe_number(point.input_time) || !safe_number(point.output_time))
            {
                return invalid("contact time-warp contains an RFC 8785 unsafe number");
            }
        }
    }
    Ok(())
}

fn validate_identity(identity: &InputIdentity) -> Result<(), ContactTransformError> {
    if identity.bytes() > CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER {
        return invalid("contact transform identity bytes exceed the RFC 8785 safe integer");
    }
    Ok(())
}

fn safe_number(value: f64) -> bool {
    value.is_finite() && value.abs() <= CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER as f64
}

fn refusal(
    operation: ContactTransformOperationV1,
    input: ContactTransformBindingV1,
    code: ContactTransformRefusalCodeV1,
    message: &str,
    event_outcomes: Vec<ContactEventOutcomeV1>,
) -> Result<ContactTransformResultV1, ContactTransformError> {
    if message.is_empty() || message.len() > CONTACT_TRANSFORM_RESULT_V1_MAX_MESSAGE_BYTES {
        return invalid("refusal message is outside the V1 byte bound");
    }
    let result = ContactTransformResultV1 {
        schema: CONTACT_TRANSFORM_RESULT_V1_ID,
        schema_version: CONTACT_TRANSFORM_RESULT_V1_SCHEMA_VERSION,
        operation,
        input,
        outcome: ContactTransformOutcomeV1::Refused,
        event_outcomes,
        output: None,
        refusal: Some(ContactTransformRefusalV1 {
            code,
            message: message.into(),
        }),
    };
    let _ = result.canonical_json()?;
    Ok(result)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultWire {
    schema: String,
    schema_version: u32,
    operation: ContactTransformOperationV1,
    input: ContactTransformBindingV1,
    outcome: ContactTransformOutcomeV1,
    #[serde(deserialize_with = "deserialize_event_outcomes")]
    event_outcomes: Vec<ContactEventOutcomeV1>,
    #[serde(default)]
    output: Present<OutputWire>,
    #[serde(default)]
    refusal: Present<ContactTransformRefusalV1>,
}

impl ResultWire {
    fn into_result(self) -> Result<ContactTransformResultV1, ContactTransformError> {
        if self.schema != CONTACT_TRANSFORM_RESULT_V1_ID || self.schema_version != 1 {
            return invalid("schema identity/version mismatch");
        }
        if let Some(refusal) = &self.refusal.0
            && (refusal.message.is_empty()
                || refusal.message.len() > CONTACT_TRANSFORM_RESULT_V1_MAX_MESSAGE_BYTES)
        {
            return invalid("refusal message is outside the V1 byte bound");
        }
        let output = self.output.0.map(OutputWire::into_output).transpose()?;
        Ok(ContactTransformResultV1 {
            schema: CONTACT_TRANSFORM_RESULT_V1_ID,
            schema_version: 1,
            operation: self.operation,
            input: self.input,
            outcome: self.outcome,
            event_outcomes: self.event_outcomes,
            output,
            refusal: self.refusal.0,
        })
    }
}

struct Present<T>(Option<T>);

impl<T> Default for Present<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Present<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(|value| Self(Some(value)))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputWire {
    artifact: InputIdentity,
    dependency_closure_identity: DependencyClosureIdentityV1,
    fragment: InputIdentity,
    contact_fragment: StrictJsonValue,
}

impl OutputWire {
    fn into_output(self) -> Result<ContactTransformOutputV1, ContactTransformError> {
        let bytes = serde_json::to_vec(&self.contact_fragment.into_value()).map_err(|error| {
            ContactTransformError::Invalid {
                message: error.to_string(),
            }
        })?;
        Ok(ContactTransformOutputV1 {
            artifact: self.artifact,
            dependency_closure_identity: self.dependency_closure_identity,
            fragment: self.fragment,
            contact_fragment: ContactFragmentV1::read_json(&bytes)?,
        })
    }
}

fn deserialize_control_points<'de, D>(
    deserializer: D,
) -> Result<Vec<ContactTimeWarpControlPointV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped(
        deserializer,
        CONTACT_TRANSFORM_RESULT_V1_MAX_CONTROL_POINTS,
        "control_points",
    )
}

fn deserialize_event_outcomes<'de, D>(
    deserializer: D,
) -> Result<Vec<ContactEventOutcomeV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped(
        deserializer,
        CONTACT_TRANSFORM_RESULT_V1_MAX_EVENT_OUTCOMES,
        "event_outcomes",
    )
}

fn deserialize_capped<'de, D, T>(
    deserializer: D,
    maximum: usize,
    field: &'static str,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct CappedVisitor<T> {
        maximum: usize,
        field: &'static str,
        marker: std::marker::PhantomData<T>,
    }
    impl<'de, T: Deserialize<'de>> Visitor<'de> for CappedVisitor<T> {
        type Value = Vec<T>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "at most {} {} rows", self.maximum, self.field)
        }
        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values =
                Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(self.maximum));
            while values.len() < self.maximum {
                let Some(value) = sequence.next_element()? else {
                    return Ok(values);
                };
                values.push(value);
            }
            if sequence.next_element::<IgnoredAny>()?.is_some() {
                while sequence.next_element::<IgnoredAny>()?.is_some() {}
                return Err(A::Error::custom(format!(
                    "{} exceeds V1 row limit {}",
                    self.field, self.maximum
                )));
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(CappedVisitor {
        maximum,
        field,
        marker: std::marker::PhantomData,
    })
}

struct CappedWriter {
    bytes: Vec<u8>,
    overflowed: bool,
}

impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len())
            > CONTACT_TRANSFORM_RESULT_V1_MAX_CANONICAL_BYTES
        {
            self.overflowed = true;
            return Err(io::Error::other("contact-transform canonical limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn invalid_decode(error: serde_json::Error) -> ContactTransformError {
    ContactTransformError::Invalid {
        message: error.to_string(),
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T, ContactTransformError> {
    Err(ContactTransformError::Invalid {
        message: message.into(),
    })
}

fn limit<T>(field: &'static str, found: usize, limit: usize) -> Result<T, ContactTransformError> {
    Err(ContactTransformError::LimitExceeded {
        field,
        found,
        limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContactClipReferenceV1, ContactExtensionV1, ContactPhaseV1, ContactRoleV1,
        DependencyClosureBuilderV1, SourceSetCoverageV1,
    };

    fn closure(primary: &InputIdentity) -> DependencyClosureIdentityV1 {
        DependencyClosureBuilderV1::new(primary.clone(), SourceSetCoverageV1::complete(), 0)
            .finish()
            .unwrap()
            .identity()
            .unwrap()
            .clone()
    }

    fn fragment(with_extension: bool) -> ContactFragmentV1 {
        let artifact = InputIdentity::from_bytes(b"source artifact");
        let events = vec![
            ContactEventV1::point(
                "left/point",
                ContactRoleV1::LeftFoot,
                ContactPhaseV1::Marker,
                0.25,
                Some(0.8),
            )
            .unwrap(),
            ContactEventV1::window(
                "right/window",
                ContactRoleV1::RightFoot,
                ContactPhaseV1::Begin,
                ContactEventWindowV1::new(0.5, 0.75).unwrap(),
                None,
            )
            .unwrap(),
        ];
        let extensions = with_extension
            .then(|| {
                ContactExtensionV1::new(
                    "urn:animsmith:contact-detector:stance-support:1",
                    1,
                    serde_json::json!({"algorithm": "fixture"}),
                )
                .unwrap()
            })
            .into_iter()
            .collect();
        ContactFragmentV1::new(
            ContactProducerV1::new("fixture", "1").unwrap(),
            artifact.clone(),
            closure(&artifact),
            ContactClipReferenceV1::document("walk").unwrap(),
            2.0,
            events,
            extensions,
        )
        .unwrap()
    }

    fn warp() -> ContactTransformOperationV1 {
        ContactTransformOperationV1::time_warp(
            2.0,
            vec![
                ContactTimeWarpControlPointV1::new(0.0, 0.0),
                ContactTimeWarpControlPointV1::new(0.5, 0.25),
                ContactTimeWarpControlPointV1::new(1.0, 1.0),
            ],
        )
    }

    fn transform(
        source: &ContactFragmentV1,
        operation: ContactTransformOperationV1,
        supported: BTreeSet<(String, u32)>,
    ) -> ContactTransformResultV1 {
        let output = InputIdentity::from_bytes(b"output artifact");
        let context = ContactTransformContextV1::new(
            source.artifact().clone(),
            source.dependency_closure_identity().clone(),
            output.clone(),
            closure(&output),
            ContactProducerV1::new("fixture", "2").unwrap(),
            supported,
        );
        transform_contact_fragment_v1(operation, source, context).unwrap()
    }

    #[test]
    fn time_warp_maps_points_and_both_window_boundaries() {
        let source = fragment(false);
        let result = transform(&source, warp(), BTreeSet::new());
        assert_eq!(result.outcome(), ContactTransformOutcomeV1::Transformed);
        assert_eq!(
            result.event_outcomes(),
            &[
                ContactEventOutcomeV1::Transformed {
                    event_id: "left/point".into(),
                    value: ContactTransformedValueV1::Point { time: 0.125 },
                },
                ContactEventOutcomeV1::Transformed {
                    event_id: "right/window".into(),
                    value: ContactTransformedValueV1::Window {
                        window: ContactEventWindowV1::new(0.25, 0.625).unwrap(),
                    },
                },
            ]
        );
        let output = result.output().unwrap().contact_fragment();
        assert_eq!(output.duration_s(), 2.0);
        assert_eq!(output.events()[0].kind(), ContactEventKindV1::Point(0.125));
        assert_eq!(
            output.events()[1].kind(),
            ContactEventKindV1::Window(ContactEventWindowV1::new(0.25, 0.625).unwrap())
        );

        let bytes = result.canonical_json().unwrap();
        assert_eq!(
            ContactTransformResultV1::read_json(&bytes, &source).unwrap(),
            result
        );
    }

    #[test]
    fn exact_knots_bypass_interpolation() {
        let source = fragment(false);
        let result = transform(&source, warp(), BTreeSet::new());
        let ContactEventOutcomeV1::Transformed {
            value: ContactTransformedValueV1::Window { window },
            ..
        } = &result.event_outcomes()[1]
        else {
            panic!("window outcome");
        };
        assert_eq!(window.start().to_bits(), 0.25_f64.to_bits());
    }

    #[test]
    fn invalid_mapping_and_stale_binding_refuse_before_event_inventory() {
        let source = fragment(false);
        let invalid = transform(
            &source,
            ContactTransformOperationV1::time_warp(
                2.0,
                vec![
                    ContactTimeWarpControlPointV1::new(0.0, 0.0),
                    ContactTimeWarpControlPointV1::new(0.5, 0.75),
                    ContactTimeWarpControlPointV1::new(1.0, 0.5),
                ],
            ),
            BTreeSet::new(),
        );
        assert_eq!(
            invalid.refusal().unwrap().code(),
            ContactTransformRefusalCodeV1::InvalidMapping
        );
        assert!(invalid.event_outcomes().is_empty());

        let output = InputIdentity::from_bytes(b"output artifact");
        let stale_context = ContactTransformContextV1::new(
            InputIdentity::from_bytes(b"changed source"),
            source.dependency_closure_identity().clone(),
            output.clone(),
            closure(&output),
            ContactProducerV1::new("fixture", "2").unwrap(),
            BTreeSet::new(),
        );
        let stale = transform_contact_fragment_v1(warp(), &source, stale_context).unwrap();
        assert_eq!(
            stale.refusal().unwrap().code(),
            ContactTransformRefusalCodeV1::InvalidBinding
        );
        assert!(stale.event_outcomes().is_empty());
    }

    #[test]
    fn partial_window_refuses_whole_operation_without_output() {
        let source = fragment(false);
        let result = transform(
            &source,
            ContactTransformOperationV1::trim(ContactTransformIntervalV1::new(0.6, 1.0)),
            BTreeSet::new(),
        );
        assert_eq!(result.outcome(), ContactTransformOutcomeV1::Refused);
        assert!(result.output().is_none());
        assert_eq!(
            result.refusal().unwrap().code(),
            ContactTransformRefusalCodeV1::PartialWindow
        );
        assert!(matches!(
            result.event_outcomes()[1],
            ContactEventOutcomeV1::Refused {
                code: ContactTransformRefusalCodeV1::PartialWindow,
                ..
            }
        ));
    }

    #[test]
    fn extensions_require_exact_explicit_support() {
        let source = fragment(true);
        let refused = transform(&source, warp(), BTreeSet::new());
        assert_eq!(
            refused.refusal().unwrap().code(),
            ContactTransformRefusalCodeV1::UnsupportedExtension
        );
        let supported =
            BTreeSet::from([("urn:animsmith:contact-detector:stance-support:1".into(), 1)]);
        assert_eq!(
            transform(&source, warp(), supported).outcome(),
            ContactTransformOutcomeV1::Transformed
        );
    }

    #[test]
    fn reader_rederives_event_values_instead_of_trusting_result_rows() {
        let source = fragment(false);
        let result = transform(&source, warp(), BTreeSet::new());
        let mut value: serde_json::Value =
            serde_json::from_slice(&result.canonical_json().unwrap()).unwrap();
        value["event_outcomes"][0]["value"]["time"] = serde_json::json!(0.2);
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(ContactTransformResultV1::read_json(&bytes, &source).is_err());
    }

    #[test]
    fn reader_rejects_unknown_duplicate_and_n_plus_one_control_points() {
        let source = fragment(false);
        let result = transform(&source, warp(), BTreeSet::new());
        let mut value: serde_json::Value =
            serde_json::from_slice(&result.canonical_json().unwrap()).unwrap();
        value["unknown"] = serde_json::json!(true);
        assert!(
            ContactTransformResultV1::read_json(&serde_json::to_vec(&value).unwrap(), &source)
                .is_err()
        );

        let duplicate = result
            .canonical_json()
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>();
        let text = String::from_utf8(duplicate).unwrap();
        let duplicate = text.replacen(
            "{\"event_outcomes\"",
            "{\"schema\":\"urn:animsmith:schema:contact-transform-result:1\",\"event_outcomes\"",
            1,
        );
        assert!(ContactTransformResultV1::read_json(duplicate.as_bytes(), &source).is_err());

        let mut value: serde_json::Value =
            serde_json::from_slice(&result.canonical_json().unwrap()).unwrap();
        value["operation"]["control_points"] = serde_json::Value::Array(
            (0..=CONTACT_TRANSFORM_RESULT_V1_MAX_CONTROL_POINTS)
                .map(|index| {
                    let value =
                        index as f64 / CONTACT_TRANSFORM_RESULT_V1_MAX_CONTROL_POINTS as f64;
                    serde_json::json!({"input_time": value, "output_time": value})
                })
                .collect(),
        );
        assert!(
            ContactTransformResultV1::read_json(&serde_json::to_vec(&value).unwrap(), &source)
                .is_err()
        );
    }

    #[test]
    fn source_and_canonical_byte_caps_fail_closed() {
        let source = fragment(false);
        assert!(matches!(
            ContactTransformResultV1::read_json(
                &vec![b' '; CONTACT_TRANSFORM_RESULT_V1_MAX_SOURCE_BYTES + 1],
                &source,
            ),
            Err(ContactTransformError::SourceTooLarge { .. })
        ));
    }
}
