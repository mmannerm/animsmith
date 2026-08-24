# Collection contract extensions

This document records versioned schema-only extensions to the file-scoped
collection decision in [DESIGN.md Appendix F](../DESIGN.md#appendix-f--decision-record-file-scoped-clip-identity-and-collections).
The contact-fragment V1 is retained in the 0.5.0 contract line; the
directional-speed policy/evaluation V1 is an ordered 0.6.0 slice. They are interchange
declarations, not new CLI commands or runtime systems.

## Directional-speed policy V1 (#552, ordered slice)

The strict `collection-directional-speed-policy:1` reader is an ordered,
manifest-bound declaration for a later directional-speed evaluator. It is a
separate TOML envelope: it does not add fields to collection-manifest V1,
revise collection-output V2, infer membership from filenames or paths, or add
the eventual `collection evaluate-directional-speed` command. This slice only
freezes the typed reader, binding contract, pure evaluator, immutable result,
and strict V2 adapter; command and publication remain a follow-up.

The envelope repeats the exact manifest identity (`collection_id` plus
`{sha256, bytes}`), one directional-blend `runtime_set_id`, and every existing
logical member exactly once in manifest order. Each member has one nonzero,
unique semantic `[x, z]` coordinate. `source_basis.x` and `source_basis.z`
are orientation witnesses for raw collection-output V2 +X/+Z endpoint
displacement in that semantic plane; their magnitudes are nonsemantic, and a
future evaluator uses unit axes for heading. They must be finite, bounded,
nonzero, and perpendicular. `diagonal_behavior` is closed and applies to
unit-input/base targets: for coordinate `c`, `preserve` uses gain
`g(c) = hypot(c)` while `normalize` uses `g(c) = 1`. Thus uniform expected
speed is `base * g(c)`, authored expected speed is the member's base
`speed_mps * g(c)`, and ratios compare
`expected_measured_ratio_i_to_ref = declared_expected_ratio_i * g(c_i) /
g(c_ref)`, with the reference declaration fixed at `1.0`. Direction comparison
is unaffected by this gain.

The declaration also carries finite bounded `direction_tolerance_deg` in the
inclusive range `0..=180` degrees. The pure evaluator normalizes raw endpoint
displacement before source-basis projection and uses published `speed_mps` for
magnitude (not travel). Its immutable result binds raw policy and evidence
`InputIdentity` values and retains a manifest-ordered row for every member:
policy coordinate, raw evidence, projected heading, comparison values,
tolerances, deviations, and pass/violation outcome. Incomplete root travel,
zero endpoint, zero ratio reference, and numeric range are typed
not-evaluated outcomes, never implicit passes, failures, or subsets.

`uniform` requires `uniform_speed_mps` and `speed_tolerance_mps`; `authored`
requires each member's `speed_mps` plus `speed_tolerance_mps`; and `ratios`
requires a declared `reference_member`, each member's `expected_ratio`, and a
dimensionless `ratio_tolerance`. Mode-inapplicable fields are rejected. The
strict reader rejects parse/declaration failures including unknown or
duplicate fields, unknown tokens, malformed identities, duplicate member
ids or coordinates, invalid coordinates or basis, nonfinite/out-of-range
values, and N+1 members before retaining them. Separate binding validation
rejects stale manifest identity, wrong runtime-set id or kind, and missing,
extra, or reordered member sequences. No licensed asset data belongs in this
declaration or its tests.

## Contact fragments (#147)

The contact-fragment V1 identity is
`urn:animsmith:schema:contact-fragment:1`.
AnimSmith 0.6 now validates through a format-neutral strict core reader and
bounded canonicalization seam. It still adds no asset loading/detection,
sidecar publication, CLI command, transform, or runtime system.

It is an importable envelope that
can be merged into a host's one authoritative measured sidecar. It binds
contact facts to the exact primary source bytes and complete versioned
dependency closure, plus producer/tool version and an unambiguous clip
reference. `artifact` is the existing primary `InputIdentity`;
`dependency_closure_identity` is the existing complete
`DependencyClosureIdentityV1` rather than the full output-v10 closure record.
Both serialize as `{sha256, bytes}`: `sha256` makes the algorithm and digest
explicit rather than adding separate `algorithm` and `digest` fields. Partial
or unavailable dependency coverage refuses fragment generation, and a
mismatch in either identity makes an existing fragment stale.
The complete captured closure's `primary_input` must equal `artifact`; producer
and consumer validate that relationship against the captured closure.
V1 deliberately binds every dependency in the complete modeled closure,
including dependencies such as textures that may not affect contacts. This can
refuse generation for an unavailable unrelated dependency, but avoids a second
format-specific relevance policy.

Collection-owned clips use the [#409 logical identity](../DESIGN.md#f2-logical-and-physical-clip-identity)
plus its `source`, take-index, and exact take-name witness. Standalone
documents use an exact embedded clip/take name scoped by both input identities;
ambiguous or duplicate names are refused. No animation-array index, filename,
or engine asset handle is an identity.

Points and windows use normalized clip time `[0, 1]`, with the measured
positive duration recorded for validation. Events have stable opaque ids,
engine-neutral roles and phases, and deterministic ordering. V1's closed role
vocabulary is `left_foot`, `right_foot`, `left_hand`, `right_hand`, `left_toe`,
`right_toe`, `left_knee`, `right_knee`, `left_elbow`, `right_elbow`, `root`,
`prop`, and `body`; its phases are `begin`, `end`, and `marker`. Trim, slice, and
time-warp operations must transform the fragment with the same operation or
return structured per-event outcomes/refusal. Events outside the retained
interval are never silently clamped, dropped, or retained.

Confidence, when present, is finite and lies in `[0, 1]`. Core fields are
closed. Extensions use strict envelopes with exactly `schema`,
`schema_version`, and `payload`, where the extension's versioned schema owns
the payload object; an unsupported but well-formed extension may be preserved
as opaque data. Any transformer must understand and apply every
extension's operation contract or refuse the whole operation with
`unsupported_extension`; it may not copy opaque extension-owned times or event
references unchanged.

The strict V1 reader caps a contact-fragment source at 8,388,608 bytes and 32
JSON container levels, with at most 4,096 events and 256 extensions. Authored
strings are at most 4,096 UTF-8 bytes and identifiers at most 255 bytes; each
extension payload is at most 262,144 canonical bytes and 16 levels deep. A
complete fragment is at most 8,388,608 canonical bytes. A transform-result
source and complete canonical result are each at most 16,777,216 bytes, with at
most 4,096 control points and 4,096 event outcomes; its inline fragment obeys
the fragment limits. Depth counts every object/array on the root-to-value path,
including the root envelope as depth 1. A payload object also starts at depth 1
for its separate 16-level limit. Each exact maximum is accepted; N+1 is
rejected during bounded decoding before the excess value is retained or an
unbounded canonical buffer is allocated; canonical-byte limits use a bounded
JCS sink.
JSON Schema `maxLength` counts Unicode code points, while this reader's text
and identifier limits count UTF-8 bytes, so a non-ASCII value may satisfy the
schema and still be correctly refused by the reader. The reader accepts decoded
IEEE-754 numbers only within the safe magnitude
`[-9007199254740991, 9007199254740991]`; this includes identity byte counts
and opaque extension payload numbers. Extension payloads are stored from their
bounded canonical JCS bytes, so canonical serialization followed by strict
reread preserves the validated fragment value. An explicitly empty `extensions` array
is refused so omitted and present data do not canonicalize to the same value.

A minimal collection-scoped envelope is:

```json
{
  "schema": "urn:animsmith:schema:contact-fragment:1",
  "schema_version": 1,
  "producer": {"tool": "animsmith", "version": "0.6.0"},
  "artifact": {"sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "bytes": 123456},
  "dependency_closure_identity": {"sha256": "1111111111111111111111111111111111111111111111111111111111111111", "bytes": 456},
  "clip": {"scope": "collection", "logical_id": "com.example.pack/locomotion/walk-forward-in-place", "source": "walk-forward", "take_index": 0, "take_name": "Take 001"},
  "duration_s": 1.2,
  "events": [{"event_id": "left-foot/0", "role": "left_foot", "phase": "marker", "time": 0.23, "confidence": 0.92}],
  "extensions": [{"schema": "urn:example:contact-quality:1", "schema_version": 1, "payload": {"quality": "high"}}]
}
```

Seconds and frame values, when shown, are derived display values rather than
identity or comparison coordinates. Canonical bytes use the complete
[RFC 8785 JSON Canonicalization Scheme (JCS)](https://www.rfc-editor.org/rfc/rfc8785),
including its object-key, string, and number rules; every extension payload
must also be JCS-canonicalizable. Before JCS serialization, events use the exact mixed-event sort tuple
`(start, kind_rank, end_key, role, phase, event_id)`: points have kind rank `0`
with `start = time` and a `null` end sentinel that sorts before every numeric
window end; windows use their declared `start`, have kind rank `1`, and use
their numeric end. This keeps point/window order stable when starts coincide.
Tuple strings, including opaque event ids, compare by
unsigned UTF-16 code units exactly like RFC 8785 property names, without
Unicode normalization.

Trim, slice, resample, and time-warp use the separate
`urn:animsmith:schema:contact-transform-result:1` result. It binds input and,
on success, output primary, dependency-closure, and canonical-fragment
identities. Its strict top-level fields are `schema`, `schema_version`,
`operation`, `input`, `outcome`, and `event_outcomes`, with `output` only on
success and `refusal { code, message }` only on refusal. `operation` is a
strict tagged object: trim/slice use `interval { start, end }`, resample uses
`mapping = "identity"`, and `time_warp` uses required finite positive
`output_duration_s` plus ordered `control_points` with `input_time`/`output_time`
from `(0,0)` to `(1,1)`. Between adjacent knots `(x0, y0)` and `(x1, y1)`, it
maps `t` by the exact piecewise-linear formula
`y0 + ((t - x0) / (x1 - x0)) * (y1 - y0)`; exact knots map exactly, and both
window endpoints use the same rule. A known V1 tag with an invalid numeric
domain or ordering remains representable for an `invalid_mapping` refusal and
uses an empty pre-inventory `event_outcomes` list. An unknown kind, version,
field, mapping token, missing field, or malformed field type is a strict
request/reader error and produces no V1 result or event outcomes. `input` has
exactly
`{artifact:{sha256,bytes},dependency_closure_identity:{sha256,bytes},fragment:{sha256,bytes}}`
and refers to a separately
supplied input fragment. Successful `output` has exactly
`{artifact:{sha256,bytes},dependency_closure_identity:{sha256,bytes},fragment:{sha256,bytes},contact_fragment}`;
all three identities must match the inline complete `contact-fragment:1` and
its two input bindings. The output closure identity is freshly captured rather
than copied, and its closure `primary_input` must equal output `artifact`. For
trim/slice `[a,b]`, a point is
outside exactly when `t < a || t > b`; endpoints are included. A window is
outside exactly when `end < a || start > b`, contained exactly when
`a <= start && end <= b`, and otherwise boundary-crossing. A crossing window
receives `refused` with code `partial_window` and refuses the whole operation.
Known-operation mapping failures, binding mismatches, unsupported extensions,
or partial windows produce a typed `refused` result with no `output` field.
After binding and identity validation, `event_outcomes` has exactly one
`{event_id, outcome}` object per input event in canonical input order. It adds
`value` only for transformed exact point/window values and `code` only for
refused events; pre-inventory binding, identity, mapping-validation, or
extension-support refusal uses an empty list.
Global success requires all events to be transformed or outside and requires
`output`; global refusal requires top-level `refusal` and omits `output`.
The inline output duration is `input_duration_s * (b - a)` for trim/slice,
the input duration for resample, and the required `output_duration_s` for
`time_warp`; the inline fragment and operation must agree exactly. A rounded
trim/slice duration that is not finite and positive refuses before event
inventory with `invalid_value` and an empty `event_outcomes` list.
All time/duration inputs and results are finite IEEE 754 binary64. The
normative sequence is `dx = rn(x1 - x0)`,
`alpha = rn(rn(t - x0) / dx)`, `dy = rn(y1 - y0)`, and
`mapped = rn(y0 + rn(alpha * dy))`; an exact knot bypasses interpolation.
Trim/slice uses `span = rn(b - a)`, `mapped = rn(rn(t - a) / span)`, and
`output_duration_s = rn(input_duration_s * span)`. Here `rn` is IEEE 754
binary64 round-to-nearest, ties-to-even, with no fused or extended-precision
intermediate, so independent producers feed the same numeric results to JCS.
Refusal codes are `partial_window`, `invalid_mapping`, `invalid_binding`,
`invalid_value`, and `unsupported_extension`. Malformed fragments/results and
duplicate, missing, or unknown event-outcome identities are strict reader
errors, not refusal results.

The success shape is illustrative:

```json
{
  "schema": "urn:animsmith:schema:contact-transform-result:1",
  "schema_version": 1,
  "operation": {"kind": "trim", "version": 1, "interval": {"start": 0.1, "end": 0.9}},
  "input": {"artifact": {"sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", "bytes": 123456}, "dependency_closure_identity": {"sha256": "1111111111111111111111111111111111111111111111111111111111111111", "bytes": 456}, "fragment": {"sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789", "bytes": 789}},
  "outcome": "transformed",
  "event_outcomes": [
    {"event_id": "left-foot/0", "outcome": "transformed", "value": {"time": 0.1625}},
    {"event_id": "right-foot/0", "outcome": "transformed", "value": {"window": {"start": 0.7625, "end": 0.8625}}}
  ],
  "output": {
    "artifact": {"sha256": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210", "bytes": 123456},
    "dependency_closure_identity": {"sha256": "2222222222222222222222222222222222222222222222222222222222222222", "bytes": 654},
    "fragment": {"sha256": "a1d63b5e10381d2aa635cfd74218a4e5d42f3d61c1451765e25d47a23e02d633", "bytes": 729},
    "contact_fragment": {
      "schema": "urn:animsmith:schema:contact-fragment:1",
      "schema_version": 1,
      "producer": {"tool": "animsmith", "version": "0.5.0"},
      "artifact": {"sha256": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210", "bytes": 123456},
      "dependency_closure_identity": {"sha256": "2222222222222222222222222222222222222222222222222222222222222222", "bytes": 654},
      "clip": {"scope": "collection", "logical_id": "com.example.pack/locomotion/walk-forward-in-place", "source": "walk-forward", "take_index": 0, "take_name": "Take 001"},
      "duration_s": 0.96,
      "events": [
        {"event_id": "left-foot/0", "role": "left_foot", "phase": "marker", "time": 0.1625, "confidence": 0.92},
        {"event_id": "right-foot/0", "role": "right_foot", "phase": "begin", "window": {"start": 0.7625, "end": 0.8625}}
      ]
    }
  }
}
```

This milestone does not detect contacts, generate events, validate foot
placement, map engine-native event types, or write a host's final sidecar.
Production generation remains [#152](https://github.com/mmannerm/animsmith/issues/152)
in 0.6.0.

## Transition families (#148)

The transition-family V1 identity is
`urn:animsmith:schema:transition-family:1`. A declaration has a stable family
id, an explicit ordered member list, a boundary selection (`entry`, `exit`, or
`both`), and closed typed tolerances with explicit units and basis.

This page freezes the future wire/config proposal. AnimSmith 0.5.0 does not
parse `transition_families` in `animsmith.toml` and exposes no collection
declaration reader or evaluator; using the examples with the current CLI is an
unknown-field error. Later implementation remains #153/#164 or separately
reviewed follow-up work.

Document-local family ids are one lowercase-ASCII token, 1–255 bytes, starting
with `[a-z0-9]` and continuing with `[a-z0-9._-]`. The table key itself is the
id, with no duplicate `family_id` field; quote the key whenever punctuation or
a dot is present (the canonical spelling quotes it always), such as
`[transition_families."walk_to_run"]` or
`[transition_families."combat.entry.v1"]`. Collection family ids retain
Appendix F's slash-qualified logical-id grammar and collection-id prefix.

The declaration distinguishes two ownership scopes:

- future `document` families will be placed in the existing `animsmith.toml`
  under `[transition_families."<family_id>"]` and resolve exact embedded clip/take
  identities. The reusable config carries no artifact digest; the future
  evaluator binds the document `InputIdentity` in its output.
- future `collection` families use a separate declaration envelope, not the
  collection-manifest V1 itself. The envelope binds the exact manifest
  `InputIdentity` `{sha256, bytes}`, then resolves declared logical clip ids
  plus their `source`, take-index, and take-name witnesses.

Members cannot cross scopes, point to paths or animation-array indices, or be
silently removed when missing or ambiguous. `stale_digest` is collection-only:
it means the declaration's manifest `InputIdentity` no longer matches. A
source digest pin in that manifest is enforced by manifest resolution; a
mismatch makes the member unavailable rather than creating a second identity
in this declaration. Reusable document-local config has no stale-digest state
at parse time; the future evaluator binds the exact
document `InputIdentity` and reports source/take resolution in its output. The
existing `[clips]`,
`[gait_groups]`, and `[sync_groups]` sections remain document-local and are
not replaced by a second collection authority. Both placements share the
`transition-family:1` family-record semantics; only ownership and placement
differ. Family declarations are canonicalized by stable family id while
preserving member order.

Both future placements cap the exact declaration and normalized envelope at
8,388,608 source or canonical bytes and 16 container levels. They permit at
most 4,096 families, 4,096 members per family, and 16,384 members in aggregate.
Authored strings are at most 4,096 UTF-8 bytes, with the 255-byte identifier
limits applied more strictly. Depth counts each table/array or JSON
object/array on the root-to-value path, including the root as depth 1. Each
exact maximum is accepted; N+1 is rejected during bounded decoding before
retention or unbounded canonical allocation; canonical-byte limits use a
bounded JCS sink.

A collection-scoped declaration uses the following typed TOML-like shape; each
logical id is accompanied by the #409 source/take witness and must agree with
the manifest:

```toml
schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "collection"
collection_id = "com.example.pack"
manifest_input_identity = { sha256 = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789", bytes = 9876 }

[[families]]
family_id = "com.example.pack/transitions/walk-to-run"
boundary = "both"

[families.basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"

[families.tolerances]
translation_m = 0.05
rotation_deg = 5.0
time_normalized = 0.02

[[families.members]]
logical_id = "com.example.pack/locomotion/walk-forward-in-place"
source = "walk-forward"
take_index = 0
take_name = "Take 001"

[[families.members]]
logical_id = "com.example.pack/locomotion/run-forward-in-place"
source = "run-forward"
take_index = 0
take_name = "Take 001"
```

The document-local placement uses the existing config basis and the same
family-record semantics:

```toml
[transition_families."walk_to_run"]
schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "document"
boundary = "entry"

[transition_families."walk_to_run".basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"

[transition_families."walk_to_run".tolerances]
translation_m = 0.05
rotation_deg = 5.0
time_normalized = 0.02

[[transition_families."walk_to_run".members]]
take_index = 0
take_name = "Walk"

[[transition_families."walk_to_run".members]]
take_index = 1
take_name = "Run"
```

The collection envelope's manifest digest/bytes binding makes a manifest edit
or reorder stale even when collection and logical ids are unchanged. No
runtime check or transition graph is implied by either placement.

Both placements normalize to the exact closed JSON envelopes shown in
[DESIGN.md §F.11](../DESIGN.md#f11-transition-family-declaration-v1-148): one
collection form retaining `collection_id` and `manifest_input_identity`, and
one document form in which each quoted table key becomes `family_id`. Families
sort by id, member order is retained, and RFC 8785 JCS produces the normalized
declaration identity. Future evidence separately binds the exact declaration
source, normalized declaration, and evaluated document or manifest identities.

This milestone does not run transition checks, emit findings or reports,
infer graph edges, add gameplay metadata, or generate engine state-machine or
blend-tree data. Those consumers remain follow-up work under [#153](https://github.com/mmannerm/animsmith/issues/153),
[#164](https://github.com/mmannerm/animsmith/issues/164), and later milestones.

The complete normative contract, including ownership, strict resolution, and
canonical serialization rules, is in [DESIGN.md §F.10](../DESIGN.md#f10-contact-fragment-v1-147)
and [§F.11](../DESIGN.md#f11-transition-family-declaration-v1-148).
