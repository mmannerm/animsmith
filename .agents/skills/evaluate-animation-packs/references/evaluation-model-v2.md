# Animation-pack evaluation model V2

`urn:animsmith:skill:animation-pack-evaluation:2` is the current structured
authority for a new animation-pack evaluation. It binds directly to one exact,
independently validated `urn:animsmith:schema:collection-output:10` document.
It does not reinterpret that input as historical collection-output V2, and it
does not broaden the frozen V1 validator or renderer contract.

The machine-readable closed schema is
[`schemas/evaluation-model-v2.schema.json`](../schemas/evaluation-model-v2.schema.json).
V2 deliberately reuses V1 record definitions where their meanings did not
change. Its version delta is the current binding: the model records the raw
collection-output SHA-256 and byte count in addition to exact manifest
identity, and retains a path-scrubbed typed row for every V10 source. The model
runtime-set vocabulary also matches the current producer exactly, including
`gait-group`; it does not preserve V1's report-only `other` token.

## Exact current binding

Run the V2 validator with the original collection-output bytes:

```text
.agents/skills/evaluate-animation-packs/scripts/validate_evaluation_model_v2.py \
  MODEL.json --binding COLLECTION-V10.json \
  --animsmith /exact/checkout/target/release/animsmith --check-canonical
```

The offline registry is pinned to collection-output V10, output V18, and
measurements V17. Wrong, historical, or future schema identities fail rather
than flowing through shared fields. Schema success is not sufficient. Before
model projection, the validator opens the binding once with no-follow regular-
file checks and a fixed byte bound, then sends that one in-memory buffer to the
explicitly selected AnimSmith binary as
`animsmith collection validate-output`. The hidden stdin-only validation command
calls the repository's one authoritative Rust strict reader and returns only an
exact internal success handshake naming collection-output V10. Python requires
that exact handshake and parses, hashes, and projects the same buffer without
reopening the pathname. There is no Python semantic mirror or second drift-
prone interpretation. A special file, symlink, missing, non-executable, timed-
out, rejecting, or wrong executable is an operator error.

Use the checkout-matched binary required by the skill snapshot procedure, not
an ambient installed executable. Retain its exact path, SHA-256, `--version`,
source revision, and dirty state in the evaluation evidence. The validator then
hashes the raw binding bytes and compares both digest and byte count with
`model.binding`, followed by manifest identity and every retained source state.

Each `model.binding.sources` row retains the source key, input availability and
observed identity or overflow reason, digest state, scrubbed config identity,
loader state, dependency-closure identity or complete typed reason set, take
inventory state, and document-result availability. Evaluator-local locators
and nested one-file lint payloads remain in the exact hashed V10 input rather
than being copied into the public model.

The model must contain every V10 clip and runtime set. Clip source, declared take
index, and exact take name must match. Established duration and root-speed
facts must equal V10 measurements. Root-speed availability is exact:
`measured` maps to an available value, `unavailable` maps to unavailable, and
`not_applicable` maps to `not-applicable`. An unavailable clip requires
unavailable duration and speed; it cannot use `not-applicable` to erase missing
evidence or become a completed assessment. Runtime-set kind and
member order are exact, and an unavailable member cannot become `complete`.
An incomplete set or collection remains typed soft-fail evidence; the validator
rejects silent omission or promotion but does not reject an honestly partial
model merely because input evidence is unavailable.

V10 slash-separated logical IDs remain valid everywhere they are referenced,
including runtime members, narrative fact references, and collection
constituent clip/set lists. Report-owned evidence and constituent record IDs
retain the frozen V1 ID grammar; only collection logical-ID positions widen.

## Rendering

The fixed renderer selects V1 or V2 only from the model schema URN:

```text
.agents/skills/evaluate-animation-packs/scripts/render_evaluation_model.py \
  MODEL.json --binding COLLECTION-V10.json \
  --animsmith /exact/checkout/target/release/animsmith \
  --report REPORT.md --appendix REPORT-evidence.md
```

V2 views retain the exact collection-output identity and the model-owned typed
source binding rows. Their scrubbed current projection includes source
dependency-closure state and runtime-set availability evidence, while omitting
path-bearing source/config locators and large nested lint envelopes. Validate
the rendered pair with `validate_report.py --evaluation-model-v2
--report-format 2`.

## Immutability and migration

Evaluation-model V1 and collection-output V2 remain immutable and supported by
their original validator path. A V1 model plus V10 input is not a V2 evaluation;
new current work must identify model V2 explicitly. Retained commercial
reports are not migrated by this contract. Any migration still requires the
authorized exact source/take/set authority named by its input gate.
