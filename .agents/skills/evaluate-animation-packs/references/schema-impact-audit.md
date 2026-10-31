# Collection-output schema-impact audit

This ledger makes producer/consumer drift a gated repository decision. When
the current `collection-output` schema advances, update the current evaluation
model and exact offline registry in the same change, or explicitly mark the
workflow blocked before shipping the producer.

Current audited producer: `urn:animsmith:schema:collection-output:12`, with
exact nested `urn:animsmith:schema:output:19` and
`urn:animsmith:schema:measurements:18` evidence.

The schema-id gate covers these repository-owned surfaces:

- canonical skill: current model and invocation wording in `SKILL.md`;
- scripts: model contract, validator, renderer, report validator, and their
  exact offline schema registry; current V3 CLI paths require the explicitly
  selected checkout-matched AnimSmith binary and invoke its one authoritative
  Rust strict reader over the same single no-follow, bounded regular-file byte
  buffer later parsed and projected by Python; an exact V12 handshake rejects
  arbitrary exit-zero executables, so no Python semantic mirror can drift;
- discovery adapters: the Claude adapter remains a thin link to the canonical
  skill and does not copy a drifting schema claim;
- documentation: `DESIGN.md`, evaluation-model V1 immutability, and the V3
  current-binding reference;
- synthetic examples and fixtures: one complete producer-emitted V11 fixture
  accepted by the authoritative Rust reader and hidden validation-only CLI,
  plus synchronized exact-binding,
  dependency identity/reason order, availability/overflow, work-byte, take
  identity, runtime-set membership, and cross-version mutations in
  `test_validators.py`;
- blocked tickets: #574 must request an owner-supplied scrubbed
  `collection-manifest:1` plus exact current `collection-output:12` and
  `animation-pack-evaluation:3` authority, while preserving its no-reconstruction
  input gate.

Commercial reports and retained external evaluation workspaces are not schema
fixtures. They remain outside source and CI, and #574 stays input-blocked until
authorized exact authority exists.

The current V12 writer preserves sampling-refused sources and successful peers.
Evaluation-model V3 and dashboard V2 retain that partial evidence. Historical
V11/V2 remains accepted through the shared bounded capture/validator with immutable
revision pins. The V12 contract vector is manually constructed synthetic reader
evidence; bounded mixed-source CLI tests independently exercise the actual V12
producer, refusal ordering, identities and successful measurements.
