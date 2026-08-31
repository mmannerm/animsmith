//! Strict, format-neutral contact-fragment V1 values.
//!
//! This module owns the interchange reader and canonical JSON seam. It does
//! not infer contacts, load assets, perform transforms, or publish files.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

use serde::de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::{DependencyClosureIdentityV1, InputIdentity};

/// Immutable schema identity for contact-fragment V1.
pub const CONTACT_FRAGMENT_V1_ID: &str = "urn:animsmith:schema:contact-fragment:1";
/// Immutable schema version for contact-fragment V1.
pub const CONTACT_FRAGMENT_V1_SCHEMA_VERSION: u32 = 1;
/// Maximum accepted UTF-8 JSON source bytes.
pub const CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
/// Maximum canonical RFC 8785 JSON bytes.
pub const CONTACT_FRAGMENT_V1_MAX_CANONICAL_BYTES: usize = 8 * 1024 * 1024;
/// Maximum core events per fragment.
pub const CONTACT_FRAGMENT_V1_MAX_EVENTS: usize = 4_096;
/// Maximum strict extension envelopes per fragment.
pub const CONTACT_FRAGMENT_V1_MAX_EXTENSIONS: usize = 256;
/// Maximum UTF-8 bytes in one ordinary authored string or object key.
pub const CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES: usize = 4_096;
/// Maximum UTF-8 bytes in a V1 identifier.
pub const CONTACT_FRAGMENT_V1_MAX_IDENTIFIER_BYTES: usize = 255;
/// Maximum full-envelope object/array depth, including the root object.
pub const CONTACT_FRAGMENT_V1_MAX_DEPTH: usize = 32;
/// Maximum RFC 8785 bytes in one opaque extension payload.
pub const CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_BYTES: usize = 256 * 1024;
/// Maximum object/array depth within one extension payload.
pub const CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_DEPTH: usize = 16;
/// Largest exactly representable integer in the RFC 8785 / IEEE-754 JSON seam.
///
/// Contact-fragment V1 rejects integral JSON values outside this range before
/// canonicalization. This is deliberately a contact-fragment contract, not a
/// general replacement for the crate's identity authority.
pub const CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
// A valid opaque payload with this many scalar members necessarily exceeds its
// JCS-byte cap; this bounds generic nested collections before retention.
const MAX_OPAQUE_MEMBERS: usize = CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_BYTES;

/// Reader or contract violation for contact-fragment V1.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ContactFragmentError {
    /// JSON source exceeded the immutable V1 input cap.
    #[error("contact fragment source has {bytes} bytes, exceeding V1 limit {limit}")]
    SourceTooLarge {
        /// Observed source byte count.
        bytes: usize,
        /// Frozen V1 source byte limit.
        limit: usize,
    },
    /// The source was not valid UTF-8 JSON or used duplicate object members.
    #[error("invalid contact fragment JSON: {message}")]
    InvalidJson {
        /// Decoder detail, not a stable machine token.
        message: String,
    },
    /// A required V1 field was absent, malformed, or an object contained an unknown field.
    #[error("invalid contact fragment {field}: {message}")]
    InvalidField {
        /// Stable V1 field path.
        field: &'static str,
        /// Decoder detail, not a stable machine token.
        message: String,
    },
    /// A frozen V1 row or byte bound was exceeded.
    #[error("contact fragment {field} has {found}, exceeding V1 limit {limit}")]
    LimitExceeded {
        /// Stable V1 bounded field.
        field: &'static str,
        /// Observed count or byte length.
        found: usize,
        /// Frozen V1 limit.
        limit: usize,
    },
    /// RFC 8785 canonical output could not be represented within the V1 cap.
    #[error("contact fragment canonical JSON exceeds V1 limit {limit}")]
    CanonicalTooLarge {
        /// Frozen V1 canonical byte limit.
        limit: usize,
    },
}

/// The closed V1 semantic role vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactRoleV1 {
    /// Left foot.
    LeftFoot,
    /// Right foot.
    RightFoot,
    /// Left hand.
    LeftHand,
    /// Right hand.
    RightHand,
    /// Left toe.
    LeftToe,
    /// Right toe.
    RightToe,
    /// Left knee.
    LeftKnee,
    /// Right knee.
    RightKnee,
    /// Left elbow.
    LeftElbow,
    /// Right elbow.
    RightElbow,
    /// Root.
    Root,
    /// Prop.
    Prop,
    /// Body.
    Body,
}

/// The closed V1 event-phase vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactPhaseV1 {
    /// Start-like phase.
    Begin,
    /// End-like phase.
    End,
    /// One instantaneous marker.
    Marker,
}

/// Exact V1 producer identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContactProducerV1 {
    tool: String,
    version: String,
}

