# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/mmannerm/animsmith/compare/v0.1.0...v0.2.0) - 2026-08-14

### Added

- *(assemble)* [**breaking**] remove selected node subtrees
- *(assemble)* [**breaking**] expose constant-track pruning
- *(scale)* [**breaking**] rebase rest-bind scale animation
- *(cli)* serialize each evidence record once and give assemble --format ([#340](https://github.com/mmannerm/animsmith/pull/340))
- *(core)* refuse a document whose two parent chains disagree ([#331](https://github.com/mmannerm/animsmith/pull/331))
- *(core)* count what each residual actually compared ([#322](https://github.com/mmannerm/animsmith/pull/322))
- *(cli)* publish scale artifacts and evidence as an atomic pair
- *(core)* add scale planning and proof contracts
- *(lint)* validate selected rest-world scale
- *(measure)* expose transform scale domains
- prune provably constant animation tracks
- detect time-complement sync pairs
- add sync-group timing diagnostics
- add required bone presence check
- add angular loop seam continuity
- detect duplicate loop endpoints
- add loop continuity checks
- measure mesh geometry centroid
- measure skeleton rest-pose domains
- report material and image measurements
- report additional skin influence sets
- add pinned clip duration expectations
- *(inspect)* inventory mesh instances
- *(cli)* verify complete assembly publication
- support PBR material texture slots
- *(cli)* add recipe-driven character assembly
- *(convert)* apply material texture recipes
- *(convert)* bake static mesh transforms
- *(convert)* preserve normal textures
- *(measure)* define static asset bound domains
- [**breaking**] finalize result contract v2
- add provisional v2 evaluation results
- *(core)* [**breaking**] share affine classification
- *(core)* make the proof record correct before it is frozen
- *(core)* publish the observed factor and prove unaffected binds
- *(core)* derive the scale tolerance policy and bound proof work
- *(gltf)* rewrite whole-document linear units on raw bytes
- *(core)* canonicalize skinned bind poses
- *(core)* add character assembly clip operations
- *(gltf)* reparameterize animated rest and bind hierarchies on raw bytes
- *(gltf)* add raw scale capability preflight
- bind reports to input bytes

### Fixed

- *(measure)* close final audit gaps
- *(measure)* [**breaking**] reconcile shared affine facts
- *(core)* [**breaking**] canonicalize the affine scale mean
- *(scale)* make artifact diagnostics structural
- *(scale)* expose artifact proof differences
- *(scale)* accumulate parent-chain rounding provenance
- *(scale)* prove unchanged skeleton placement
- *(scale)* reject negative primary skin weights
- *(core)* tolerate f32 rounding a rotation hid from the comparison base ([#333](https://github.com/mmannerm/animsmith/pull/333))
- *(cli)* flush a published temp through a writable handle
- *(cli)* a symlinked input must not be read through and then overwritten
- *(core)* close fail-open and precision defects in scale proof
- report emissive material texture bindings
- honor inclusive loop continuity caps
- align skeleton schema states
- simplify skeleton coverage states
- harden skeleton measurement validation
- resolve encoded external resource paths
- preserve per-primitive influence mismatches
- reject invalid duration pins
- *(inspect)* emit copyable TOML selectors
- *(inspect)* mirror assembly name ambiguity
- *(cli)* reject linked assembly textures
- *(cli)* gate assembly config provenance
- *(cli)* simplify loaded config evidence
- *(cli)* bind assembly config evidence
- *(cli)* satisfy assembly lint gates
- *(lint)* separate scale diagnostic ownership
- *(convert)* harden recipe path and image contracts
- *(convert)* preserve normal texture bake state
- *(convert)* harden static bake validation
- *(cli)* stream human-readable output
- preserve diff cardinality error precedence
- address final result-contract audit
- close final contract audit gaps
- enforce schema-valid embedded results
- validate embedded rig evidence
- close final result contract audit gaps
- close audit coverage gaps
- address result contract review findings
- harden final result contract
- close result contract audit gaps
- preserve v1 contracts in evaluation preview
- *(core)* widen connector planning spans
- *(core)* widen bridged linear rebase
- *(core)* widen bridged translation sum
- *(core)* close connector proof gaps
- *(core)* compose rest-bind through static connectors
- *(scale)* bind calibration to production comparisons
- *(scale)* prove mesh-instance placement, not just its payloads ([#324](https://github.com/mmannerm/animsmith/pull/324))
- *(core)* retract the impossibility claim and pin what it hid
- *(core)* bound the candidate side and correct the policy's own arithmetic
- *(core)* make the closure property true and charge the work proof does
- *(core)* report the rotation residual in the unit it declares
- *(lint)* preserve large scale ordering
- *(lint)* quantize derived scale evidence
- *(lint)* honor authored scale boundaries
- stabilize milliradian track comparisons
- preserve empty clip warning behavior
- retain expected duration on degenerate clips
- *(core)* derive skinned geometry bind space
- *(convert)* order bake evidence by source node
- bind rig evidence to resolved skeleton
- *(gltf)* refuse geometry accessors the reader cannot decode ([#326](https://github.com/mmannerm/animsmith/pull/326))
- *(gltf)* stop reading JSON null as a declared member, and wire the guards
- *(gltf)* move two source-validity checks into the capability gate
- *(gltf)* refuse out-of-contract nodes and pin the proof's claims
- keep image dimensions policy-neutral

### Added

- *(convert)* preserve normal textures through FBX ingestion and glTF scene round-trips, including glTF normal scale ([#222](https://github.com/mmannerm/animsmith/issues/222))

### Fixed

- *(core)* allow rest/bind scale planning to compose through finite static
  source-node connectors between projected joints while preserving each
  connector local transform, rebasing only its projected successor, and
  independently proving that connector span's exact raw write set
  ([#332](https://github.com/mmannerm/animsmith/issues/332))

### Changed

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
