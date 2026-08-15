# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- *(convert)* preserve normal textures through FBX ingestion and glTF scene round-trips, including glTF normal scale ([#222](https://github.com/mmannerm/animsmith/issues/222))
- *(gltf)* add `rewrite_scale_plan`, a plan-taking raw scale writer for callers
  that reuse one compiled core plan across rewrite and artifact proof
  ([#374](https://github.com/mmannerm/animsmith/issues/374))

### Fixed

- *(core)* allow rest/bind scale planning to compose through finite static
  source-node connectors between projected joints while preserving each
  connector local transform, rebasing only its projected successor, and
  independently proving that connector span's exact raw write set
  ([#332](https://github.com/mmannerm/animsmith/issues/332))

### Changed

- *(core)* continue the behavior-neutral scale module split by moving the
  public facade to `scale/mod.rs`, its complete lib-target test and calibration
  graph to private `scale/tests.rs`, and the shared policy-neutral matrix
  rewrite, residual, and arithmetic-provenance leaves to private
  `scale/numeric.rs`, without changing public paths, policy, evidence, or proof
  behavior
  ([#383](https://github.com/mmannerm/animsmith/issues/383))
- [**breaking**] *(core)* remove the production-looking public
  `build_scale_candidate` API. Format frontends remain responsible for exact
  source rewriting and pass the emitted reload through
  `ScaleCandidate::from_document` to independent core proof; analytic tests
  can opt into the renamed reference constructor under the existing
  non-default `fixtures` feature. Scale behavior, evidence v1/v2/v3, and the
  `appendix-d-v6` policy remain unchanged
  ([#381](https://github.com/mmannerm/animsmith/issues/381))
- [**breaking**] *(core)* replace each public `ScaleProof` maximum/count field
  pair with one read-only `ScaleProofResidual`. Its `max()`, `comparisons()`,
  and `evaluated()` accessors keep a claim's measurements mechanically paired;
  scale-evidence v1/v2/v3 JSON and tolerance behavior remain unchanged
  ([#323](https://github.com/mmannerm/animsmith/issues/323))
- [**breaking**] *(core)* replace the public scale domain-rewrite and
  proof-obligation boolean bags/accessors with a non-exhaustive read-only typed
  plan ledger covering global canonical topology, numeric-value-free payload
  shape, explicit write-ownership dispositions with structural rewrite rules,
  and derived proof obligations. Scale-evidence v1/v2/v3 (including v3's five
  domain booleans), `ScaleProof` serialization, and the `appendix-d-v6` policy
  remain unchanged ([#374](https://github.com/mmannerm/animsmith/issues/374))
- *(gltf)* drive both raw scale writers from the compiled typed plan, pass one
  immutable plan through CLI rewrite and artifact proof, cross-check raw node
  hierarchy against canonical source topology, validate the complete plan
  inventory before raw replay, and keep proof-owned component selection and
  numeric derivations independent. At factor one, length-bearing node fields,
  accessor payloads, and authored bounds are now excluded from the raw write
  set under their compiled `PreserveExact` disposition. Parsed JSON numeric
  values therefore avoid narrowing through `f32`, and accessor bytes remain
  authored; JSON reserialization can still canonicalize lexical spelling.
  Otherwise equivalent artifact bytes and their publication digest can
  therefore change, and artifact proof checks factor-one node JSON and raw
  complement values exactly. Evidence v1/v2/v3 schema identities and shapes,
  selectors, tolerance policy, and refusal kinds remain unchanged; factor-one
  v3 rewrite inventories and `rewritten_accessor_count` now report the empty
  compiled write set
  ([#374](https://github.com/mmannerm/animsmith/issues/374))
- [**breaking**] *(assemble)* advance the character-assembly recipe and
  evidence contracts to immutable v3. Recipes can exact-name base nodes for
  fail-closed subtree removal after animation transforms, and evidence records
  every removed node in original hierarchy order. Recipe/evidence v1 and v2
  remain immutable historical contracts ([#350](https://github.com/mmannerm/animsmith/issues/350))
- [**breaking**] *(assemble)* advance the character-assembly recipe and
  evidence contracts to immutable v2. Recipes can opt into constant-track
  pruning after all other transforms; evidence records each removed track by
  original index, exact output bone identity, TRS property, interpolation, and
  key count. Recipe/evidence v1 remain immutable historical contracts, and the
  new recipe option defaults to `false` ([#349](https://github.com/mmannerm/animsmith/issues/349))
- [**breaking**] *(scale)* advance scale evidence to immutable v3: rest/bind
  now rebases valid affected-node scale animation (including cubic-spline
  tangents), publishes `result.domain_rewrites.scale_animation`, and raw glTF
  preflight refuses animation channels targeting a node authored with `matrix`.
  The immutable v1/v2 schemas remain historical, and the pre-1.0 public
  `ScaleError::AffectedScaleAnimation` variant is removed ([#352](https://github.com/mmannerm/animsmith/issues/352))
- [**breaking**] *(scale)* advance the Appendix D tolerance policy to
  `appendix-d-v6`: finite widened affine axis lengths are sorted ascending
  before averaging, making classification and observed factors independent of
  authored axis order; the pre-1.0 `APPENDIX_D_V5` associated constant is
  removed ([#361](https://github.com/mmannerm/animsmith/issues/361))
- [**breaking**] *(output)* advance measure, lint, and diff JSON to output v6
  and publish corrected affine linear-transform observations as measurements
  v12. The JSON shapes and vocabularies are unchanged, but the outer immutable
  identity advances because its schema statically pins the nested measurement
  URN; current `diff` rejects v5/v11 reports and directs operators to regenerate
  them from the original asset ([#355](https://github.com/mmannerm/animsmith/issues/355))
- [**breaking**] *(core)* expose typed `validate_document_shape` snapshot
  validation and collapse scale's former structural error variants into
  `ScaleError::InvalidDocumentShape` ([#293](https://github.com/mmannerm/animsmith/issues/293))
- [**breaking**] *(core)* move `AffineDomainViolation` to its canonical root/model path and remove the former `scale` module path ([#354](https://github.com/mmannerm/animsmith/pull/354))

## [0.1.0](https://github.com/mmannerm/animsmith/releases/tag/v0.1.0) - 2026-07-11

### Added

- *(measure)* mesh-level measurements from SceneAssets ([#16](https://github.com/mmannerm/animsmith/pull/16))
- *(gltf)* parse meshes/skins/materials into SceneAssets ([#16](https://github.com/mmannerm/animsmith/pull/16))
- [**breaking**] fix --dry-run is the repair check mode; drop repair groups
- polish public api and release readiness
- weld converted meshes into indexed primitives and embed textures
- convert carries meshes, skins, and materials (FBX2glTF replacement)
- add transform subcommand — slice, hold-extend, gait-anchor rotation
- add fix subcommand with quat hemisphere normalization
- M2 part 2 — self-contained HTML report with WebGL skeleton viewer
- M1 — rig profiles, config, and reference-parity semantic checks
- bootstrap animlint M0 — workspace, core model, glTF ingest, mechanical checks

### Fixed

- address writer summary review
- report converted artifact counts
- address release target audit findings
- *(examples)* make the walk fixture byte-stable across platforms
- sanitize non-finite mesh measurements; skip non-triangle prims ([#16](https://github.com/mmannerm/animsmith/pull/16))
- *(gltf)* guard mesh/skin accessor reads against count-0 panic ([#16](https://github.com/mmannerm/animsmith/pull/16))
- [**breaking**] unify SceneAssets into Document so transform keeps meshes
- compose selected repairs in fix pipeline
- honor min stride step config
- [**breaking**] reject malformed track data at load; panic-free sampling; GLB external-buffer fix writes
- treat sub-0.05 gait-phase drift as diff noise
- enforce schema_version in diff ingestion; honest FixError classification
- reject --dry-run with a write target; pin removed-flag and skip semantics
- address publishing audit findings