impl ContactProducerV1 {
    /// Construct a bounded producer identity.
    pub fn new(
        tool: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, ContactFragmentError> {
        let value = Self {
            tool: tool.into(),
            version: version.into(),
        };
        identifier(&value.tool, "producer.tool")?;
        identifier(&value.version, "producer.version")?;
        Ok(value)
    }

    /// Producer tool identifier.
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// Producer version identifier.
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Exact selected clip witness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum ContactClipReferenceV1 {
    /// A uniquely named clip in one loaded document.
    Document {
        /// Exact embedded clip name.
        clip_name: String,
    },
    /// A collection manifest witness and its exact source take.
    Collection {
        /// Logical clip identifier.
        logical_id: String,
        /// Collection source key.
        source: String,
        /// Exact source-local take index.
        take_index: u32,
        /// Exact source-local take name.
        take_name: String,
    },
}

impl ContactClipReferenceV1 {
    /// Construct a document-scoped witness.
    pub fn document(clip_name: impl Into<String>) -> Result<Self, ContactFragmentError> {
        let clip_name = clip_name.into();
        text(&clip_name, "clip.clip_name")?;
        Ok(Self::Document { clip_name })
    }

    /// Construct a collection-scoped witness.
    pub fn collection(
        logical_id: impl Into<String>,
        source: impl Into<String>,
        take_index: u32,
        take_name: impl Into<String>,
    ) -> Result<Self, ContactFragmentError> {
        let value = Self::Collection {
            logical_id: logical_id.into(),
            source: source.into(),
            take_index,
            take_name: take_name.into(),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), ContactFragmentError> {
        match self {
            Self::Document { clip_name } => text(clip_name, "clip.clip_name"),
            Self::Collection {
                logical_id,
                source,
                take_name,
                ..
            } => {
                identifier(logical_id, "clip.logical_id")?;
                identifier(source, "clip.source")?;
                text(take_name, "clip.take_name")
            }
        }
    }
}

/// Inclusive normalized time window.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactEventWindowV1 {
    start: f64,
    end: f64,
}

impl ContactEventWindowV1 {
    /// Construct a finite normalized window with `start <= end`.
    pub fn new(start: f64, end: f64) -> Result<Self, ContactFragmentError> {
        normalized_time(start, "event.window.start")?;
        normalized_time(end, "event.window.end")?;
        if start > end {
            return invalid("event.window", "start must not exceed end");
        }
        Ok(Self {
            start: canonical_number(start),
            end: canonical_number(end),
        })
    }
    /// Window start.
    pub const fn start(self) -> f64 {
        self.start
    }
    /// Window end.
    pub const fn end(self) -> f64 {
        self.end
    }
}

/// The exactly-one point/window event shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContactEventKindV1 {
    /// One normalized instant.
    Point(f64),
    /// One normalized interval.
    Window(ContactEventWindowV1),
}

/// One stable event fact.
#[derive(Debug, Clone, PartialEq)]
pub struct ContactEventV1 {
    event_id: String,
    role: ContactRoleV1,
    phase: ContactPhaseV1,
    kind: ContactEventKindV1,
    confidence: Option<f64>,
}

impl ContactEventV1 {
    /// Construct a point event.
    pub fn point(
        event_id: impl Into<String>,
        role: ContactRoleV1,
        phase: ContactPhaseV1,
        time: f64,
        confidence: Option<f64>,
    ) -> Result<Self, ContactFragmentError> {
        Self::new(
            event_id.into(),
            role,
            phase,
            ContactEventKindV1::Point(time),
            confidence,
        )
    }
    /// Construct a window event.
    pub fn window(
        event_id: impl Into<String>,
        role: ContactRoleV1,
        phase: ContactPhaseV1,
        window: ContactEventWindowV1,
        confidence: Option<f64>,
    ) -> Result<Self, ContactFragmentError> {
        Self::new(
            event_id.into(),
            role,
            phase,
            ContactEventKindV1::Window(window),
            confidence,
        )
    }
    fn new(
        event_id: String,
        role: ContactRoleV1,
        phase: ContactPhaseV1,
        kind: ContactEventKindV1,
        confidence: Option<f64>,
    ) -> Result<Self, ContactFragmentError> {
        identifier(&event_id, "event.event_id")?;
        match kind {
            ContactEventKindV1::Point(time) => normalized_time(time, "event.time")?,
            ContactEventKindV1::Window(window) => {
                let _ = ContactEventWindowV1::new(window.start, window.end)?;
            }
        }
        if let Some(confidence) = confidence {
            finite_range(confidence, 0.0, 1.0, "event.confidence")?;
        }
        let kind = match kind {
            ContactEventKindV1::Point(time) => ContactEventKindV1::Point(canonical_number(time)),
            ContactEventKindV1::Window(window) => ContactEventKindV1::Window(window),
        };
        Ok(Self {
            event_id,
            role,
            phase,
            kind,
            confidence: confidence.map(canonical_number),
        })
    }
    /// Opaque stable event identifier.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }
    /// Event role.
    pub const fn role(&self) -> ContactRoleV1 {
        self.role
    }
    /// Event phase.
    pub const fn phase(&self) -> ContactPhaseV1 {
        self.phase
    }
    /// Point or window shape.
    pub const fn kind(&self) -> ContactEventKindV1 {
        self.kind
    }

    /// Optional confidence in the closed interval `[0, 1]`.
    pub const fn confidence(&self) -> Option<f64> {
        self.confidence
    }
}

impl Serialize for ContactEventV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(5))?;
        map.serialize_entry("event_id", &self.event_id)?;
        map.serialize_entry("role", &self.role)?;
        map.serialize_entry("phase", &self.phase)?;
        match self.kind {
            ContactEventKindV1::Point(time) => map.serialize_entry("time", &time)?,
            ContactEventKindV1::Window(window) => map.serialize_entry("window", &window)?,
        }
        if let Some(confidence) = self.confidence {
            map.serialize_entry("confidence", &confidence)?;
        }
        map.end()
    }
}

/// One strict extension envelope preserved as a generic JSON object payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContactExtensionV1 {
    schema: String,
    schema_version: u32,
    payload: Value,
}

impl ContactExtensionV1 {
    /// Construct and bound an opaque extension payload.
    pub fn new(
        schema: impl Into<String>,
        schema_version: u32,
        payload: Value,
    ) -> Result<Self, ContactFragmentError> {
        let mut value = Self {
            schema: schema.into(),
            schema_version,
            payload,
        };
        identifier(&value.schema, "extension.schema")?;
        if value.schema_version == 0 {
            return invalid("extension.schema_version", "must be positive");
        }
        if !value.payload.is_object() {
            return invalid("extension.payload", "must be an object");
        }
        validate_json_value(
            &value.payload,
            1,
            CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_DEPTH,
            "extension.payload",
        )?;
        let canonical_payload = jcs(
            &value.payload,
            CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_BYTES,
        )?;
        value.payload = serde_json::from_slice(&canonical_payload).map_err(|error| {
            ContactFragmentError::InvalidField {
                field: "extension.payload",
                message: error.to_string(),
            }
        })?;
        Ok(value)
    }

    /// Versioned extension schema identity.
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Extension schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Opaque, strict-object extension payload.
    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// A validated, canonicalizable contact-fragment V1 envelope.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContactFragmentV1 {
    schema: &'static str,
    schema_version: u32,
    producer: ContactProducerV1,
    artifact: InputIdentity,
    dependency_closure_identity: DependencyClosureIdentityV1,
    clip: ContactClipReferenceV1,
    duration_s: f64,
    events: Vec<ContactEventV1>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extensions: Vec<ContactExtensionV1>,
}

