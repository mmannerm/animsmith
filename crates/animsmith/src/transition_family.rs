//! Strict bounded TOML decoding for transition-family declaration V1.
//!
//! This module deliberately stops at declaration parsing. It does not add a
//! `Config` field, evaluator, collection command, or source resolver.
//! Its closed TOML grammar spells document families and all members as tables
//! or arrays-of-tables; equivalent inline aggregate maps/arrays are refused by
//! lexical preflight before typed TOML decoding.

use animsmith_core::{
    COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES, CollectionIdV1, CollectionLogicalIdV1,
    CollectionSourceKeyV1, CollectionTransitionFamilyMemberV1, CollectionTransitionFamilyV1,
    DocumentTransitionFamilyMemberV1, DocumentTransitionFamilyV1, InputIdentity,
    TRANSITION_FAMILY_V1_ID, TRANSITION_FAMILY_V1_MAX_DEPTH,
    TRANSITION_FAMILY_V1_MAX_DOCUMENT_FAMILY_ID_BYTES, TRANSITION_FAMILY_V1_MAX_FAMILIES,
    TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY, TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES,
    TRANSITION_FAMILY_V1_MAX_STRING_BYTES, TRANSITION_FAMILY_V1_SCHEMA_VERSION,
    TransitionFamilyBoundaryV1, TransitionFamilyDeclarationInputV1, TransitionFamilyDeclarationV1,
    TransitionFamilyManifestIdentityV1, TransitionFamilyTolerancesV1,
};
use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Typed control-error class for the declaration reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransitionFamilyControlKind {
    TooLarge,
    Encoding,
    Depth,
    NormalizedTooLarge,
    Malformed,
    UnsupportedSchema,
    UnsupportedSchemaVersion,
    InvalidDeclaration,
}

impl TransitionFamilyControlKind {
    fn label(self) -> &'static str {
        match self {
            Self::TooLarge => "transition-family-too-large",
            Self::Encoding => "transition-family-encoding",
            Self::Depth => "transition-family-depth",
            Self::NormalizedTooLarge => "transition-family-normalized-too-large",
            Self::Malformed => "transition-family-malformed",
            Self::UnsupportedSchema => "transition-family-unsupported-schema",
            Self::UnsupportedSchemaVersion => "transition-family-unsupported-schema-version",
            Self::InvalidDeclaration => "transition-family-invalid-declaration",
        }
    }
}

/// A strict transition-family declaration control error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransitionFamilyControlError {
    kind: TransitionFamilyControlKind,
}

impl TransitionFamilyControlError {
    fn new(kind: TransitionFamilyControlKind) -> Self {
        Self { kind }
    }
    #[cfg(test)]
    fn kind(self) -> TransitionFamilyControlKind {
        self.kind
    }
}

impl fmt::Display for TransitionFamilyControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "transition-family declaration control error ({})",
            self.kind.label()
        )
    }
}
impl std::error::Error for TransitionFamilyControlError {}

/// Parse document-owned `[transition_families."<id>"]` tables from exact config bytes.
///
/// The source identity covers all `bytes`, while the normalized identity covers
/// only the closed transition-family declaration. An absent table is a valid,
/// empty document declaration.
#[allow(dead_code, reason = "declaration slice precedes Config and CLI wiring")]
pub(crate) fn parse_document_transition_families_bytes(
    bytes: &[u8],
) -> Result<TransitionFamilyDeclarationInputV1, TransitionFamilyControlError> {
    let text = preflight_toml(bytes, DeclarationScope::Document)?;
    // This deserializes only the transition-family subtree. Serde skips
    // unrelated root fields without materializing an untyped full config;
    // nevertheless the preflight depth bound applies to the exact full source.
    let root = toml::from_str::<DocumentConfigWire>(text).map_err(|_| invalid())?;
    let families = root
        .transition_families
        .map_or_else(BTreeMap::new, |value| value.0);
    let decoded = families
        .into_iter()
        .map(|(family_id, family)| decode_document_family(family_id, family))
        .collect::<Result<Vec<_>, _>>()?;
    bind_document(decoded, bytes)
}

/// Parse one complete collection-owned transition-family declaration envelope.
#[allow(
    dead_code,
    reason = "declaration slice precedes collection evaluator and CLI wiring"
)]
pub(crate) fn parse_collection_transition_families_bytes(
    bytes: &[u8],
) -> Result<TransitionFamilyDeclarationInputV1, TransitionFamilyControlError> {
    let text = preflight_toml(bytes, DeclarationScope::Collection)?;
    let header = toml::from_str::<CollectionHeaderWire>(text)
        .map_err(|_| TransitionFamilyControlError::new(TransitionFamilyControlKind::Malformed))?;
    classify_header(&header.schema, header.schema_version)?;
    let wire = toml::from_str::<CollectionEnvelopeWire>(text).map_err(|_| invalid())?;
    classify_header(&wire.schema, wire.schema_version)?;
    if wire.scope != "collection" {
        return Err(TransitionFamilyControlError::new(
            TransitionFamilyControlKind::InvalidDeclaration,
        ));
    }
    let collection_id = CollectionIdV1::new(wire.collection_id).map_err(|_| invalid())?;
    let manifest = TransitionFamilyManifestIdentityV1::new(
        collection_id,
        InputIdentity::from_sha256_digest(
            decode_digest(&wire.manifest_input_identity.sha256)?,
            wire.manifest_input_identity.bytes,
        ),
    )
    .map_err(|_| invalid())?;
    let decoded = wire
        .families
        .into_iter()
        .map(decode_collection_family)
        .collect::<Result<Vec<_>, _>>()?;
    let declaration =
        TransitionFamilyDeclarationV1::collection(manifest, decoded).map_err(|_| invalid())?;
    TransitionFamilyDeclarationInputV1::new(declaration, bytes).map_err(classify_core_error)
}

fn bind_document(
    families: Vec<DocumentTransitionFamilyV1>,
    bytes: &[u8],
) -> Result<TransitionFamilyDeclarationInputV1, TransitionFamilyControlError> {
    let declaration = TransitionFamilyDeclarationV1::document(families).map_err(|_| invalid())?;
    TransitionFamilyDeclarationInputV1::new(declaration, bytes).map_err(classify_core_error)
}

fn decode_document_family(
    family_id: String,
    wire: DocumentFamilyWire,
) -> Result<DocumentTransitionFamilyV1, TransitionFamilyControlError> {
    classify_header(&wire.schema, wire.schema_version)?;
    if wire.scope != "document" {
        return Err(invalid());
    }
    DocumentTransitionFamilyV1::new(
        family_id,
        decode_boundary(&wire.boundary)?,
        decode_tolerances(&wire.basis, &wire.tolerances)?,
        wire.members
            .into_iter()
            .map(|member| {
                DocumentTransitionFamilyMemberV1::new(member.take_index, member.take_name)
                    .map_err(|_| invalid())
            })
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|_| invalid())
}

fn decode_collection_family(
    wire: CollectionFamilyWire,
) -> Result<CollectionTransitionFamilyV1, TransitionFamilyControlError> {
    CollectionTransitionFamilyV1::new(
        CollectionLogicalIdV1::new(wire.family_id).map_err(|_| invalid())?,
        decode_boundary(&wire.boundary)?,
        decode_tolerances(&wire.basis, &wire.tolerances)?,
        wire.members
            .into_iter()
            .map(|member| {
                CollectionTransitionFamilyMemberV1::new(
                    CollectionLogicalIdV1::new(member.logical_id).map_err(|_| invalid())?,
                    CollectionSourceKeyV1::new(member.source).map_err(|_| invalid())?,
                    member.take_index,
                    member.take_name,
                )
                .map_err(|_| invalid())
            })
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|_| invalid())
}

