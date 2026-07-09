# Animsmith Release Roadmap

Last reconciled: 2026-07-09

Scope note: this document records release intent — which capabilities
land in which release, and what must be researched before implementation.
GitHub milestones and issues are the source of truth for status; when
this document and the tracker disagree, the tracker wins and this file
needs a re-reconcile. Shipped kebab-case check ids (`loop-seam`,
`foot-slide`, …) are public contract; the dotted category names used in
the research notes (`root.classification`, `engine.bevy_import_prediction`)
are conceptual groupings, not proposed ids.

Inputs reconciled here:

- [`docs/research/game-ready-animation-clips.md`](research/game-ready-animation-clips.md)
  — the engine-survey research note whose recommendations this roadmap
  turns into milestone scope.
- [`DESIGN.md` §11](../DESIGN.md) — the original M0–M3 build-out roadmap,
  now essentially delivered; this file is the forward-looking successor.
- The GitHub [milestones](https://github.com/mmannerm/animsmith/milestones)
  and open issues as of the date above.

The research note's four-milestone arc (trustworthy audit → safe
preparation → engine feedback loop → retargeting diagnostics) maps onto
the releases below. Its first milestone is already largely shipped: the
17-check catalog, `inspect`/`measure`/`lint`/`report`/`transform`/`fix`/
`convert`/`diff`, versioned JSON output, the HTML report, rig profiles,
and CI-ready exit codes all exist today.

## 0.1.0 — First public release