impl ContactFragmentV1 {
    /// Construct an envelope and impose the deterministic V1 event order.
    pub fn new(
        producer: ContactProducerV1,
        artifact: InputIdentity,
        dependency_closure_identity: DependencyClosureIdentityV1,
        clip: ContactClipReferenceV1,
        duration_s: f64,
        mut events: Vec<ContactEventV1>,
        extensions: Vec<ContactExtensionV1>,
    ) -> Result<Self, ContactFragmentError> {
        safe_identity_bytes(&artifact, "artifact.bytes")?;
        safe_identity_bytes(
            dependency_closure_identity.input_identity(),
            "dependency_closure_identity.bytes",
        )?;
        if !safe_f64(duration_s) {
            return invalid("duration_s", "must be an RFC 8785 safe number");
        }
        positive_finite(duration_s, "duration_s")?;
        let duration_s = canonical_number(duration_s);
        clip.validate()?;
        if events.len() > CONTACT_FRAGMENT_V1_MAX_EVENTS {
            return limit("events", events.len(), CONTACT_FRAGMENT_V1_MAX_EVENTS);
        }
        if extensions.len() > CONTACT_FRAGMENT_V1_MAX_EXTENSIONS {
            return limit(
                "extensions",
                extensions.len(),
                CONTACT_FRAGMENT_V1_MAX_EXTENSIONS,
            );
        }
        let mut ids = BTreeSet::new();
        for event in &events {
            if !ids.insert(event.event_id.clone()) {
                return invalid("events", "event_id must be unique");
            }
        }
        events.sort_by(event_order);
        let result = Self {
            schema: CONTACT_FRAGMENT_V1_ID,
            schema_version: CONTACT_FRAGMENT_V1_SCHEMA_VERSION,
            producer,
            artifact,
            dependency_closure_identity,
            clip,
            duration_s,
            events,
            extensions,
        };
        let _ = result.canonical_json()?;
        Ok(result)
    }

    /// Strictly decode one bounded UTF-8 JSON contact fragment.
    pub fn read_json(bytes: &[u8]) -> Result<Self, ContactFragmentError> {
        if bytes.len() > CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES {
            return Err(ContactFragmentError::SourceTooLarge {
                bytes: bytes.len(),
                limit: CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES,
            });
        }
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);
        let wire = ContactFragmentWire::deserialize(&mut deserializer).map_err(|error| {
            ContactFragmentError::InvalidJson {
                message: error.to_string(),
            }
        })?;
        deserializer
            .end()
            .map_err(|error| ContactFragmentError::InvalidJson {
                message: error.to_string(),
            })?;
        if wire.events.overflowed {
            return limit(
                "events",
                CONTACT_FRAGMENT_V1_MAX_EVENTS + 1,
                CONTACT_FRAGMENT_V1_MAX_EVENTS,
            );
        }
        if wire.extensions.overflowed {
            return limit(
                "extensions",
                CONTACT_FRAGMENT_V1_MAX_EXTENSIONS + 1,
                CONTACT_FRAGMENT_V1_MAX_EXTENSIONS,
            );
        }
        if wire.extensions_present && wire.extensions.values.is_empty() {
            return invalid("extensions", "must be omitted when empty");
        }
        let value = wire.into_value();
        parse_fragment(value)
    }

    /// RFC 8785 bytes, with the frozen V1 canonical-output bound.
    pub fn canonical_json(&self) -> Result<Vec<u8>, ContactFragmentError> {
        jcs(self, CONTACT_FRAGMENT_V1_MAX_CANONICAL_BYTES)
    }
    /// Identity of the exact canonical fragment bytes.
    pub fn canonical_identity(&self) -> Result<InputIdentity, ContactFragmentError> {
        Ok(InputIdentity::from_bytes(&self.canonical_json()?))
    }

    /// Exact tool and version that produced this fragment.
    pub fn producer(&self) -> &ContactProducerV1 {
        &self.producer
    }

    /// Events in frozen V1 canonical event order.
    pub fn events(&self) -> &[ContactEventV1] {
        &self.events
    }

    /// Exact source-artifact identity binding.
    pub fn artifact(&self) -> &InputIdentity {
        &self.artifact
    }

    /// Complete dependency-closure identity binding.
    pub fn dependency_closure_identity(&self) -> &DependencyClosureIdentityV1 {
        &self.dependency_closure_identity
    }

    /// Selected clip witness.
    pub fn clip(&self) -> &ContactClipReferenceV1 {
        &self.clip
    }

    /// Positive clip duration in seconds.
    pub const fn duration_s(&self) -> f64 {
        self.duration_s
    }

    /// Strict extension envelopes in declared array order.
    pub fn extensions(&self) -> &[ContactExtensionV1] {
        &self.extensions
    }
}

