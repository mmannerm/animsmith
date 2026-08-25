# Animation-pack evaluation model V1

`urn:animsmith:skill:animation-pack-evaluation:1` is the repository-owned,
versioned JSON authority for one animation-pack evaluation. It is distinct from
the historical `animation-pack-evaluation-manifest:1`: the latter remains the
legacy report-input contract until the migration slice has landed.

The machine-readable, closed schema is
[`schemas/evaluation-model-v1.schema.json`](../schemas/evaluation-model-v1.schema.json).
`validate_evaluation_model.py` loads that checked-in Draft 2020-12 schema with
the repository's `jsonschema` dependency as its sole structural gate, then
applies relational closure and bounded canonicalization defenses that JSON
Schema cannot express economically (binding equality, exact set order, derived
pairs, and reference closure). The schema's
`maxLength` is Unicode code points, while the validator's 32 KiB text limit is
UTF-8 bytes; that intentional stricter runtime limit protects bounded reads and
is tested separately rather than being claimed as schema-equivalent.

## Authority and binding

Run `validate_evaluation_model.py MODEL.json --binding COLLECTION.json` only
after `COLLECTION.json` has been independently validated as
`urn:animsmith:schema:collection-output:2`. The model records the binding
collection id plus the exact manifest input SHA-256 and byte count. The
validator consumes the output's clip projection (logical id, source, take
index, take name) and runtime-set member ordering. It does **not** read or
reimplement `collection-manifest:1` TOML identity rules.
The independently validated projection must include its `sources` list, even
when the list is empty, so derived source-file totals distinguish known zero
from an absent projection.

Every V1 model is a closed object with these fixed records: presentation; one
current plus zero or more historical runs; manifest-bound clips and runtime
sets; capability assessments; integration steps; one-owner issues;
remediations; engine evidence; limitations; sources; fixed-slot narratives;
and collection constituents/exclusions/cross-pack records. IDs are lowercase,
unique, and lexicographically ordered in every ID-keyed array. Unknown fields
and vocabulary tokens fail validation. The binding makes clip source/take
witnesses and runtime-set membership/order non-editable duplicate authority.

Collection constituents are sorted and their complete unordered pair set is
derived as `n*(n-1)/2`; each pair must have exactly one cross-pack record.
V1 bounds a collection to 90 constituents, so its 4,005 derived pair records
remain within the 4,096-record envelope. Record totals are derived by later
views, never hand-authored headlines.

## Typed record inventory

The validator and the checked-in JSON Schema reject unknown record fields.
`evidence` is the only evidence authority; every consequential record carries
sorted `evidence_refs`. Clip records retain the exact binding witness plus one
canonical primary role, sorted tags/classification bases, typed loop state,
typed duration and root-motion-speed availability/value records, movement
owner, assessment, and coverage. Runtime sets retain the bound id/kind/member
order and give each member one eligibility state (`complete`, `incomplete`, or
`quarantined`), rather than copying metrics already owned by the clip.

Profiles contain every catalog id with status, activation basis, and evidence;
pipeline stages contain all ten ids with coverage/evidence. Readiness records
carry state and adoption consequence. Capabilities carry typed assessment;
recipe steps carry fixed order/action, movement/phase owner, and a coordinate
or threshold expression. Issues separate impact, one primary owner, current
action, future-tool candidacy, and secondary workaround. Remediations retain
their run, input/refusal evidence, output identity, and historical link;
output identities use the same public stable-ID grammar as other model IDs;
refusals can point only to output produced by a historical run.

Engine evidence records runtime/version/level/coverage/settings/procedure;
source records retain source commit, report blob digest, acquisition and license
scope. Narrative is fixed-slot prose with stable fact references only: no
second table, status, identity, count, numeric, or private-path authority.
Evidence locators are direct public HTTPS citations or lowercase
repository-relative `.md`/`.json` paths; they cannot name source assets or
archives. Collection
constituents declare their referenced model digest, bound clip references,
source-file count, and runtime-set references; exclusions are explicit and
disjoint; cross-pack records name `left` and `right` tuple endpoints rather
than encoding an ambiguous pair string. Future views derive constituent,
logical-clip, source-file, runtime-set, and pair totals from these binding
records rather than accepting editable headlines.

## Canonical JSON and digests

V1 canonical bytes are UTF-8 JSON with sorted object keys and no insignificant
whitespace. Numbers are written by the repository-owned
`python-binary64-shortest-v1` encoder: built-in integers and finite IEEE-754
Python `float` values only, Python's stable shortest round-tripping spelling
with a normalized exponent, and `-0` rendered as `0`. This is deliberately a
versioned Python wire rule, not an implied ECMAScript/JCS rule. NaN,
infinity, Decimal, duplicate keys, and non-string object keys are rejected.
The canonical SHA-256 is over those bytes. `json.dumps(sort_keys=True)` is not
the V1 numeric authority. `--check-canonical` verifies stored bytes exactly.

## Migration policy

No legacy report or manifest becomes generated authority in this slice. A
future migration must create a V1 model, bind it to independently validated
collection output, and record the source commit and report blob digests in
fixed source records. It must preserve semantic facts—member identity/order,
measurements, issue owner/action, current versus historical remediation,
engine/artist/vendor boundary, and collection inclusion/exclusion/pairs—not
byte-for-byte prose. The renderer and report migration slices will make the
model authoritative only after their fixed views, AST assertions, and reviewed
migration evidence land. Old schema identifiers must never be silently
reinterpreted; a changed wire contract receives a new schema URN and explicit
migration.