fn decode_boundary(
    value: &str,
) -> Result<TransitionFamilyBoundaryV1, TransitionFamilyControlError> {
    match value {
        "entry" => Ok(TransitionFamilyBoundaryV1::Entry),
        "exit" => Ok(TransitionFamilyBoundaryV1::Exit),
        "both" => Ok(TransitionFamilyBoundaryV1::Both),
        _ => Err(invalid()),
    }
}

fn decode_tolerances(
    basis: &BasisWire,
    tolerances: &TolerancesWire,
) -> Result<TransitionFamilyTolerancesV1, TransitionFamilyControlError> {
    if basis.translation != "skeleton-local-metres"
        || basis.rotation != "skeleton-local-degrees"
        || basis.time != "normalized-clip"
    {
        return Err(invalid());
    }
    TransitionFamilyTolerancesV1::new(
        tolerances.translation_m,
        tolerances.rotation_deg,
        tolerances.time_normalized,
    )
    .map_err(|_| invalid())
}

fn classify_header(schema: &str, schema_version: u32) -> Result<(), TransitionFamilyControlError> {
    if schema != TRANSITION_FAMILY_V1_ID {
        return Err(TransitionFamilyControlError::new(
            TransitionFamilyControlKind::UnsupportedSchema,
        ));
    }
    if schema_version != TRANSITION_FAMILY_V1_SCHEMA_VERSION {
        return Err(TransitionFamilyControlError::new(
            TransitionFamilyControlKind::UnsupportedSchemaVersion,
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum DeclarationScope {
    Document,
    Collection,
}

fn preflight_toml(
    bytes: &[u8],
    scope: DeclarationScope,
) -> Result<&str, TransitionFamilyControlError> {
    if bytes.len() as u64 > TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES {
        return Err(TransitionFamilyControlError::new(
            TransitionFamilyControlKind::TooLarge,
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| TransitionFamilyControlError::new(TransitionFamilyControlKind::Encoding))?;
    TomlFirstPass::new(scope).scan(text)?;
    Ok(text)
}

/// TOML-aware bounded first pass over the exact source text.
///
/// The pass deliberately handles only lexical structure, never values. It
/// therefore runs before serde/TOML can retain a deep or over-cap declaration.
/// The normal TOML parser remains responsible for semantic syntax errors.
struct TomlFirstPass {
    scope: DeclarationScope,
    table_depths: BTreeMap<Vec<String>, usize>,
    current_path: Vec<String>,
    current_table_depth: usize,
    document_family_ids: BTreeSet<String>,
    document_members: BTreeMap<String, usize>,
    collection_family_count: usize,
    collection_current_family: Option<usize>,
    aggregate_members: usize,
}

impl TomlFirstPass {
    fn new(scope: DeclarationScope) -> Self {
        Self {
            scope,
            table_depths: BTreeMap::new(),
            current_path: Vec::new(),
            current_table_depth: 1,
            document_family_ids: BTreeSet::new(),
            document_members: BTreeMap::new(),
            collection_family_count: 0,
            collection_current_family: None,
            aggregate_members: 0,
        }
    }

    fn scan(mut self, text: &str) -> Result<(), TransitionFamilyControlError> {
        let bytes = text.as_bytes();
        let mut index = 0;
        let mut line_start = true;
        let mut value = ValueScan::idle();
        while index < bytes.len() {
            if value.active() {
                index = value.scan(bytes, index)?;
                line_start = value.at_line_start;
                continue;
            }
            if line_start {
                let start = skip_horizontal(bytes, index);
                if start == bytes.len() {
                    break;
                }
                if bytes[start] == b'\n' {
                    index = start + 1;
                    continue;
                }
                if bytes[start] == b'#' {
                    index = skip_comment(bytes, start);
                    line_start = true;
                    continue;
                }
                if bytes[start] == b'[' {
                    let (path, array_table, after) = parse_header(bytes, start)?;
                    self.header(path, array_table)?;
                    index = skip_to_line_end(bytes, after);
                    line_start = true;
                    continue;
                }
                if let Some((path, value_start)) =
                    parse_assignment(bytes, start, &self.current_path)?
                {
                    let (full_path, base) = self.assignment_base_depth(&path)?;
                    if self.noncanonical_inline_form(bytes, value_start, &full_path) {
                        // The closed V1 TOML grammar uses document family
                        // tables and collection/member arrays-of-tables. The
                        // equivalent inline aggregate forms are refused here,
                        // before TOML can retain their unbounded maps/arrays.
                        return Err(invalid());
                    }
                    value = ValueScan::new(base, self.field_string_limit(&full_path));
                    index = value_start;
                    line_start = false;
                    continue;
                }
            }
            if bytes[index] == b'\n' {
                line_start = true;
            } else if !matches!(bytes[index], b' ' | b'\t' | b'\r') {
                line_start = false;
            }
            index += 1;
        }
        Ok(())
    }

    fn header(
        &mut self,
        path: Vec<String>,
        array_table: bool,
    ) -> Result<(), TransitionFamilyControlError> {
        let mut depth = 1;
        let mut prefix = Vec::new();
        for (index, component) in path.iter().enumerate() {
            prefix.push(component.clone());
            let known = self.table_depths.get(&prefix);
            if let Some(known) = known {
                depth = *known;
            } else {
                depth += 1;
                if index + 1 == path.len() && array_table {
                    depth += 1;
                }
                if depth > TRANSITION_FAMILY_V1_MAX_DEPTH {
                    return Err(depth_error());
                }
                self.table_depths.insert(prefix.clone(), depth);
                // TOML materializes omitted parent tables. Count an implicit
                // document family here so a stream of member-array headers
                // cannot evade the family cap by never spelling its parent.
                if matches!(self.scope, DeclarationScope::Document)
                    && prefix.len() == 2
                    && prefix[0] == "transition_families"
                {
                    self.bump_document_family(&prefix[1])?;
                }
            }
        }
        self.current_table_depth = depth;
        self.current_path = path.clone();
        self.table_depths.insert(path.clone(), depth);
        match self.scope {
            DeclarationScope::Document => {
                if array_table
                    && path.len() == 3
                    && path[0] == "transition_families"
                    && path[2] == "members"
                {
                    self.bump_document_member(&path[1])?;
                }
            }
            DeclarationScope::Collection => {
                if array_table && path.as_slice() == ["families"] {
                    self.bump_collection_family()?;
                    self.collection_current_family = Some(self.collection_family_count);
                }
                if array_table && path.as_slice() == ["families", "members"] {
                    // Collection members have no implicit owner in the
                    // closed V1 grammar. Reject before TOML can retain even
                    // the first orphan header; otherwise a near-source-cap
                    // stream of these headers bypasses every member budget.
                    let family = self.collection_current_family.ok_or_else(invalid)?;
                    self.bump_collection_member(family)?;
                }
            }
        }
        Ok(())
    }

    fn assignment_base_depth(
        &mut self,
        path: &[String],
    ) -> Result<(Vec<String>, usize), TransitionFamilyControlError> {
        let mut full_path = self.current_path.clone();
        full_path.extend_from_slice(path);
        let mut depth = 1_usize;
        let mut prefix = Vec::new();
        // A dotted assignment's final component names a value. Every earlier
        // component introduces (or reuses) an implicit table. Record those
        // tables so depth and document-family caps also cover valid dotted
        // assignment spelling rather than only explicit headers.
        for component in full_path.iter().take(full_path.len().saturating_sub(1)) {
            prefix.push(component.clone());
            if let Some(known) = self.table_depths.get(&prefix) {
                depth = *known;
            } else {
                depth = depth.checked_add(1).ok_or_else(depth_error)?;
                if depth > TRANSITION_FAMILY_V1_MAX_DEPTH {
                    return Err(depth_error());
                }
                self.table_depths.insert(prefix.clone(), depth);
                if matches!(self.scope, DeclarationScope::Document)
                    && prefix.len() == 2
                    && prefix[0] == "transition_families"
                {
                    self.bump_document_family(&prefix[1])?;
                }
            }
        }
        Ok((full_path, depth))
    }

    fn noncanonical_inline_form(&self, bytes: &[u8], value_start: usize, path: &[String]) -> bool {
        let first = bytes.get(skip_horizontal(bytes, value_start));
        match self.scope {
            DeclarationScope::Document => {
                (path.len() == 1 && path[0] == "transition_families" && first == Some(&b'{'))
                    || (path.len() == 2 && path[0] == "transition_families" && first == Some(&b'{'))
                    || (path.len() == 3 && path[0] == "transition_families" && path[2] == "members")
            }
            DeclarationScope::Collection => {
                (path == ["families"] && first == Some(&b'[')) || path == ["families", "members"]
            }
        }
    }

    fn field_string_limit(&self, path: &[String]) -> usize {
        if matches!(self.scope, DeclarationScope::Collection) {
            match path {
                [field] if field == "collection_id" => COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES,
                [families, field] if families == "families" && field == "family_id" => {
                    COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES
                }
                [families, members, field]
                    if families == "families"
                        && members == "members"
                        && matches!(field.as_str(), "logical_id" | "source") =>
                {
                    COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES
                }
                _ => TRANSITION_FAMILY_V1_MAX_STRING_BYTES,
            }
        } else {
            TRANSITION_FAMILY_V1_MAX_STRING_BYTES
        }
    }

    fn bump_document_family(&mut self, family: &str) -> Result<(), TransitionFamilyControlError> {
        if !self.document_family_ids.insert(family.to_owned()) {
            return Ok(());
        }
        if self.document_family_ids.len() > TRANSITION_FAMILY_V1_MAX_FAMILIES {
            return Err(invalid());
        }
        Ok(())
    }

    fn bump_collection_family(&mut self) -> Result<(), TransitionFamilyControlError> {
        let next = self.collection_family_count + 1;
        if next > TRANSITION_FAMILY_V1_MAX_FAMILIES {
            return Err(invalid());
        }
        self.collection_family_count = next;
        Ok(())
    }

    fn bump_document_member(&mut self, family: &str) -> Result<(), TransitionFamilyControlError> {
        let count = {
            let count = self.document_members.entry(family.to_owned()).or_default();
            *count += 1;
            *count
        };
        self.check_member_budget(count)
    }

    fn bump_collection_member(
        &mut self,
        family: usize,
    ) -> Result<(), TransitionFamilyControlError> {
        let key = family.to_string();
        let count = {
            let count = self.document_members.entry(key).or_default();
            *count += 1;
            *count
        };
        self.check_member_budget(count)
    }

    fn check_member_budget(&mut self, count: usize) -> Result<(), TransitionFamilyControlError> {
        if count > TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY {
            return Err(invalid());
        }
        self.aggregate_members += 1;
        if self.aggregate_members > animsmith_core::TRANSITION_FAMILY_V1_MAX_AGGREGATE_MEMBERS {
            return Err(invalid());
        }
        Ok(())
    }
}

fn depth_error() -> TransitionFamilyControlError {
    TransitionFamilyControlError::new(TransitionFamilyControlKind::Depth)
}

struct ValueScan {
    base_depth: usize,
    string_limit: usize,
    containers: usize,
    state: StringState,
    at_line_start: bool,
    inline_tables: Vec<InlineTableFrame>,
}

struct InlineTableFrame {
    container_depth: usize,
    key_start: usize,
}

impl ValueScan {
    fn idle() -> Self {
        Self {
            base_depth: 1,
            string_limit: TRANSITION_FAMILY_V1_MAX_STRING_BYTES,
            containers: 0,
            state: StringState::None,
            at_line_start: true,
            inline_tables: Vec::new(),
        }
    }
    fn new(base_depth: usize, string_limit: usize) -> Self {
        Self {
            base_depth,
            string_limit,
            containers: 0,
            state: StringState::None,
            at_line_start: false,
            inline_tables: Vec::new(),
        }
    }
    fn active(&self) -> bool {
        self.containers != 0 || !matches!(self.state, StringState::None) || !self.at_line_start
    }
    fn scan(
        &mut self,
        bytes: &[u8],
        mut index: usize,
    ) -> Result<usize, TransitionFamilyControlError> {
        while index < bytes.len() {
            if let Some(after) = self.state.consume(bytes, index) {
                index = after;
                continue;
            }
            match bytes[index] {
                b'#' => {
                    index = skip_comment(bytes, index);
                    self.at_line_start = true;
                    if self.containers == 0 {
                        return Ok(index);
                    }
                    continue;
                }
                b'"' => {
                    index = scan_toml_string(bytes, index, self.string_limit)?;
                    self.at_line_start = false;
                    continue;
                }
                b'\'' => {
                    index = scan_toml_string(bytes, index, self.string_limit)?;
                    self.at_line_start = false;
                    continue;
                }
                b'[' | b'{' => {
                    self.containers += 1;
                    if self.base_depth + self.containers > TRANSITION_FAMILY_V1_MAX_DEPTH {
                        return Err(depth_error());
                    }
                    if bytes[index] == b'{' {
                        self.inline_tables.push(InlineTableFrame {
                            container_depth: self.containers,
                            key_start: index + 1,
                        });
                    }
                }
                b'}' => {
                    self.inline_tables.pop();
                    self.containers = self.containers.saturating_sub(1);
                }
                b']' => self.containers = self.containers.saturating_sub(1),
                b'=' => {
                    if let Some(frame) = self.inline_tables.last()
                        && frame.container_depth == self.containers
                    {
                        let key = parse_key_path(&bytes[frame.key_start..index], &[])?;
                        let key_depth = self
                            .base_depth
                            .checked_add(frame.container_depth)
                            .and_then(|depth| depth.checked_add(key.len().saturating_sub(1)))
                            .ok_or_else(depth_error)?;
                        if key_depth > TRANSITION_FAMILY_V1_MAX_DEPTH {
                            return Err(depth_error());
                        }
                    }
                }
                b',' => {
                    if let Some(frame) = self.inline_tables.last_mut()
                        && frame.container_depth == self.containers
                    {
                        frame.key_start = index + 1;
                    }
                }
                b'\n' => {
                    self.at_line_start = true;
                    if self.containers == 0 {
                        return Ok(index + 1);
                    }
                }
                _ => self.at_line_start = false,
            }
            index += 1;
        }
        Ok(index)
    }
}

#[derive(Clone, Copy)]
enum StringState {
    None,
    Basic { multi: bool, open: usize },
    Literal { multi: bool, open: usize },
}
impl StringState {
    fn start_basic(bytes: &[u8], index: usize) -> Self {
        let multi = bytes.get(index..index + 3) == Some(b"\"\"\"");
        Self::Basic {
            multi,
            open: if multi { 3 } else { 1 },
        }
    }
    fn start_literal(bytes: &[u8], index: usize) -> Self {
        let multi = bytes.get(index..index + 3) == Some(b"'''");
        Self::Literal {
            multi,
            open: if multi { 3 } else { 1 },
        }
    }
    fn open_len(self) -> usize {
        match self {
            Self::Basic { open, .. } | Self::Literal { open, .. } => open,
            Self::None => 0,
        }
    }
    fn consume(&mut self, bytes: &[u8], index: usize) -> Option<usize> {
        match *self {
            Self::None => None,
            Self::Basic { multi: false, .. } => {
                if bytes[index] == b'\\' {
                    return Some((index + 2).min(bytes.len()));
                }
                if bytes[index] == b'"' {
                    *self = Self::None;
                }
                Some(index + 1)
            }
            Self::Literal { multi: false, .. } => {
                if bytes[index] == b'\'' {
                    *self = Self::None;
                }
                Some(index + 1)
            }
            Self::Basic { multi: true, .. } => {
                if bytes[index] == b'\\' {
                    return Some((index + 2).min(bytes.len()));
                }
                if bytes[index] == b'"' && quote_run(bytes, index, b'"') >= 3 {
                    *self = Self::None;
                    return Some(index + quote_run(bytes, index, b'"'));
                }
                Some(index + 1)
            }
            Self::Literal { multi: true, .. } => {
                if bytes[index] == b'\'' && quote_run(bytes, index, b'\'') >= 3 {
                    *self = Self::None;
                    return Some(index + quote_run(bytes, index, b'\''));
                }
                Some(index + 1)
            }
        }
    }
}

fn quote_run(bytes: &[u8], index: usize, quote: u8) -> usize {
    let mut end = index;
    while bytes.get(end) == Some(&quote) {
        end += 1;
    }
    end - index
}

/// Scan one TOML basic or literal string and bound its decoded UTF-8 bytes
/// before serde can allocate its `String`. Basic escapes use their decoded
/// scalar byte length; multiline backslash-newline continuations contribute no
/// decoded bytes.
fn scan_toml_string(
    bytes: &[u8],
    start: usize,
    maximum: usize,
) -> Result<usize, TransitionFamilyControlError> {
    let quote = bytes[start];
    let basic = quote == b'"';
    let multi = bytes.get(start..start + 3) == Some(if basic { b"\"\"\"" } else { b"'''" });
    let mut index = start + if multi { 3 } else { 1 };
    // TOML trims one immediate LF or CRLF after a multiline opening
    // delimiter. Do it before accounting decoded bytes so this first pass
    // exactly matches the decoder's authored-string length.
    if multi {
        if bytes.get(index..index + 2) == Some(b"\r\n") {
            index += 2;
        } else if bytes.get(index) == Some(&b'\n') {
            index += 1;
        }
    }
    let mut decoded = 0_usize;
    while index < bytes.len() {
        if multi && bytes[index] == quote {
            let run = quote_run(bytes, index, quote);
            if run >= 3 {
                decoded = decoded.checked_add(run - 3).ok_or_else(string_too_large)?;
                if decoded > maximum {
                    return Err(string_too_large());
                }
                return Ok(index + run);
            }
        } else if !multi && bytes[index] == quote {
            return Ok(index + 1);
        }
        if basic && bytes[index] == b'\\' {
            index += 1;
            let escaped = *bytes.get(index).ok_or_else(malformed_error)?;
            if multi && matches!(escaped, b' ' | b'\t' | b'\r' | b'\n') {
                while matches!(bytes.get(index), Some(b' ' | b'\t' | b'\r' | b'\n')) {
                    index += 1;
                }
                continue;
            }
            let added = match escaped {
                b'b' | b't' | b'n' | b'f' | b'r' | b'"' | b'\\' => 1,
                b'u' => escaped_unicode_len(bytes, index + 1, 4)?,
                b'U' => escaped_unicode_len(bytes, index + 1, 8)?,
                _ => return malformed(),
            };
            decoded = decoded.checked_add(added).ok_or_else(string_too_large)?;
            if decoded > maximum {
                return Err(string_too_large());
            }
            index += match escaped {
                b'u' => 5,
                b'U' => 9,
                _ => 1,
            };
            continue;
        }
        decoded = decoded.checked_add(1).ok_or_else(string_too_large)?;
        if decoded > maximum {
            return Err(string_too_large());
        }
        index += 1;
    }
    malformed()
}

fn escaped_unicode_len(
    bytes: &[u8],
    start: usize,
    digits: usize,
) -> Result<usize, TransitionFamilyControlError> {
    let mut value = 0_u32;
    for offset in 0..digits {
        let digit = bytes
            .get(start + offset)
            .and_then(|byte| char::from(*byte).to_digit(16))
            .ok_or_else(malformed_error)?;
        value = value
            .checked_mul(16)
            .and_then(|current| current.checked_add(digit))
            .ok_or_else(malformed_error)?;
    }
    char::from_u32(value)
        .map(|character| character.len_utf8())
        .ok_or_else(malformed_error)
}

fn string_too_large() -> TransitionFamilyControlError {
    TransitionFamilyControlError::new(TransitionFamilyControlKind::TooLarge)
}

fn skip_horizontal(bytes: &[u8], mut index: usize) -> usize {
    while matches!(bytes.get(index), Some(b' ' | b'\t' | b'\r')) {
        index += 1;
    }
    index
}
fn skip_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    if index < bytes.len() {
        index + 1
    } else {
        index
    }
}
fn skip_to_line_end(bytes: &[u8], index: usize) -> usize {
    skip_comment(bytes, index)
}

fn parse_header(
    bytes: &[u8],
    start: usize,
) -> Result<(Vec<String>, bool, usize), TransitionFamilyControlError> {
    let array = bytes.get(start + 1) == Some(&b'[');
    let open = start + usize::from(array);
    let mut index = open + 1;
    let mut state = StringState::None;
    while index < bytes.len() {
        if let Some(after) = state.consume(bytes, index) {
            index = after;
            continue;
        }
        match bytes[index] {
            b'"' => {
                state = StringState::start_basic(bytes, index);
                index += state.open_len();
            }
            b'\'' => {
                state = StringState::start_literal(bytes, index);
                index += state.open_len();
            }
            b']' => {
                if !array || bytes.get(index + 1) == Some(&b']') {
                    return Ok((
                        parse_key_path(&bytes[open + 1..index], &[])?,
                        array,
                        index + 1 + usize::from(array),
                    ));
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    malformed()
}

fn parse_assignment(
    bytes: &[u8],
    start: usize,
    current_path: &[String],
) -> Result<Option<(Vec<String>, usize)>, TransitionFamilyControlError> {
    let mut index = start;
    let mut state = StringState::None;
    while index < bytes.len() && bytes[index] != b'\n' {
        if let Some(after) = state.consume(bytes, index) {
            index = after;
            continue;
        }
        match bytes[index] {
            b'"' => {
                state = StringState::start_basic(bytes, index);
                index += state.open_len();
            }
            b'\'' => {
                state = StringState::start_literal(bytes, index);
                index += state.open_len();
            }
            b'#' => return Ok(None),
            b'=' => {
                return Ok(Some((
                    parse_key_path(&bytes[start..index], current_path)?,
                    index + 1,
                )));
            }
            _ => index += 1,
        }
    }
    Ok(None)
}

fn parse_key_path(
    source: &[u8],
    current_path: &[String],
) -> Result<Vec<String>, TransitionFamilyControlError> {
    let mut path = Vec::new();
    let mut index = 0;
    loop {
        // No accepted V1 path can have more than sixteen components: the
        // root itself consumes one depth level. Refuse component seventeen
        // while scanning so a hostile dotted key cannot create a quadratic
        // prefix map or an unbounded component vector.
        if path.len() == TRANSITION_FAMILY_V1_MAX_DEPTH {
            return Err(depth_error());
        }
        index = skip_horizontal(source, index);
        if index >= source.len() {
            break;
        }
        let start = index;
        let maximum = key_component_limit(current_path, &path);
        if matches!(source[index], b'"' | b'\'') {
            let quote = source[index];
            index += 1;
            let content = index;
            while index < source.len() && source[index] != quote {
                if quote == b'"' && source[index] == b'\\' {
                    index += 1;
                }
                index += 1;
            }
            if index >= source.len() {
                return malformed();
            }
            let component = if quote == b'"' {
                decode_basic_key(&source[content..index], maximum)?
            } else {
                if index - content > maximum {
                    return Err(string_too_large());
                }
                let raw = std::str::from_utf8(&source[content..index]).map_err(|_| {
                    TransitionFamilyControlError::new(TransitionFamilyControlKind::Encoding)
                })?;
                raw.to_owned()
            };
            path.push(component);
            index += 1;
        } else {
            while index < source.len() && !matches!(source[index], b'.' | b' ' | b'\t' | b'\r') {
                index += 1;
            }
            if index - start > maximum {
                return Err(string_too_large());
            }
            let component = std::str::from_utf8(&source[start..index]).map_err(|_| {
                TransitionFamilyControlError::new(TransitionFamilyControlKind::Encoding)
            })?;
            path.push(component.to_owned());
        }
        index = skip_horizontal(source, index);
        if index >= source.len() {
            break;
        }
        if source[index] != b'.' {
            return malformed();
        }
        index += 1;
    }
    if path.is_empty() || path.iter().any(String::is_empty) {
        malformed()
    } else {
        Ok(path)
    }
}

/// The second component under `transition_families` is the document-local
/// family id. All other TOML keys use the V1 generic authored-string bound.
fn key_component_limit(current_path: &[String], parsed: &[String]) -> usize {
    let preceding = current_path.len() + parsed.len();
    let first = current_path.first().or_else(|| parsed.first());
    if preceding == 1 && first.is_some_and(|component| component == "transition_families") {
        TRANSITION_FAMILY_V1_MAX_DOCUMENT_FAMILY_ID_BYTES
    } else {
        TRANSITION_FAMILY_V1_MAX_STRING_BYTES
    }
}

/// Decode the TOML basic-string escapes permitted in a quoted key.  The
/// lexical first pass needs semantic component equality for declarations such
/// as `"famil\\u0069es"`; the TOML decoder performs the final syntax check.
fn decode_basic_key(source: &[u8], maximum: usize) -> Result<String, TransitionFamilyControlError> {
    let text = std::str::from_utf8(source)
        .map_err(|_| TransitionFamilyControlError::new(TransitionFamilyControlKind::Encoding))?;
    let mut output = String::new();
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            push_bounded_key_character(&mut output, character, maximum)?;
            continue;
        }
        let escaped = chars.next().ok_or_else(invalid)?;
        let character = match escaped {
            'b' => '\u{0008}',
            't' => '\t',
            'n' => '\n',
            'f' => '\u{000C}',
            'r' => '\r',
            '"' => '"',
            '\\' => '\\',
            'u' => decode_unicode_escape(&mut chars, 4)?,
            'U' => decode_unicode_escape(&mut chars, 8)?,
            _ => return malformed(),
        };
        push_bounded_key_character(&mut output, character, maximum)?;
    }
    Ok(output)
}

fn push_bounded_key_character(
    output: &mut String,
    character: char,
    maximum: usize,
) -> Result<(), TransitionFamilyControlError> {
    if output
        .len()
        .checked_add(character.len_utf8())
        .is_none_or(|length| length > maximum)
    {
        return Err(string_too_large());
    }
    output.push(character);
    Ok(())
}

fn decode_unicode_escape(
    chars: &mut std::str::Chars<'_>,
    digits: usize,
) -> Result<char, TransitionFamilyControlError> {
    let mut value = 0_u32;
    for _ in 0..digits {
        let character = chars.next().ok_or_else(invalid)?;
        value = value
            .checked_mul(16)
            .and_then(|value| character.to_digit(16).map(|digit| value + digit))
            .ok_or_else(invalid)?;
    }
    char::from_u32(value).ok_or_else(invalid)
}

fn decode_digest(value: &str) -> Result<[u8; 32], TransitionFamilyControlError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(invalid());
    }
    let mut digest = [0_u8; 32];
    let bytes = value.as_bytes();
    for (index, output) in digest.iter_mut().enumerate() {
        let offset = index * 2;
        *output = (hex(bytes[offset])? << 4) | hex(bytes[offset + 1])?;
    }
    Ok(digest)
}
fn hex(byte: u8) -> Result<u8, TransitionFamilyControlError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(invalid()),
    }
}
fn malformed<T>() -> Result<T, TransitionFamilyControlError> {
    Err(malformed_error())
}
fn malformed_error() -> TransitionFamilyControlError {
    TransitionFamilyControlError::new(TransitionFamilyControlKind::Malformed)
}
fn invalid() -> TransitionFamilyControlError {
    TransitionFamilyControlError::new(TransitionFamilyControlKind::InvalidDeclaration)
}
fn classify_core_error(
    error: animsmith_core::TransitionFamilyError,
) -> TransitionFamilyControlError {
    match error {
        animsmith_core::TransitionFamilyError::NormalizedTooLarge => {
            TransitionFamilyControlError::new(TransitionFamilyControlKind::NormalizedTooLarge)
        }
        _ => invalid(),
    }
}

#[derive(Deserialize)]
struct CollectionHeaderWire {
    schema: String,
    schema_version: u32,
}
#[derive(Deserialize)]
struct DocumentConfigWire {
    #[serde(default)]
    transition_families: Option<DocumentFamiliesWire>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionEnvelopeWire {
    schema: String,
    schema_version: u32,
    scope: String,
    collection_id: String,
    manifest_input_identity: IdentityWire,
    #[serde(deserialize_with = "deserialize_families")]
    families: Vec<CollectionFamilyWire>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityWire {
    sha256: String,
    bytes: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionFamilyWire {
    family_id: String,
    boundary: String,
    basis: BasisWire,
    tolerances: TolerancesWire,
    #[serde(deserialize_with = "deserialize_collection_members")]
    members: Vec<CollectionMemberWire>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentFamilyWire {
    schema: String,
    schema_version: u32,
    scope: String,
    boundary: String,
    basis: BasisWire,
    tolerances: TolerancesWire,
    #[serde(deserialize_with = "deserialize_document_members")]
    members: Vec<DocumentMemberWire>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BasisWire {
    translation: String,
    rotation: String,
    time: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TolerancesWire {
    translation_m: f64,
    rotation_deg: f64,
    time_normalized: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentMemberWire {
    take_index: u64,
    take_name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionMemberWire {
    logical_id: String,
    source: String,
    take_index: u64,
    take_name: String,
}

struct DocumentFamiliesWire(BTreeMap<String, DocumentFamilyWire>);
impl<'de> Deserialize<'de> for DocumentFamiliesWire {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Families;
        impl<'de> Visitor<'de> for Families {
            type Value = DocumentFamiliesWire;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("at most 4096 document transition families")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut families = BTreeMap::new();
                while let Some((key, family)) = map.next_entry::<String, DocumentFamilyWire>()? {
                    if families.len() >= TRANSITION_FAMILY_V1_MAX_FAMILIES {
                        return Err(serde::de::Error::custom("too many transition families"));
                    }
                    if families.insert(key, family).is_some() {
                        return Err(serde::de::Error::duplicate_field("transition family"));
                    }
                }
                Ok(DocumentFamiliesWire(families))
            }
        }
        deserializer.deserialize_map(Families)
    }
}

fn deserialize_families<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<CollectionFamilyWire>, D::Error> {
    deserialize_bounded_seq(
        deserializer,
        "transition families",
        TRANSITION_FAMILY_V1_MAX_FAMILIES,
        "too many transition families",
    )
}
fn deserialize_document_members<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<DocumentMemberWire>, D::Error> {
    deserialize_bounded_seq(
        deserializer,
        "document transition members",
        TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY,
        "too many document transition members",
    )
}
fn deserialize_collection_members<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<CollectionMemberWire>, D::Error> {
    deserialize_bounded_seq(
        deserializer,
        "collection transition members",
        TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY,
        "too many collection transition members",
    )
}
fn deserialize_bounded_seq<'de, D, T>(
    deserializer: D,
    label: &'static str,
    maximum: usize,
    limit_message: &'static str,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Bounded<T> {
        label: &'static str,
        maximum: usize,
        limit_message: &'static str,
        marker: std::marker::PhantomData<T>,
    }
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Bounded<T> {
        type Value = Vec<T>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.label)
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = sequence.next_element()? {
                if values.len() >= self.maximum {
                    return Err(serde::de::Error::custom(self.limit_message));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Bounded {
        label,
        maximum,
        limit_message,
        marker: std::marker::PhantomData,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    fn document() -> String {
        format!(
            r#"[transition_families."walk_to_run"]
schema = "{TRANSITION_FAMILY_V1_ID}"
schema_version = 1
scope = "document"
boundary = "both"
[transition_families."walk_to_run".basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[transition_families."walk_to_run".tolerances]
translation_m = 0.05
rotation_deg = 5.0
time_normalized = 0.0
[[transition_families."walk_to_run".members]]
take_index = 0
take_name = "Walk"
[[transition_families."walk_to_run".members]]
take_index = 1
take_name = "Run"
"#
        )
    }
    fn collection() -> String {
        format!(
            r#"schema = "{TRANSITION_FAMILY_V1_ID}"
schema_version = 1
scope = "collection"
collection_id = "com.example"
manifest_input_identity = {{ sha256 = "{DIGEST}", bytes = 7 }}
[[families]]
family_id = "com.example/walk-to-run"
boundary = "entry"
[families.basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[families.tolerances]
translation_m = 0.05
rotation_deg = 5.0
time_normalized = 0.0
[[families.members]]
logical_id = "com.example/walk"
source = "walk"
take_index = 0
take_name = "Walk"
[[families.members]]
logical_id = "com.example/run"
source = "run"
take_index = 1
take_name = "Run"
"#
        )
    }
    #[test]
    fn document_reader_sorts_families_preserves_members_and_keeps_identity_authorities_distinct() {
        let source = format!(
            "{}{}",
            document(),
            document()
                .replace("walk_to_run", "idle_to_walk")
                .replace("take_name = \"Walk\"", "take_name = \"Idle\"")
        );
        let parsed = parse_document_transition_families_bytes(source.as_bytes()).unwrap();
        let families = parsed.declaration().document_families().expect("document");
        assert_eq!(
            families
                .iter()
                .map(|family| family.family_id())
                .collect::<Vec<_>>(),
            ["idle_to_walk", "walk_to_run"]
        );
        assert_eq!(families[1].members()[0].take_name(), "Walk");
        assert_ne!(parsed.source_identity(), parsed.normalized_identity());
    }
    #[test]
    fn collection_reader_rejects_closed_shape_scope_schema_numbers_and_members() {
        let valid = collection();
        let parsed = parse_collection_transition_families_bytes(valid.as_bytes()).unwrap();
        assert_eq!(
            parsed.declaration().collection_families().unwrap()[0].boundary(),
            TransitionFamilyBoundaryV1::Entry
        );
        assert_eq!(
            parse_collection_transition_families_bytes(
                valid
                    .replace("boundary = \"entry\"", "boundary = \"exit\"")
                    .as_bytes()
            )
            .unwrap()
            .declaration()
            .collection_families()
            .unwrap()[0]
                .boundary(),
            TransitionFamilyBoundaryV1::Exit
        );
        for invalid_source in [
            valid.replace("scope = \"collection\"", "scope = \"document\""),
            valid.replace("schema_version = 1", "schema_version = 2"),
            valid.replace(TRANSITION_FAMILY_V1_ID, "urn:animsmith:schema:other:1"),
            valid.replace("boundary = \"entry\"", "boundary = \"side\""),
            valid.replace(
                "boundary = \"entry\"",
                "boundary = \"entry\"\nboundary = \"entry\"",
            ),
            valid.replace("translation_m = 0.05", "translation_m = nan"),
            valid.replace(
                "logical_id = \"com.example/run\"",
                "logical_id = \"com.example/walk\"",
            ),
            valid.replace(
                "logical_id = \"com.example/run\"",
                "logical_id = \"other.example/run\"",
            ),
            format!("{valid}unknown = true"),
        ] {
            assert!(parse_collection_transition_families_bytes(invalid_source.as_bytes()).is_err());
        }
    }
    #[test]
    fn explicit_preflight_refuses_depth_before_toml_decode() {
        let nested = |levels| format!("value = {}0{}", "[".repeat(levels), "]".repeat(levels));
        assert!(
            preflight_toml(
                nested(TRANSITION_FAMILY_V1_MAX_DEPTH - 1).as_bytes(),
                DeclarationScope::Document
            )
            .is_ok()
        );
        assert_eq!(
            preflight_toml(
                nested(TRANSITION_FAMILY_V1_MAX_DEPTH).as_bytes(),
                DeclarationScope::Document
            )
            .unwrap_err()
            .kind(),
            TransitionFamilyControlKind::Depth
        );
    }

    #[test]
    fn first_pass_honors_toml_string_comment_key_and_depth_grammar() {
        let bare = |count| {
            (0..count)
                .map(|index| format!("a{index}"))
                .collect::<Vec<_>>()
                .join(".")
        };
        assert!(
            preflight_toml(
                format!("{} = 1\n", bare(16)).as_bytes(),
                DeclarationScope::Document
            )
            .is_ok()
        );
        assert_eq!(
            preflight_toml(
                format!("{} = 1\n", bare(17)).as_bytes(),
                DeclarationScope::Document
            )
            .unwrap_err()
            .kind(),
            TransitionFamilyControlKind::Depth
        );
        let shallow = format!(
            "note = \"\"\"escaped quote: \\\" and # [] .\nstill string\"\"\"\nother = '''literal ] . #\ncontinues'''\n[transition_families.\"combat.entry.v1.with.many.dots.and.]#.\"] # ] . #\n{}",
            document().replace("walk_to_run", "combat.entry.v1.with.many.dots.and.]#")
        );
        assert!(preflight_toml(shallow.as_bytes(), DeclarationScope::Document).is_ok());
        assert!(
            parse_document_transition_families_bytes(
                format!(
                    "note = \"\"\"escaped quote: \\\" and # [] .\nstill string\"\"\"\n{}",
                    document()
                )
                .as_bytes()
            )
            .is_ok()
        );
        let desync = format!(
            "value = \"\"\"four quotes \\\"\\\"\\\"\\\" stay lexical\ntext\"\"\"\n{} = 0\n",
            bare(17)
        );
        assert_eq!(
            preflight_toml(desync.as_bytes(), DeclarationScope::Document)
                .unwrap_err()
                .kind(),
            TransitionFamilyControlKind::Depth
        );
        let arrays = "[[a]] # comment [ ]\n[a.\"b.c]#\"]\nvalue = [ { quoted = \"\\\" ] #\" } ]\n";
        assert!(preflight_toml(arrays.as_bytes(), DeclarationScope::Document).is_ok());
        let tables = |count| {
            (0..count)
                .map(|index| format!("a{index}"))
                .collect::<Vec<_>>()
                .join(".")
        };
        assert!(
            preflight_toml(
                format!("[{}]\n", tables(15)).as_bytes(),
                DeclarationScope::Document
            )
            .is_ok()
        );
        assert_eq!(
            preflight_toml(
                format!("[{}]\n", tables(16)).as_bytes(),
                DeclarationScope::Document
            )
            .unwrap_err()
            .kind(),
            TransitionFamilyControlKind::Depth
        );
        assert!(
            preflight_toml(
                format!("[[{}]]\n", tables(14)).as_bytes(),
                DeclarationScope::Document
            )
            .is_ok()
        );
        assert_eq!(
            preflight_toml(
                format!("[[{}]]\n", tables(15)).as_bytes(),
                DeclarationScope::Document
            )
            .unwrap_err()
            .kind(),
            TransitionFamilyControlKind::Depth
        );
    }

    #[test]
    fn first_pass_rejects_first_over_cap_before_toml_decode() {
        let document_families = |count| {
            (0..count)
                .map(|index| format!("[transition_families.\"f{index}\"]\n"))
                .collect::<String>()
        };
        assert!(
            preflight_toml(
                document_families(TRANSITION_FAMILY_V1_MAX_FAMILIES).as_bytes(),
                DeclarationScope::Document
            )
            .is_ok()
        );
        assert!(
            preflight_toml(
                document_families(TRANSITION_FAMILY_V1_MAX_FAMILIES + 1).as_bytes(),
                DeclarationScope::Document
            )
            .is_err()
        );
        let dotted_document_families = (0..(TRANSITION_FAMILY_V1_MAX_FAMILIES + 1))
            .map(|index| format!("transition_families.f{index}.schema = \"x\"\n"))
            .collect::<String>();
        assert!(
            preflight_toml(
                dotted_document_families.as_bytes(),
                DeclarationScope::Document
            )
            .is_err()
        );
        let implicit_document_families = (0..(TRANSITION_FAMILY_V1_MAX_FAMILIES + 1))
            .map(|index| format!("[[transition_families.\"f{index}\".members]]\n"))
            .collect::<String>();
        assert!(
            preflight_toml(
                implicit_document_families.as_bytes(),
                DeclarationScope::Document
            )
            .is_err()
        );

        let members = |count| {
            format!(
                "[transition_families.\"f\"]\n{}",
                (0..count)
                    .map(|_| "[[transition_families.\"f\".members]]\n")
                    .collect::<String>()
            )
        };
        assert!(
            preflight_toml(
                members(TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY).as_bytes(),
                DeclarationScope::Document
            )
            .is_ok()
        );
        assert!(
            preflight_toml(
                members(TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY + 1).as_bytes(),
                DeclarationScope::Document
            )
            .is_err()
        );

        let aggregate = (0..5)
            .map(|family| {
                let count = if family == 4 {
                    1
                } else {
                    TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY
                };
                format!(
                    "[transition_families.\"f{family}\"]\n{}",
                    (0..count)
                        .map(|_| format!("[[transition_families.\"f{family}\".members]]\n"))
                        .collect::<String>()
                )
            })
            .collect::<String>();
        assert!(preflight_toml(aggregate.as_bytes(), DeclarationScope::Document).is_err());

        let collection_families = (0..(TRANSITION_FAMILY_V1_MAX_FAMILIES + 1))
            .map(|_| "[[families]]\n")
            .collect::<String>();
        assert!(
            preflight_toml(collection_families.as_bytes(), DeclarationScope::Collection).is_err()
        );
        let collection_members = |count| {
            format!(
                "[[families]]\n{}",
                (0..count)
                    .map(|_| "[[families.members]]\n")
                    .collect::<String>()
            )
        };
        assert!(
            preflight_toml(
                collection_members(TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY).as_bytes(),
                DeclarationScope::Collection
            )
            .is_ok()
        );
        assert!(
            preflight_toml(
                collection_members(TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY + 1).as_bytes(),
                DeclarationScope::Collection
            )
            .is_err()
        );
        let collection_aggregate = (0..5)
            .map(|family| {
                let count = if family == 4 {
                    1
                } else {
                    TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY
                };
                format!(
                    "[[families]]\n{}",
                    (0..count)
                        .map(|_| "[[families.members]]\n")
                        .collect::<String>()
                )
            })
            .collect::<String>();
        assert!(
            preflight_toml(
                collection_aggregate.as_bytes(),
                DeclarationScope::Collection
            )
            .is_err()
        );

        let orphan_member = "[[families.members]]\n";
        assert!(preflight_toml(orphan_member.as_bytes(), DeclarationScope::Collection).is_err());
        assert!(
            preflight_toml(
                orphan_member
                    .repeat(TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY + 1)
                    .as_bytes(),
                DeclarationScope::Collection
            )
            .is_err()
        );
        let near_source_cap_orphans = orphan_member.repeat(
            (TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES as usize - orphan_member.len())
                / orphan_member.len(),
        );
        assert!(near_source_cap_orphans.len() > 7 * 1024 * 1024);
        assert!(
            preflight_toml(
                near_source_cap_orphans.as_bytes(),
                DeclarationScope::Collection
            )
            .is_err()
        );
    }

    #[test]
    fn source_byte_bound_is_exact_and_full_config_fields_remain_unowned() {
        let exact = vec![b' '; TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES as usize];
        assert!(preflight_toml(&exact, DeclarationScope::Document).is_ok());
        assert!(parse_document_transition_families_bytes(&exact).is_ok());
        let over = vec![b' '; TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES as usize + 1];
        assert_eq!(
            preflight_toml(&over, DeclarationScope::Document)
                .unwrap_err()
                .kind(),
            TransitionFamilyControlKind::TooLarge
        );
        let full_config = format!(
            "[unrelated.\"key.with.dots\"]\ntext = \"# ] .\"\n{}",
            document()
        );
        assert!(parse_document_transition_families_bytes(full_config.as_bytes()).is_ok());
    }

    #[test]
    fn lexical_string_bounds_refuse_n_plus_one_before_toml_strings() {
        let exact_take = "x".repeat(TRANSITION_FAMILY_V1_MAX_STRING_BYTES);
        assert!(
            parse_document_transition_families_bytes(
                document()
                    .replace(
                        "take_name = \"Walk\"",
                        &format!("take_name = \"{exact_take}\"")
                    )
                    .as_bytes()
            )
            .is_ok()
        );
        let over_take = "x".repeat(TRANSITION_FAMILY_V1_MAX_STRING_BYTES + 1);
        assert_eq!(
            preflight_toml(
                document()
                    .replace(
                        "take_name = \"Walk\"",
                        &format!("take_name = \"{over_take}\"")
                    )
                    .as_bytes(),
                DeclarationScope::Document
            )
            .unwrap_err()
            .kind(),
            TransitionFamilyControlKind::TooLarge
        );
        let escaped = "\\u00e9".repeat(TRANSITION_FAMILY_V1_MAX_STRING_BYTES / 2);
        assert!(
            preflight_toml(
                format!("unrelated = \"{escaped}\"\n").as_bytes(),
                DeclarationScope::Document
            )
            .is_ok()
        );
        let escaped_over = "\\u00e9".repeat(TRANSITION_FAMILY_V1_MAX_STRING_BYTES / 2 + 1);
        assert!(
            preflight_toml(
                format!("unrelated = \"{escaped_over}\"\n").as_bytes(),
                DeclarationScope::Document
            )
            .is_err()
        );
        let exact_identifier = "a".repeat(COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES);
        assert!(
            preflight_toml(
                collection()
                    .replace(
                        "source = \"walk\"",
                        &format!("source = \"{exact_identifier}\"")
                    )
                    .as_bytes(),
                DeclarationScope::Collection
            )
            .is_ok()
        );
        let over_identifier = "a".repeat(COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES + 1);
        assert!(
            preflight_toml(
                collection()
                    .replace(
                        "source = \"walk\"",
                        &format!("source = \"{over_identifier}\"")
                    )
                    .as_bytes(),
                DeclarationScope::Collection
            )
            .is_err()
        );
        for (field, original) in [
            ("collection_id", "com.example"),
            ("family_id", "com.example/walk-to-run"),
            ("logical_id", "com.example/walk"),
            ("source", "walk"),
        ] {
            let exact = "a".repeat(COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES);
            let over = "a".repeat(COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES + 1);
            let exact_source = collection().replace(
                &format!("{field} = \"{original}\""),
                &format!("{field} = \"{exact}\""),
            );
            assert!(preflight_toml(exact_source.as_bytes(), DeclarationScope::Collection).is_ok());
            assert!(
                preflight_toml(
                    exact_source
                        .replace(
                            &format!("{field} = \"{exact}\""),
                            &format!("{field} = \"{over}\"")
                        )
                        .as_bytes(),
                    DeclarationScope::Collection
                )
                .is_err()
            );
        }
        let exact_family_key = "a".repeat(TRANSITION_FAMILY_V1_MAX_DOCUMENT_FAMILY_ID_BYTES);
        let over_family_key = "a".repeat(TRANSITION_FAMILY_V1_MAX_DOCUMENT_FAMILY_ID_BYTES + 1);
        for key in [&format!("\"{exact_family_key}\""), &exact_family_key] {
            assert!(
                preflight_toml(
                    format!("[transition_families.{key}]\n").as_bytes(),
                    DeclarationScope::Document
                )
                .is_ok()
            );
        }
        for key in [&format!("\"{over_family_key}\""), &over_family_key] {
            assert!(
                preflight_toml(
                    format!("[transition_families.{key}]\n").as_bytes(),
                    DeclarationScope::Document
                )
                .is_err()
            );
        }
        let exact_unrelated_key = "a".repeat(TRANSITION_FAMILY_V1_MAX_STRING_BYTES);
        assert!(
            preflight_toml(
                format!("{exact_unrelated_key} = 1\n").as_bytes(),
                DeclarationScope::Document
            )
            .is_ok()
        );
        let over_unrelated_key = "a".repeat(TRANSITION_FAMILY_V1_MAX_STRING_BYTES + 1);
        assert!(
            preflight_toml(
                format!("{over_unrelated_key} = 1\n").as_bytes(),
                DeclarationScope::Document
            )
            .is_err()
        );
        for (opening, closing) in [("\"\"\"", "\"\"\""), ("'''", "'''")] {
            for newline in ["\n", "\r\n"] {
                let exact = "x".repeat(TRANSITION_FAMILY_V1_MAX_STRING_BYTES);
                assert!(
                    preflight_toml(
                        format!("unrelated = {opening}{newline}{exact}{closing}\n").as_bytes(),
                        DeclarationScope::Document
                    )
                    .is_ok()
                );
                let over = "x".repeat(TRANSITION_FAMILY_V1_MAX_STRING_BYTES + 1);
                assert!(
                    preflight_toml(
                        format!("unrelated = {opening}{newline}{over}{closing}\n").as_bytes(),
                        DeclarationScope::Document
                    )
                    .is_err()
                );
            }
        }
        let continued = "x".repeat(TRANSITION_FAMILY_V1_MAX_STRING_BYTES);
        assert!(
            preflight_toml(
                format!("unrelated = \"\"\"\\\n  {continued}\"\"\"\n").as_bytes(),
                DeclarationScope::Document
            )
            .is_ok()
        );
        let continued_over = "x".repeat(TRANSITION_FAMILY_V1_MAX_STRING_BYTES + 1);
        assert!(
            preflight_toml(
                format!("unrelated = \"\"\"\\\n  {continued_over}\"\"\"\n").as_bytes(),
                DeclarationScope::Document
            )
            .is_err()
        );
    }

    #[test]
    fn empty_document_is_valid_and_empty_collection_is_not() {
        let document =
            parse_document_transition_families_bytes(b"[unrelated]\nvalue = 1\n").unwrap();
        let families = document
            .declaration()
            .document_families()
            .expect("document");
        assert!(families.is_empty());
        let empty = collection().replace("[[families]]\nfamily_id = \"com.example/walk-to-run\"\nboundary = \"entry\"\n[families.basis]\ntranslation = \"skeleton-local-metres\"\nrotation = \"skeleton-local-degrees\"\ntime = \"normalized-clip\"\n[families.tolerances]\ntranslation_m = 0.05\nrotation_deg = 5.0\ntime_normalized = 0.0\n[[families.members]]\nlogical_id = \"com.example/walk\"\nsource = \"walk\"\ntake_index = 0\ntake_name = \"Walk\"\n[[families.members]]\nlogical_id = \"com.example/run\"\nsource = \"run\"\ntake_index = 1\ntake_name = \"Run\"\n", "");
        assert!(parse_collection_transition_families_bytes(empty.as_bytes()).is_err());
    }

    #[test]
    fn first_pass_closes_noncanonical_inline_declaration_aggregates() {
        let inline = document()
            .replace(
                "boundary = \"both\"\n",
                "boundary = \"both\"\nmembers = [{ take_index = 0, take_name = \"Walk\" }, { take_index = 1, take_name = \"Run\" }]\n",
            )
            .replace(
                "[[transition_families.\"walk_to_run\".members]]\ntake_index = 0\ntake_name = \"Walk\"\n[[transition_families.\"walk_to_run\".members]]\ntake_index = 1\ntake_name = \"Run\"\n",
                "",
            );
        assert!(preflight_toml(inline.as_bytes(), DeclarationScope::Document).is_err());
        assert!(
            preflight_toml(
                b"transition_families = { f = { members = [] } }\n",
                DeclarationScope::Document
            )
            .is_err()
        );
        assert!(
            preflight_toml(
                b"[transition_families]\nf = { members = [] }\n",
                DeclarationScope::Document
            )
            .is_err()
        );
        assert!(
            preflight_toml(
                b"families = [{ family_id = \"com.example/f\", members = [] }]\n",
                DeclarationScope::Collection
            )
            .is_err()
        );
        assert!(preflight_toml(b"families.members = []\n", DeclarationScope::Collection).is_err());
    }

    #[test]
    fn document_family_count_does_not_rescan_unrelated_tables() {
        let unrelated = (0..10_000)
            .map(|index| format!("[unrelated.t{index}]\n"))
            .collect::<String>();
        let families = (0..TRANSITION_FAMILY_V1_MAX_FAMILIES)
            .map(|index| format!("[transition_families.\"f{index}\"]\n"))
            .collect::<String>();
        assert!(
            preflight_toml(
                format!("{unrelated}{families}").as_bytes(),
                DeclarationScope::Document
            )
            .is_ok()
        );
    }

    #[test]
    fn inline_table_dotted_keys_have_their_own_depth_budget() {
        let dotted = |count| {
            (0..count)
                .map(|index| format!("a{index}"))
                .collect::<Vec<_>>()
                .join(".")
        };
        let exact = format!("unrelated = {{ {} = 1 }}\n", dotted(15));
        assert!(parse_document_transition_families_bytes(exact.as_bytes()).is_ok());
        let over = format!("unrelated = {{ {} = 1 }}\n", dotted(16));
        assert_eq!(
            preflight_toml(over.as_bytes(), DeclarationScope::Document)
                .unwrap_err()
                .kind(),
            TransitionFamilyControlKind::Depth
        );
    }
}