fn parse_fragment(value: Value) -> Result<ContactFragmentV1, ContactFragmentError> {
    let mut root = object(value, "fragment")?;
    exact_fields(
        &root,
        "fragment",
        &[
            "schema",
            "schema_version",
            "producer",
            "artifact",
            "dependency_closure_identity",
            "clip",
            "duration_s",
            "events",
            "extensions",
        ],
    )?;
    let schema = string(take(&mut root, "schema", "fragment")?, "schema")?;
    if schema != CONTACT_FRAGMENT_V1_ID {
        return invalid("schema", "must equal contact-fragment V1 schema id");
    }
    let version = u32_value(
        take(&mut root, "schema_version", "fragment")?,
        "schema_version",
    )?;
    if version != CONTACT_FRAGMENT_V1_SCHEMA_VERSION {
        return invalid("schema_version", "must equal 1");
    }
    let producer = parse_producer(take(&mut root, "producer", "fragment")?)?;
    let artifact = parse_input_identity(take(&mut root, "artifact", "fragment")?, "artifact")?;
    let dependency_closure_identity = parse_dependency_closure_identity(
        take(&mut root, "dependency_closure_identity", "fragment")?,
        "dependency_closure_identity",
    )?;
    let clip = parse_clip(take(&mut root, "clip", "fragment")?)?;
    let duration_s = number(take(&mut root, "duration_s", "fragment")?, "duration_s")?;
    let events = array(take(&mut root, "events", "fragment")?, "events")?
        .into_iter()
        .map(parse_event)
        .collect::<Result<Vec<_>, _>>()?;
    let extensions = match root.remove("extensions") {
        Some(value) => array(value, "extensions")?
            .into_iter()
            .map(parse_extension)
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    ContactFragmentV1::new(
        producer,
        artifact,
        dependency_closure_identity,
        clip,
        duration_s,
        events,
        extensions,
    )
}

fn parse_producer(value: Value) -> Result<ContactProducerV1, ContactFragmentError> {
    let mut value = object(value, "producer")?;
    exact_fields(&value, "producer", &["tool", "version"])?;
    ContactProducerV1::new(
        string(take(&mut value, "tool", "producer")?, "producer.tool")?,
        string(take(&mut value, "version", "producer")?, "producer.version")?,
    )
}

fn parse_input_identity(
    value: Value,
    field: &'static str,
) -> Result<InputIdentity, ContactFragmentError> {
    serde_json::from_value(normalize_identity_wire(value, field)?).map_err(|error| {
        ContactFragmentError::InvalidField {
            field,
            message: error.to_string(),
        }
    })
}

fn parse_dependency_closure_identity(
    value: Value,
    field: &'static str,
) -> Result<DependencyClosureIdentityV1, ContactFragmentError> {
    serde_json::from_value(normalize_identity_wire(value, field)?).map_err(|error| {
        ContactFragmentError::InvalidField {
            field,
            message: error.to_string(),
        }
    })
}

fn normalize_identity_wire(
    value: Value,
    field: &'static str,
) -> Result<Value, ContactFragmentError> {
    let mut value = object(value, field)?;
    let bytes_field = if field == "artifact" {
        "artifact.bytes"
    } else {
        "dependency_closure_identity.bytes"
    };
    let bytes = unsigned_integer(
        take(&mut value, "bytes", field)?,
        CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER,
        bytes_field,
    )?;
    value.insert("bytes".into(), Value::Number(bytes.into()));
    Ok(Value::Object(value))
}
fn parse_clip(value: Value) -> Result<ContactClipReferenceV1, ContactFragmentError> {
    let mut value = object(value, "clip")?;
    let scope = string(take(&mut value, "scope", "clip")?, "clip.scope")?;
    match scope.as_str() {
        "document" => {
            exact_fields(&value, "clip", &["scope", "clip_name"])?;
            ContactClipReferenceV1::document(string(
                take(&mut value, "clip_name", "clip")?,
                "clip.clip_name",
            )?)
        }
        "collection" => {
            exact_fields(
                &value,
                "clip",
                &["scope", "logical_id", "source", "take_index", "take_name"],
            )?;
            ContactClipReferenceV1::collection(
                string(take(&mut value, "logical_id", "clip")?, "clip.logical_id")?,
                string(take(&mut value, "source", "clip")?, "clip.source")?,
                u32_value(take(&mut value, "take_index", "clip")?, "clip.take_index")?,
                string(take(&mut value, "take_name", "clip")?, "clip.take_name")?,
            )
        }
        _ => invalid("clip.scope", "must be document or collection"),
    }
}
fn parse_event(value: Value) -> Result<ContactEventV1, ContactFragmentError> {
    let mut value = object(value, "event")?;
    exact_fields(
        &value,
        "event",
        &["event_id", "role", "phase", "time", "window", "confidence"],
    )?;
    let event_id = string(take(&mut value, "event_id", "event")?, "event.event_id")?;
    let role = parse_role(&string(take(&mut value, "role", "event")?, "event.role")?)?;
    let phase = parse_phase(&string(take(&mut value, "phase", "event")?, "event.phase")?)?;
    let confidence = value
        .remove("confidence")
        .map(|value| number(value, "event.confidence"))
        .transpose()?;
    match (value.remove("time"), value.remove("window")) {
        (Some(time), None) => ContactEventV1::point(
            event_id,
            role,
            phase,
            number(time, "event.time")?,
            confidence,
        ),
        (None, Some(window)) => {
            let mut window = object(window, "event.window")?;
            exact_fields(&window, "event.window", &["start", "end"])?;
            ContactEventV1::window(
                event_id,
                role,
                phase,
                ContactEventWindowV1::new(
                    number(
                        take(&mut window, "start", "event.window")?,
                        "event.window.start",
                    )?,
                    number(
                        take(&mut window, "end", "event.window")?,
                        "event.window.end",
                    )?,
                )?,
                confidence,
            )
        }
        _ => invalid("event", "must contain exactly one of time or window"),
    }
}
fn parse_extension(value: Value) -> Result<ContactExtensionV1, ContactFragmentError> {
    let mut value = object(value, "extension")?;
    exact_fields(
        &value,
        "extension",
        &["schema", "schema_version", "payload"],
    )?;
    ContactExtensionV1::new(
        string(take(&mut value, "schema", "extension")?, "extension.schema")?,
        u32_value(
            take(&mut value, "schema_version", "extension")?,
            "extension.schema_version",
        )?,
        take(&mut value, "payload", "extension")?,
    )
}

fn parse_role(value: &str) -> Result<ContactRoleV1, ContactFragmentError> {
    Ok(match value {
        "left_foot" => ContactRoleV1::LeftFoot,
        "right_foot" => ContactRoleV1::RightFoot,
        "left_hand" => ContactRoleV1::LeftHand,
        "right_hand" => ContactRoleV1::RightHand,
        "left_toe" => ContactRoleV1::LeftToe,
        "right_toe" => ContactRoleV1::RightToe,
        "left_knee" => ContactRoleV1::LeftKnee,
        "right_knee" => ContactRoleV1::RightKnee,
        "left_elbow" => ContactRoleV1::LeftElbow,
        "right_elbow" => ContactRoleV1::RightElbow,
        "root" => ContactRoleV1::Root,
        "prop" => ContactRoleV1::Prop,
        "body" => ContactRoleV1::Body,
        _ => return invalid("event.role", "is not a V1 role"),
    })
}
fn parse_phase(value: &str) -> Result<ContactPhaseV1, ContactFragmentError> {
    Ok(match value {
        "begin" => ContactPhaseV1::Begin,
        "end" => ContactPhaseV1::End,
        "marker" => ContactPhaseV1::Marker,
        _ => return invalid("event.phase", "is not a V1 phase"),
    })
}

fn event_order(left: &ContactEventV1, right: &ContactEventV1) -> Ordering {
    let (left_start, left_rank, left_end) = event_key(left);
    let (right_start, right_rank, right_end) = event_key(right);
    left_start
        .total_cmp(&right_start)
        .then(left_rank.cmp(&right_rank))
        .then_with(|| match (left_end, right_end) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => a.total_cmp(&b),
        })
        .then(utf16_cmp(role_name(left.role), role_name(right.role)))
        .then(utf16_cmp(phase_name(left.phase), phase_name(right.phase)))
        .then(utf16_cmp(&left.event_id, &right.event_id))
}
fn event_key(event: &ContactEventV1) -> (f64, u8, Option<f64>) {
    match event.kind {
        ContactEventKindV1::Point(time) => (time, 0, None),
        ContactEventKindV1::Window(window) => (window.start, 1, Some(window.end)),
    }
}
fn utf16_cmp(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}
fn role_name(value: ContactRoleV1) -> &'static str {
    match value {
        ContactRoleV1::LeftFoot => "left_foot",
        ContactRoleV1::RightFoot => "right_foot",
        ContactRoleV1::LeftHand => "left_hand",
        ContactRoleV1::RightHand => "right_hand",
        ContactRoleV1::LeftToe => "left_toe",
        ContactRoleV1::RightToe => "right_toe",
        ContactRoleV1::LeftKnee => "left_knee",
        ContactRoleV1::RightKnee => "right_knee",
        ContactRoleV1::LeftElbow => "left_elbow",
        ContactRoleV1::RightElbow => "right_elbow",
        ContactRoleV1::Root => "root",
        ContactRoleV1::Prop => "prop",
        ContactRoleV1::Body => "body",
    }
}
fn phase_name(value: ContactPhaseV1) -> &'static str {
    match value {
        ContactPhaseV1::Begin => "begin",
        ContactPhaseV1::End => "end",
        ContactPhaseV1::Marker => "marker",
    }
}