Milestone: [0.1.0](https://github.com/mmannerm/animsmith/milestone/1).

The first public (alpha) release: production quality and fully
documented, but not yet validated in a real consuming project. The
dogfooding target is Rauta's CI/CD asset pipeline; learnings from that
usage re-scope 0.2.0 and 0.3.0 before further work is filed. Per the
milestone description, scope is hardening and documentation of the
existing surface — no net-new checks, transforms, or loaders — with one
deliberate exception noted below.

The release must let a stakeholder who has never seen the tool answer:
what is animsmith, why does it exist, what is it worth to me, and how do
I use it in my scenario.

Open at reconcile time: documentation-requirements umbrella
([#61](https://github.com/mmannerm/animsmith/issues/61)), examples and
tutorials tracker
([#68](https://github.com/mmannerm/animsmith/issues/68)), cookbook
motivation and routing
([#135](https://github.com/mmannerm/animsmith/issues/135)), and the
release-matrix field contract
([#141](https://github.com/mmannerm/animsmith/issues/141)).

New scope proposed by this roadmap (issue drafts in the
[appendix](#appendix-proposed-issue-filings)):

- **Stakeholder positioning and value-proposition page** (A1). The
  research note's positioning, stakeholder-value, and landing-page
  material as a durable doc: what animsmith is in one sentence, the four
  game-ready contracts in user terms, and the pitch per audience (artist,
  technical animator, gameplay engineer, producer, tool engineer).
  [#61](https://github.com/mmannerm/animsmith/issues/61) routes existing
  reference docs to audiences; this page is the missing "why should my
  team adopt this" layer above it.
- **Pipeline scenario guide** (A2). The raw-to-game-ready asset process
  and the named workflows teams actually run: marketplace-pack intake,
  mocap cleanup gate, outsourced-asset acceptance, CI gating on asset
  changes, raw-vs-transformed artifact storage.
- **Markdown output for lint findings** (A3). The one feature exception
  in 0.1.0: a lightweight Markdown rendering of findings for CI comments
  and reviews, needed by the Rauta dogfooding loop from day one. JSON
  remains the machine-readable source of truth per
  [output.md](output.md).

Existing unmilestoned issues proposed for re-milestone into 0.1.0, both
in scope per the milestone's own description (docs correctness and test
hardening):

- [#137](https://github.com/mmannerm/animsmith/issues/137) —
  single-source the check catalog with a drift guard. Directly protects
  the new stakeholder docs from drifting against the shipped checks.
- [#72](https://github.com/mmannerm/animsmith/issues/72) — strengthen the
  foreign-layout guard test.
- Optional, lower priority:
  [#73](https://github.com/mmannerm/animsmith/issues/73) and
  [#75](https://github.com/mmannerm/animsmith/issues/75) (version-test
  hardening).

Explicitly not 0.1.0: everything net-new surfaced by the research note —
engine profiles, new checks, sidecars, import prediction. Those are
0.2.0+.

## 0.2.0 — Safe preparation and contract depth

Milestone: [0.2.0](https://github.com/mmannerm/animsmith/milestone/2).

The research note's second milestone: grow the surface with mechanical,
check-verifiable capability. Everything here must stay inside the design
guardrail — animsmith rewrites clips only in ways whose correctness its
own checks can verify. This milestone is expected to be re-scoped once
Rauta dogfooding learnings arrive.

Open at reconcile time: per-bone loop-closure and seam-velocity checks
([#14](https://github.com/mmannerm/animsmith/issues/14)), pinned
`duration_s` expectations
([#15](https://github.com/mmannerm/animsmith/issues/15)), foot-cycle
time-warp transform
([#18](https://github.com/mmannerm/animsmith/issues/18)),
time-complement sync-pair detection
([#21](https://github.com/mmannerm/animsmith/issues/21)), sync-group
timing diagnostics
([#22](https://github.com/mmannerm/animsmith/issues/22)), and man page +
shell completions
([#87](https://github.com/mmannerm/animsmith/issues/87)).

New scope proposed by this roadmap:

- **Rauta pilot with exit criteria** (B0). The concrete issue behind the
  feedback loop this roadmap assumes: adopt the 0.1.0 release in Rauta's
  asset pipeline, and write the observed gaps back into this document
  before further 0.2.0/0.3.0 work is filed.
- **Duplicate loop-endpoint detection and safe removal** (B1). The
  research note flags the duplicated final frame as a top "bad cut"
  cause; detection plus a mechanical, verifiable removal transform.
  Scoped against `loop-seam` and
  [#14](https://github.com/mmannerm/animsmith/issues/14) so the three do
  not overlap.
- **Contact-event sidecar generation** (B3), gated on a **sidecar format
  spike** (B2). Generating footstep/contact events from the existing
  foot analysis is mechanical, but the sidecar file format (schema,
  versioning, stable clip ids, behavior under trim/resample) is a
  contract decision that deserves its own design pass first.
- **Blend entry/exit pose-delta checks** (B5), gated on a
  **transition-family config spike** (B4). Comparing entry/exit poses is
  measurement; declaring which clips form a transition family is a new
  config concept that must fit the existing `[clips]`/`[gait_groups]`
  shapes.
- **Provably equivalent track pruning** (B6). The research note's
  safe-preparation list includes dropping constant tracks and all-zero
  curves when the result is provably equivalent; the `constant-track`
  check already detects them, but no transform removes them yet.

One research safe-preparation item is deliberately deferred rather than
filed: an automatic before/after diff embedded in every transform's
output. The workflow already exists manually — `animsmith diff` compares
a raw asset against its transformed result — so wiring diff summaries
into `transform`/`fix` output waits for Rauta pilot evidence that the
manual step is real friction.

## 0.3.0 — Engine feedback loop

Proposed new milestone (not yet created; see
[filing plan](#filing-plan)).

The research note's third milestone: animsmith predicts, and where
practical verifies, real engine import behavior instead of only judging
the data in isolation. This is direction, not commitment — the milestone
is expected to be re-cut after 0.2.0 and the first rounds of Rauta
feedback.

Sequencing note: the research note ranks engine profiles as P0/P1,
trustworthy-audit work. Placing them here is a deliberate product
decision, not a straight translation — 0.1.0 is hardening-only by its
milestone definition, and the engine-profile config shape should be
informed by real Rauta usage (B0) before it becomes public contract,
rather than designed speculatively.

Proposed scope (issue drafts in the appendix):

- **Engine-profile config model spike** (C1). The gating design question
  for the whole milestone: engine profiles (`generic`, `unity-generic`,
  `unity-humanoid`, `unreal`, `godot`, `bevy`, `gltf-runtime`) as a
  concept distinct from rig profiles, defining root policy, unit/axis
  expectations, clip boundary rules, loop/contact thresholds, and
  unsupported-track behavior. The config shape must be consistent with
  the existing `[rig]`/`[checks]`/`[clips]` schema from the start —
  an asymmetric bolt-on here becomes a breaking cleanup later.
- **Engine import prediction checks** (C2). Profile-driven checks for
  what an engine will reject, drop, or reinterpret: Unreal whole-frame
  end ranges and root-bone requirements, Bevy unsupported glTF
  extensions and unnamed animations, Unity root-transform
  interpretation. Filed as one umbrella with a per-engine task list;
  split per engine if the umbrella proves unwieldy.
- **Engine import preset generation** (C3). Suggested Unity/Unreal/Godot
  import settings derived from measured data: loop flags, root motion
  source, sample rate, clip ranges.
- **Bevy asset manifest generation** (C4). Bevy readiness is partly
  asset addressability: stable `GltfAssetLabel` paths, named-animation
  inventories, and target-id reports replace fragile hardcoded indices.
  For Rauta specifically, the manifest plus an adapter into its
  programmatic animation-graph setup is the high-leverage surface.
- **Bevy animation-graph template demand spike** (C5). Downgraded from a
  committed feature: Rauta builds its animation graphs programmatically
  and deliberately skipped graph-asset editing, so generated RON graph
  templates have no confirmed consumer. The spike assesses demand and
  shape (template vs manifest-driven adapter) before any implementation
  is filed.
- **Per-engine profile guides** (C6). One docs page per profile (Unity,
  Unreal, Godot, Bevy, glTF/generic): what the engine expects, what
  animsmith checks, common failures, threshold config, and DCC/import
  fixes.
- **Engine smoke-test feasibility spike** (C7). Headless Unity/Unreal/
  Godot import runs and a Bevy runtime harness are attractive but carry
  licensing, CI-cost, and maintenance questions. The spike's outcome
  decides whether smoke tests enter 0.3.0 or stay backlog.

## Backlog

Unmilestoned; revisited after Rauta learnings. Filed as issues (appendix
D-series) so they are visible and searchable, but deliberately not
scheduled:

- **Retargeting diagnostics** (D1): bone-map quality, rest-pose delta,
  bone-length delta, and a retarget risk score. The research note's
  fourth milestone — diagnostics only, never silent retargeting.
- **Compression readiness estimation** (D2): estimated error under key
  reduction and a jitter-risk score. Needs metric research before
  implementation; a spike is filed when someone picks it up.
- **Metadata and required-events checks** (D3): required events per clip
  family, event/window ordering, alignment with inferred contacts.
  Depends on the sidecar (B2/B3) and config work landing first.
- **Batch library inventory and dashboard** (D4).
- **DCC integrations** (D5): Blender/Maya validate-before-export
  helpers.
- **Additional report serializers** (D6): SARIF/JUnit/CSV, already
  anticipated as future serializers by [output.md](output.md).

## Needs an ADR before becoming issues

Per the design guardrail (animsmith may rewrite clips only in ways its
own checks can verify), these research-note ideas are out of scope until
a design-record update deliberately admits them. They are recorded here
so they are not re-proposed piecemeal:

- Root-motion ↔ in-place conversion.
- Retargeting and rest-pose rewriting.
- Procedural foot locking, motion warping, stride normalization.
- Key reduction with nonzero visual/motion error tolerance.
- Loop-pose correction distribution across a clip.

## Verification notes

Claims from the research note to re-verify at implementation time rather
than trust from the page:

- Unreal's fractional-end-frame import behavior — re-check against the
  current UE release before implementing the whole-frame-end check.
- Bevy's glTF extension support (e.g. `KHR_animation_pointer`
  unsupported) is pinned to Bevy 0.19 in the note — re-verify per Bevy
  release when C2/C4 are implemented.
- Any check-id taxonomy change (dotted names, renames) is a separate
  pre-1.0 design decision, not implied by this roadmap.

The feedback loop this roadmap assumes: 0.1.0 ships → the Rauta pilot
(B0) adopts it in CI/CD → observed friction and gaps are written up →
0.2.0 and 0.3.0 scope is re-reconciled here before further issues are
filed. B0 is the tracked issue that makes this loop concrete instead of
aspirational.

## Filing plan

Executed only after this document is reviewed:

1. Create milestone **0.3.0 — Engine feedback loop** with a description
   matching its section above (including the "direction, expected to be
   re-cut" caveat).
2. File the appendix issues with the listed milestones and labels. The
   `spike` label does not exist yet and needs to be created for the
   B2/B4/C1/C5/C7 issues.
3. Re-milestone [#137](https://github.com/mmannerm/animsmith/issues/137)
   and [#72](https://github.com/mmannerm/animsmith/issues/72) into
   0.1.0; [#73](https://github.com/mmannerm/animsmith/issues/73) and
   [#75](https://github.com/mmannerm/animsmith/issues/75) if accepted.
4. Back-fill the filed issue numbers into the appendix below.

## Appendix: proposed issue filings

Each entry: title, milestone, labels, blockers, and a body draft. Bodies
get expanded at filing time with links back to the relevant research and
roadmap sections.

### A1 — docs: stakeholder positioning and value-proposition page

Milestone 0.1.0 · `documentation`, `type:docs` · no blockers.

> Add a stakeholder-facing page under `docs/` answering: what is
> animsmith in one sentence, what problem does it solve, and what is it
> worth per audience (artist, technical animator, gameplay engineer,
> producer, tool engineer). Present the four game-ready contracts
> (runtime, character controller, retargeting, pipeline) in user terms.
> Complements #61 (reference-doc routing) and `docs/game-ready-clips.md`
> (failure-symptom explainer); linked from `README.md` and
> `docs/README.md`. Source material: research note §"Product
> Positioning", §"Stakeholder Value", §"Documentation Landing Page".

### A2 — docs: pipeline scenario guide (raw to game-ready)

Milestone 0.1.0 · `documentation`, `type:docs` · no blockers.

> Document the raw-to-game-ready asset process (research note §"Common
> Asset Process") and the concrete workflows teams run: marketplace-pack
> intake, mocap cleanup gate, outsourced-asset acceptance, CI gating on
> asset changes, and raw-vs-transformed artifact storage. Each scenario
> names the animsmith commands and config involved and cross-links the
> cookbook (`examples/README.md`) and `docs/embedding.md`. Distinct from
> #68 (runnable tutorials): this is the process-level guide those
> tutorials plug into.

### A3 — feat: Markdown output for lint findings

Milestone 0.1.0 · `type:feature`, `priority:medium` · no blockers.

> Add a Markdown rendering of lint findings suitable for CI comments and
> asset-review threads (likely `lint --format markdown`; exact surface
> decided in-issue, consistent with the existing `--format text|json`).
> JSON stays the machine-readable source of truth (`docs/output.md`);
> Markdown is presentation only, no schema guarantees. This is the one
> deliberate feature exception in the 0.1.0 hardening milestone, needed
> for the Rauta CI dogfooding loop.

### B0 — Rauta pilot: dogfood the 0.1.0 release with exit criteria

Milestone 0.2.0 · `type:chore`, `priority:high` · no blockers (starts
when 0.1.0 ships); blocks further 0.2.0/0.3.0 filing per the roadmap's
feedback loop.

> Adopt the released 0.1.0 in Rauta's asset pipeline: build a Rauta
> `animsmith.toml` (or embedder adapter) from the existing asset
> contract and metadata, run `measure`/`lint`/`diff` against
> `assets/models/character.glb` in CI, and compare results against
> `rauta-asset-contract`. Exit criteria: the observed gaps, friction,
> and false positives/negatives are written up and reconciled back into
> `docs/ROADMAP.md`, re-scoping 0.2.0/0.3.0 before further issues from
> this roadmap are filed.

### B1 — lint+transform: duplicate loop-endpoint detection and removal

Milestone 0.2.0 · `type:feature`, `priority:medium` · no blockers.

> Detect loops whose final key duplicates the first pose (the classic
> hitch-causing "frame 0 and frame N both present" cut) and offer a
> mechanical removal transform whose result the loop checks can verify.
> Must be scoped against `loop-seam` and #14 so detection, per-bone
> closure, and endpoint dedup do not overlap.

### B2 — Spike: contact-event sidecar format

Milestone 0.2.0 · `spike`, `type:feature` · blocks B3.

> Decide the sidecar file contract for generated animation events:
> format, schema versioning (alignment with the `schema_version`
> discipline of the JSON envelope), stable clip identifiers, time
> coordinates, and behavior under trim/slice/resample. Must explicitly
> decide how animsmith sidecars interact with Rauta's existing measured
> sidecars — a compatible producer/consumer boundary, not a second
> source of truth. Outcome is a documented format decision, not code.

### B3 — transform: generate contact-event sidecar from foot analysis

Milestone 0.2.0 · `type:feature` · blocked by B2.

> Generate footstep/contact events (left/right markers, contact windows)
> from the existing foot-contact analysis into the sidecar format decided
> in B2. Mechanical and verifiable: events must align with the inferred
> contact frames the checks already compute.

### B4 — Spike: transition-family config model

Milestone 0.2.0 · `spike`, `type:feature` · blocks B5.

> Design how a project declares transition families (which clips are
> expected to blend into which) in `animsmith.toml`, consistent with the
> existing `[clips]` glob and `[gait_groups]` shapes. Outcome is a config
> schema proposal.

### B5 — lint: blend entry/exit pose-delta against transition families

Milestone 0.2.0 · `type:feature` · blocked by B4.

> Check that entry/exit poses of clips in a declared transition family
> are within tolerance, so state-machine transitions do not pop. Pure
> measurement once B4 defines the family declaration.

### B6 — transform: provably equivalent track pruning

Milestone 0.2.0 · `type:feature`, `priority:medium` · no blockers.

> Add mechanical removal of constant tracks below tolerance and all-zero
> curves not required by the profile, when the result is provably
> equivalent under the existing measurement grid. The `constant-track`
> check already detects these; this adds the verifiable transform the
> research note's safe-preparation milestone calls for.

### C1 — Spike: engine-profile config model

Milestone 0.3.0 · `spike`, `type:feature`, `priority:high` · blocks C2,
C3, C4, C6.

> Design engine profiles as a first-class config concept distinct from
> rig profiles: `generic`, `unity-generic`, `unity-humanoid`, `unreal`,
> `godot`, `bevy`, `gltf-runtime`. A profile defines root policy,
> unit/axis expectations, clip boundary rules, loop/contact thresholds,
> and unsupported-track behavior. Must compose cleanly with the existing
> `[rig]`/`[checks]`/`[clips]` schema — get the shape right before it
> ships rather than deferring a breaking cleanup. Outcome is a design
> proposal (likely a DESIGN.md update).

### C2 — lint: engine import prediction checks per profile

Milestone 0.3.0 · `type:feature` · blocked by C1.

> Umbrella for profile-driven import-prediction checks: Unreal
> whole-frame end ranges and root-bone presence, Bevy unsupported glTF
> extensions and unnamed animations, Unity root-transform
> interpretation, Godot slice/retarget expectations. Per-engine task
> list inside; split into per-engine issues if the umbrella proves
> unwieldy. Re-verify each engine claim against current engine docs at
> implementation time.

### C3 — feat: engine import preset generation

Milestone 0.3.0 · `type:feature` · blocked by C1.

> Generate suggested import settings per engine from measured data: loop
> flags, root-motion node/bone, sample rate, clip ranges, compression
> tolerances. Output format per engine decided in-issue.

### C4 — feat: Bevy glTF asset-label and named-animation manifest

Milestone 0.3.0 · `type:feature` · blocked by C1.

> Generate a Bevy-facing manifest of a glTF asset: scenes, animations,
> named animations, skins, animation target ids, root candidates, and
> typed `GltfAssetLabel` paths, so Bevy projects stop relying on fragile
> numeric indices and misspellable string labels.

### C5 — Spike: Bevy animation-graph template demand and shape

Milestone 0.3.0 · `spike`, `type:feature`, `priority:low` · blocked by
C4.

> Assess whether generated RON animation-graph templates have a real
> consumer before committing to them: Rauta builds its graphs
> programmatically and deliberately skipped graph-asset editing, so a
> C4 manifest plus a Rauta adapter into its `AnimationKind` setup may be
> the higher-leverage surface. Outcome: a demand/shape decision
> (template, manifest-driven adapter, or drop) — implementation is filed
> only if the spike finds a consumer.

### C6 — docs: per-engine profile guides

Milestone 0.3.0 · `documentation`, `type:docs` · blocked by C1.

> One page per engine profile (Unity, Unreal, Godot, Bevy, glTF/generic):
> what the engine expects, what animsmith checks under that profile,
> common failures, threshold configuration, and how to fix findings in
> the DCC or engine import settings.

### C7 — Spike: engine import smoke-test feasibility

Milestone 0.3.0 · `spike`, `type:feature` · no blockers.

> Assess headless import smoke tests: Unity/Unreal/Godot licensing and
> CI cost, and the shape of a Bevy runtime harness that loads an asset
> and captures import warnings for comparison against animsmith's
> predictions. Outcome decides whether smoke tests enter 0.3.0 scope or
> stay backlog.

### D1 — feat: retargeting diagnostics

No milestone (backlog) · `type:feature`, `priority:low` · no blockers.

> Bone-map quality (matched/missing/duplicate/extra/parent-mismatch),
> rest-pose delta, bone-length delta, and an overall retarget risk
> score against a target skeleton profile. Diagnostics only — actual
> retargeting stays behind an ADR.

### D2 — feat: compression readiness estimation

No milestone (backlog) · `type:feature`, `priority:low` · no blockers.

> Estimate error under key reduction and score high-frequency jitter
> risk so teams see compression problems before engine import. Needs
> metric research first; file a spike when picked up.

### D3 — lint: required events and metadata checks

No milestone (backlog) · `type:feature`, `priority:low` · blocked by B2,
B3.

> Check that clips carry the events/windows their clip family requires,
> that windows are ordered, and that declared events align with inferred
> contacts. Depends on the event sidecar contract.

### D4 — feat: batch library inventory and dashboard

No milestone (backlog) · `type:feature`, `priority:low` · no blockers.

> Library-wide inventory and quality metrics across an asset directory:
> per-clip status, metric trends, and review-ready summaries.

### D5 — feat: DCC validate-before-export helpers

No milestone (backlog) · `type:feature`, `priority:low` · no blockers.

> Blender/Maya helper scripts that run animsmith before export and open
> reports at the failing frame/bone.

### D6 — feat: SARIF/JUnit/CSV report serializers

No milestone (backlog) · `type:feature`, `priority:low` · no blockers.

> Additional serializers over the versioned JSON envelope for code-review
> and CI systems that natively ingest SARIF or JUnit, as anticipated by
> `docs/output.md`.