fn object(
    value: Value,
    field: &'static str,
) -> Result<serde_json::Map<String, Value>, ContactFragmentError> {
    match value {
        Value::Object(value) => Ok(value),
        _ => Err(ContactFragmentError::InvalidField {
            field,
            message: "must be an object".into(),
        }),
    }
}
fn array(value: Value, field: &'static str) -> Result<Vec<Value>, ContactFragmentError> {
    match value {
        Value::Array(value) => Ok(value),
        _ => Err(ContactFragmentError::InvalidField {
            field,
            message: "must be an array".into(),
        }),
    }
}
fn string(value: Value, field: &'static str) -> Result<String, ContactFragmentError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| ContactFragmentError::InvalidField {
            field,
            message: "must be a string".into(),
        })
}
fn number(value: Value, field: &'static str) -> Result<f64, ContactFragmentError> {
    value
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| ContactFragmentError::InvalidField {
            field,
            message: "must be a finite JSON number".into(),
        })
}
fn u32_value(value: Value, field: &'static str) -> Result<u32, ContactFragmentError> {
    Ok(unsigned_integer(value, u64::from(u32::MAX), field)? as u32)
}

fn unsigned_integer(
    value: Value,
    maximum: u64,
    field: &'static str,
) -> Result<u64, ContactFragmentError> {
    let Value::Number(number) = value else {
        return invalid(field, "must be a nonnegative integer-valued JSON number");
    };
    if let Some(value) = number.as_u64() {
        if value <= maximum {
            return Ok(value);
        }
    } else if let Some(value) = number.as_f64()
        && safe_f64(value)
        && value >= 0.0
        && value.fract() == 0.0
        && value <= maximum as f64
    {
        return Ok(value as u64);
    }
    invalid(field, "must be a nonnegative integer-valued JSON number")
}
fn take(
    map: &mut serde_json::Map<String, Value>,
    key: &'static str,
    field: &'static str,
) -> Result<Value, ContactFragmentError> {
    map.remove(key)
        .ok_or_else(|| ContactFragmentError::InvalidField {
            field,
            message: format!("is missing {key:?}"),
        })
}
fn exact_fields(
    map: &serde_json::Map<String, Value>,
    field: &'static str,
    allowed: &[&str],
) -> Result<(), ContactFragmentError> {
    for key in map.keys() {
        if !allowed.contains(&key.as_str()) {
            return invalid(field, "contains an unknown field");
        }
    }
    Ok(())
}
fn invalid<T>(field: &'static str, message: &'static str) -> Result<T, ContactFragmentError> {
    Err(ContactFragmentError::InvalidField {
        field,
        message: message.into(),
    })
}
fn limit<T>(field: &'static str, found: usize, limit: usize) -> Result<T, ContactFragmentError> {
    Err(ContactFragmentError::LimitExceeded {
        field,
        found,
        limit,
    })
}
fn text(value: &str, field: &'static str) -> Result<(), ContactFragmentError> {
    if value.is_empty() {
        return invalid(field, "must not be empty");
    }
    if value.len() > CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES {
        return limit(field, value.len(), CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES);
    }
    Ok(())
}
fn identifier(value: &str, field: &'static str) -> Result<(), ContactFragmentError> {
    text(value, field)?;
    if value.len() > CONTACT_FRAGMENT_V1_MAX_IDENTIFIER_BYTES {
        return limit(field, value.len(), CONTACT_FRAGMENT_V1_MAX_IDENTIFIER_BYTES);
    }
    Ok(())
}
fn positive_finite(value: f64, field: &'static str) -> Result<(), ContactFragmentError> {
    if !value.is_finite() || value <= 0.0 {
        return invalid(field, "must be finite and positive");
    }
    Ok(())
}
fn normalized_time(value: f64, field: &'static str) -> Result<(), ContactFragmentError> {
    finite_range(value, 0.0, 1.0, field)
}
fn finite_range(
    value: f64,
    min: f64,
    max: f64,
    field: &'static str,
) -> Result<(), ContactFragmentError> {
    if !value.is_finite() || value < min || value > max {
        return invalid(field, "must be finite and within its V1 range");
    }
    Ok(())
}
fn canonical_number(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn safe_identity_bytes(
    identity: &InputIdentity,
    field: &'static str,
) -> Result<(), ContactFragmentError> {
    if identity.bytes() > CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER {
        return invalid(field, "must be an RFC 8785 safe integer");
    }
    Ok(())
}

fn safe_i64(value: i64) -> bool {
    value.unsigned_abs() <= CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER
}

fn safe_u64(value: u64) -> bool {
    value <= CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER
}

fn safe_f64(value: f64) -> bool {
    value.is_finite() && value.abs() <= CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER as f64
}

fn validate_jcs_number(
    value: &serde_json::Number,
    field: &'static str,
) -> Result<(), ContactFragmentError> {
    if let Some(value) = value.as_i64() {
        if !safe_i64(value) {
            return invalid(field, "contains an RFC 8785 unsafe integer");
        }
    } else if let Some(value) = value.as_u64() {
        if !safe_u64(value) {
            return invalid(field, "contains an RFC 8785 unsafe integer");
        }
    } else if !value.as_f64().is_some_and(safe_f64) {
        return invalid(field, "contains a non-finite or RFC 8785 unsafe number");
    }
    Ok(())
}

fn validate_json_value(
    value: &Value,
    depth: usize,
    max_depth: usize,
    field: &'static str,
) -> Result<(), ContactFragmentError> {
    if matches!(value, Value::Array(_) | Value::Object(_)) && depth > max_depth {
        return limit(field, depth, max_depth);
    }
    match value {
        Value::String(value) => {
            if value.len() > CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES {
                return limit(field, value.len(), CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES);
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_json_value(value, depth + 1, max_depth, field)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if key.len() > CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES {
                    return limit(field, key.len(), CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES);
                }
                validate_json_value(value, depth + 1, max_depth, field)?;
            }
        }
        Value::Number(value) => validate_jcs_number(value, field)?,
        _ => {}
    }
    Ok(())
}

fn jcs<T: Serialize>(value: &T, limit: usize) -> Result<Vec<u8>, ContactFragmentError> {
    let mut output = CappedWriter {
        bytes: Vec::new(),
        limit,
        overflowed: false,
    };
    serde_jcs::to_writer(&mut output, value).map_err(|error| {
        if output.overflowed {
            ContactFragmentError::CanonicalTooLarge { limit }
        } else {
            ContactFragmentError::InvalidJson {
                message: error.to_string(),
            }
        }
    })?;
    Ok(output.bytes)
}
struct CappedWriter {
    bytes: Vec<u8>,
    limit: usize,
    overflowed: bool,
}
impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > self.limit {
            self.overflowed = true;
            return Err(io::Error::other("contact-fragment canonical limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) enum StrictJsonValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

struct CappedValues {
    values: Vec<StrictJsonValue>,
    overflowed: bool,
}

struct CappedValuesSeed {
    limit: usize,
    depth: usize,
}

struct CappedExtensionValuesSeed;

impl<'de> DeserializeSeed<'de> for CappedExtensionValuesSeed {
    type Value = CappedValues;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CappedExtensionValuesVisitor;

        impl<'de> Visitor<'de> for CappedExtensionValuesVisitor {
            type Value = CappedValues;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON array with bounded contact extensions")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(
                    sequence
                        .size_hint()
                        .unwrap_or(0)
                        .min(CONTACT_FRAGMENT_V1_MAX_EXTENSIONS),
                );
                while values.len() < CONTACT_FRAGMENT_V1_MAX_EXTENSIONS {
                    let Some(value) = sequence.next_element::<ContactExtensionWire>()? else {
                        return Ok(CappedValues {
                            values,
                            overflowed: false,
                        });
                    };
                    values.push(value.value);
                }
                let overflowed = sequence.next_element::<IgnoredAny>()?.is_some();
                if overflowed {
                    while sequence.next_element::<IgnoredAny>()?.is_some() {}
                }
                Ok(CappedValues { values, overflowed })
            }
        }

        deserializer.deserialize_seq(CappedExtensionValuesVisitor)
    }
}

struct ContactExtensionWire {
    value: StrictJsonValue,
}

impl<'de> Deserialize<'de> for ContactExtensionWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ContactExtensionVisitor;

        impl<'de> Visitor<'de> for ContactExtensionVisitor {
            type Value = ContactExtensionWire;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a strict bounded contact extension object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut fields = BTreeMap::new();
                while let Some(key) = map.next_key::<String>()? {
                    if fields.contains_key(&key) {
                        return Err(A::Error::custom(format!("duplicate object member {key:?}")));
                    }
                    let value = match key.as_str() {
                        "schema" | "schema_version" => {
                            map.next_value_seed(StrictJsonValueSeed { depth: 4 })?
                        }
                        "payload" => {
                            map.next_value_seed(MeasuredJsonValueSeed { depth: 1 })?
                                .value
                        }
                        _ => {
                            let _ = map.next_value::<IgnoredAny>()?;
                            return Err(A::Error::custom(
                                "contact extension contains an unknown field",
                            ));
                        }
                    };
                    fields.insert(key, value);
                }
                Ok(ContactExtensionWire {
                    value: StrictJsonValue::Object(fields),
                })
            }
        }

        deserializer.deserialize_map(ContactExtensionVisitor)
    }
}

impl<'de> DeserializeSeed<'de> for CappedValuesSeed {
    type Value = CappedValues;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CappedValuesVisitor {
            limit: usize,
            depth: usize,
        }

        impl<'de> Visitor<'de> for CappedValuesVisitor {
            type Value = CappedValues;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "a JSON array with at most {} values", self.limit)
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values =
                    Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(self.limit));
                while values.len() < self.limit {
                    let Some(value) =
                        sequence.next_element_seed(StrictJsonValueSeed { depth: self.depth })?
                    else {
                        return Ok(CappedValues {
                            values,
                            overflowed: false,
                        });
                    };
                    values.push(value);
                }
                let overflowed = sequence.next_element::<IgnoredAny>()?.is_some();
                if overflowed {
                    while sequence.next_element::<IgnoredAny>()?.is_some() {}
                }
                Ok(CappedValues { values, overflowed })
            }
        }

        deserializer.deserialize_seq(CappedValuesVisitor {
            limit: self.limit,
            depth: self.depth,
        })
    }
}

struct ContactFragmentWire {
    fields: BTreeMap<String, StrictJsonValue>,
    events: CappedValues,
    events_present: bool,
    extensions: CappedValues,
    extensions_present: bool,
}

impl ContactFragmentWire {
    fn into_value(self) -> Value {
        let mut fields = self
            .fields
            .into_iter()
            .map(|(key, value)| (key, value.into_value()))
            .collect::<serde_json::Map<_, _>>();
        if self.events_present {
            fields.insert(
                "events".into(),
                Value::Array(
                    self.events
                        .values
                        .into_iter()
                        .map(StrictJsonValue::into_value)
                        .collect(),
                ),
            );
        }
        if !self.extensions.values.is_empty() {
            fields.insert(
                "extensions".into(),
                Value::Array(
                    self.extensions
                        .values
                        .into_iter()
                        .map(StrictJsonValue::into_value)
                        .collect(),
                ),
            );
        }
        Value::Object(fields)
    }
}

impl<'de> Deserialize<'de> for ContactFragmentWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ContactFragmentVisitor;

        impl<'de> Visitor<'de> for ContactFragmentVisitor {
            type Value = ContactFragmentWire;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a strict contact-fragment V1 object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut fields = BTreeMap::new();
                let mut seen = BTreeSet::new();
                let mut events = None;
                let mut extensions = None;
                while let Some(key) = map.next_key::<String>()? {
                    if !seen.insert(key.clone()) {
                        return Err(A::Error::custom(format!("duplicate object member {key:?}")));
                    }
                    match key.as_str() {
                        "events" => {
                            events = Some(map.next_value_seed(CappedValuesSeed {
                                limit: CONTACT_FRAGMENT_V1_MAX_EVENTS,
                                depth: 3,
                            })?)
                        }
                        "extensions" => {
                            extensions = Some(map.next_value_seed(CappedExtensionValuesSeed)?)
                        }
                        "schema"
                        | "schema_version"
                        | "producer"
                        | "artifact"
                        | "dependency_closure_identity"
                        | "clip"
                        | "duration_s" => {
                            fields.insert(
                                key,
                                map.next_value_seed(StrictJsonValueSeed { depth: 2 })?,
                            );
                        }
                        _ => {
                            let _ = map.next_value::<IgnoredAny>()?;
                            return Err(A::Error::custom(
                                "contact fragment contains an unknown field",
                            ));
                        }
                    }
                }
                let extensions_present = extensions.is_some();
                Ok(ContactFragmentWire {
                    fields,
                    events_present: events.is_some(),
                    events: events.unwrap_or(CappedValues {
                        values: Vec::new(),
                        overflowed: false,
                    }),
                    extensions: extensions.unwrap_or(CappedValues {
                        values: Vec::new(),
                        overflowed: false,
                    }),
                    extensions_present,
                })
            }
        }

        deserializer.deserialize_map(ContactFragmentVisitor)
    }
}

pub(crate) struct StrictJsonValueSeed {
    pub(crate) depth: usize,
}

impl<'de> DeserializeSeed<'de> for StrictJsonValueSeed {
    type Value = StrictJsonValue;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictJsonValueVisitor { depth: self.depth })
    }
}

struct StrictJsonValueVisitor {
    depth: usize,
}
impl StrictJsonValue {
    pub(crate) fn into_value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(value) => Value::Bool(value),
            Self::Number(value) => Value::Number(value),
            Self::String(value) => Value::String(value),
            Self::Array(values) => Value::Array(values.into_iter().map(Self::into_value).collect()),
            Self::Object(values) => Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, value.into_value()))
                    .collect(),
            ),
        }
    }
}
impl<'de> Deserialize<'de> for StrictJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        StrictJsonValueSeed { depth: 1 }.deserialize(deserializer)
    }
}

impl<'de> Visitor<'de> for StrictJsonValueVisitor {
    type Value = StrictJsonValue;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JSON value")
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(StrictJsonValue::Null)
    }
    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(StrictJsonValue::Bool(value))
    }
    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
        if !safe_i64(value) {
            return Err(E::custom(
                "contact fragment contains an RFC 8785 unsafe integer",
            ));
        }
        Ok(StrictJsonValue::Number(value.into()))
    }
    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
        if !safe_u64(value) {
            return Err(E::custom(
                "contact fragment contains an RFC 8785 unsafe integer",
            ));
        }
        Ok(StrictJsonValue::Number(value.into()))
    }
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
        if !safe_f64(value) {
            return Err(E::custom(
                "contact fragment contains a non-finite or RFC 8785 unsafe number",
            ));
        }
        serde_json::Number::from_f64(value)
            .map(StrictJsonValue::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
        if value.len() > CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES {
            return Err(E::custom("contact fragment string exceeds V1 byte limit"));
        }
        Ok(StrictJsonValue::String(value.into()))
    }
    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
        if value.len() > CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES {
            return Err(E::custom("contact fragment string exceeds V1 byte limit"));
        }
        Ok(StrictJsonValue::String(value))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Self::Value, A::Error> {
        if self.depth > CONTACT_FRAGMENT_V1_MAX_DEPTH {
            return Err(A::Error::custom(
                "contact fragment JSON exceeds V1 nesting depth",
            ));
        }
        let mut output =
            Vec::with_capacity(values.size_hint().unwrap_or(0).min(MAX_OPAQUE_MEMBERS));
        while output.len() < MAX_OPAQUE_MEMBERS {
            let Some(value) = values.next_element_seed(StrictJsonValueSeed {
                depth: self.depth + 1,
            })?
            else {
                return Ok(StrictJsonValue::Array(output));
            };
            output.push(value);
        }
        let _ = values.next_element::<IgnoredAny>()?;
        Err(A::Error::custom(
            "contact fragment nested array exceeds V1 bounded member limit",
        ))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut values: A) -> Result<Self::Value, A::Error> {
        if self.depth > CONTACT_FRAGMENT_V1_MAX_DEPTH {
            return Err(A::Error::custom(
                "contact fragment JSON exceeds V1 nesting depth",
            ));
        }
        let mut output = BTreeMap::new();
        while output.len() < MAX_OPAQUE_MEMBERS {
            let Some(key) = values.next_key::<String>()? else {
                return Ok(StrictJsonValue::Object(output));
            };
            if key.len() > CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES {
                return Err(A::Error::custom(
                    "contact fragment object key exceeds V1 byte limit",
                ));
            }
            let value = values.next_value_seed(StrictJsonValueSeed {
                depth: self.depth + 1,
            })?;
            if output.insert(key.clone(), value).is_some() {
                return Err(A::Error::custom(format!("duplicate object member {key:?}")));
            }
        }
        let _ = values.next_key::<IgnoredAny>()?;
        Err(A::Error::custom(
            "contact fragment nested object exceeds V1 bounded member limit",
        ))
    }
}

/// A payload value plus its exact RFC 8785 byte count. This is deliberately
/// local to contact-fragment decoding: it keeps the payload budget enforced
/// while the parser is still deciding whether to retain a child.
struct MeasuredJsonValue {
    value: StrictJsonValue,
    canonical_len: usize,
}

struct MeasuredJsonValueSeed {
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for MeasuredJsonValueSeed {
    type Value = MeasuredJsonValue;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(MeasuredJsonValueVisitor { depth: self.depth })
    }
}

struct MeasuredJsonValueVisitor {
    depth: usize,
}

impl MeasuredJsonValue {
    fn scalar<E: serde::de::Error>(value: StrictJsonValue) -> Result<Self, E> {
        let json = match &value {
            StrictJsonValue::Null => Value::Null,
            StrictJsonValue::Bool(value) => Value::Bool(*value),
            StrictJsonValue::Number(value) => Value::Number(value.clone()),
            StrictJsonValue::String(value) => Value::String(value.clone()),
            StrictJsonValue::Array(_) | StrictJsonValue::Object(_) => {
                unreachable!("scalar measurement only receives scalars")
            }
        };
        let canonical_len = serde_jcs::to_vec(&json).map_err(E::custom)?.len();
        if canonical_len > CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_BYTES {
            return Err(E::custom(
                "contact extension payload exceeds V1 canonical byte limit",
            ));
        }
        Ok(Self {
            value,
            canonical_len,
        })
    }

    fn bounded<E: serde::de::Error>(canonical_len: usize) -> Result<(), E> {
        if canonical_len > CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_BYTES {
            return Err(E::custom(
                "contact extension payload exceeds V1 canonical byte limit",
            ));
        }
        Ok(())
    }
}

impl<'de> Visitor<'de> for MeasuredJsonValueVisitor {
    type Value = MeasuredJsonValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a bounded JSON extension payload value")
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        MeasuredJsonValue::scalar(StrictJsonValue::Null)
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
        MeasuredJsonValue::scalar(StrictJsonValue::Bool(value))
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
        if !safe_i64(value) {
            return Err(E::custom(
                "contact extension payload contains an RFC 8785 unsafe integer",
            ));
        }
        MeasuredJsonValue::scalar(StrictJsonValue::Number(value.into()))
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
        if !safe_u64(value) {
            return Err(E::custom(
                "contact extension payload contains an RFC 8785 unsafe integer",
            ));
        }
        MeasuredJsonValue::scalar(StrictJsonValue::Number(value.into()))
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
        if !safe_f64(value) {
            return Err(E::custom(
                "contact extension payload contains a non-finite or RFC 8785 unsafe number",
            ));
        }
        serde_json::Number::from_f64(value)
            .map(StrictJsonValue::Number)
            .ok_or_else(|| E::custom("non-finite number"))
            .and_then(MeasuredJsonValue::scalar)
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
        if value.len() > CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES {
            return Err(E::custom(
                "contact extension payload string exceeds V1 byte limit",
            ));
        }
        MeasuredJsonValue::scalar(StrictJsonValue::String(value.into()))
    }

    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
        if value.len() > CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES {
            return Err(E::custom(
                "contact extension payload string exceeds V1 byte limit",
            ));
        }
        MeasuredJsonValue::scalar(StrictJsonValue::String(value))
    }

    fn visit_seq<A>(self, mut values: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if self.depth > CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_DEPTH {
            return Err(A::Error::custom(
                "contact extension payload exceeds V1 nesting depth",
            ));
        }
        let mut output =
            Vec::with_capacity(values.size_hint().unwrap_or(0).min(MAX_OPAQUE_MEMBERS));
        let mut canonical_len = 2; // []
        while output.len() < MAX_OPAQUE_MEMBERS {
            let Some(child) = values.next_element_seed(MeasuredJsonValueSeed {
                depth: self.depth + 1,
            })?
            else {
                return Ok(MeasuredJsonValue {
                    value: StrictJsonValue::Array(output),
                    canonical_len,
                });
            };
            let candidate = canonical_len
                .saturating_add(child.canonical_len)
                .saturating_add(usize::from(!output.is_empty()));
            MeasuredJsonValue::bounded::<A::Error>(candidate)?;
            output.push(child.value);
            canonical_len = candidate;
        }
        let _ = values.next_element::<IgnoredAny>()?;
        Err(A::Error::custom(
            "contact extension payload nested array exceeds V1 bounded member limit",
        ))
    }

    fn visit_map<A>(self, mut values: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        if self.depth > CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_DEPTH {
            return Err(A::Error::custom(
                "contact extension payload exceeds V1 nesting depth",
            ));
        }
        let mut output = BTreeMap::new();
        let mut canonical_len = 2; // {}
        while output.len() < MAX_OPAQUE_MEMBERS {
            let Some(key) = values.next_key::<String>()? else {
                return Ok(MeasuredJsonValue {
                    value: StrictJsonValue::Object(output),
                    canonical_len,
                });
            };
            if key.len() > CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES {
                return Err(A::Error::custom(
                    "contact extension payload object key exceeds V1 byte limit",
                ));
            }
            if output.contains_key(&key) {
                let _ = values.next_value::<IgnoredAny>()?;
                return Err(A::Error::custom(format!("duplicate object member {key:?}")));
            }
            let child = values.next_value_seed(MeasuredJsonValueSeed {
                depth: self.depth + 1,
            })?;
            let key_len = serde_jcs::to_vec(&key).map_err(A::Error::custom)?.len();
            let candidate = canonical_len
                .saturating_add(key_len)
                .saturating_add(1) // colon
                .saturating_add(child.canonical_len)
                .saturating_add(usize::from(!output.is_empty()));
            MeasuredJsonValue::bounded::<A::Error>(candidate)?;
            output.insert(key, child.value);
            canonical_len = candidate;
        }
        let _ = values.next_key::<IgnoredAny>()?;
        Err(A::Error::custom(
            "contact extension payload nested object exceeds V1 bounded member limit",
        ))
    }
}
