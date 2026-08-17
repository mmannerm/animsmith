# Assessment taxonomy

Use this taxonomy to keep evidence, readiness, and remediation claims
consistent across animation-pack reports.

## Contents

- [Evidence labels](#evidence-labels)
- [Coverage states](#coverage-states)
- [Decision vocabulary](#decision-vocabulary)
- [Readiness lanes](#readiness-lanes)
- [Remediation ownership](#remediation-ownership)
- [Future AnimSmith test](#future-animsmith-test)
- [Effort and confidence](#effort-and-confidence)
- [Issue severity](#issue-severity)

## Evidence labels

Attach one or more labels to every consequential claim.

| Label | Meaning | Acceptable evidence |
|---|---|---|
| `user-stated` | Supplied by the user but not independently present in the delivered evidence. | Explicit acquisition route, target-game requirement, intended use, or other scoped statement; retain the statement and unresolved corroboration separately. |
| `observed-file` | Directly established from delivered bytes. | File inventory, hash, parsed structure, or a deterministic tool result. |
| `observed-animsmith` | Produced by the captured AnimSmith version. | Preserved JSON/evidence plus command, version, config, and exit code. |
| `observed-report` | Visually inspected in an AnimSmith offline report. | Named report/clip and reviewer note. |
| `observed-engine` | Reproduced in the named engine version. | Import settings, logs, screenshots/video, and test procedure. |
| `vendor-stated` | Claimed by the seller or author. | Dated product page, manual, manifest, or direct communication. |
| `documentation-stated` | Specified by official engine, format, or tool docs. | Direct citation to the exact relevant version. |
| `inferred` | Reasoned from other evidence but not directly tested. | Explicit premise, reasoning, and confidence. |
| `not-evaluated` | No defensible result was obtained. | State why and what evidence would resolve it. |

Do not use `observed` without saying where it was observed. A user's
recollection is `user-stated`, not a transaction record. A vendor video is
`vendor-stated`, not engine or file evidence. A clean representative clip does
not establish a clean pack.

## Coverage states

Use one state for each promised evaluation cell:

- `evaluated-clean`: the intended work ran and produced no applicable finding
  at the stated policy level;
- `evaluated-finding`: the intended work ran and found a material result;
- `partially-evaluated`: some declared work ran and the missing portion is
  identified;
- `not-applicable`: the check or concern genuinely does not apply;
- `not-evaluated`: it could apply but was not run or lacked prerequisites;
- `unsupported-input`: the captured tool or engine could not consume it;
- `unavailable-evidence`: the full pack, license, engine, or project contract
  was not available.

Never rewrite a coverage gap, inactive check, not-applicable result, unsupported
format, or missing engine as `pass`.

## Decision vocabulary

Use one technical verdict:

- `Usable`: suitable for the stated game-engine use with no material unresolved
  technical conditions.
- `Usable with conditions`: technically useful when the listed remediation and
  validation gates are completed.
- `Restricted use`: suitable only for a named subset, prototype, placeholder,
  or constrained gameplay mode.
- `Poor fit`: material technical, artistic, or content gaps conflict with the
  stated use.
- `Insufficient technical evidence`: preview/partial access or missing runtime
  tests prevent a defensible technical conclusion.

Record evaluation completeness separately as `complete`, `partial`, or
`preview-only`. Evaluator setup failures, price, receipt/license provenance,
or marketplace availability may reduce completeness or add a provenance gate;
they do not change a pack's technical verdict unless they withhold the bytes or
rights needed for the stated technical evaluation.

Do not use a numeric total score. It hides veto conditions and creates false
precision across different genres. Use per-lane verdicts with evidence.

## Readiness lanes

Assign `ready`, `conditional`, `poor fit`, `not applicable`, or `unknown` to
each lane:

1. Delivery completeness and organization
2. AnimSmith-readable format coverage
3. Untouched mechanical clip health
4. Declared clip semantics
5. Set, sync, and locomotion-blend behavior
6. Rig, rest/bind, and retargeting behavior
7. Root-motion and in-place behavior
8. Target-engine import and playback
9. Masks, additive layers, IK, and attachments
10. Performance and runtime footprint
11. Game/content coverage and artistic fit
12. Cross-pack compatibility
13. Maintainability and reproducibility

Track acquisition, rights, and artifact provenance beside these technical lanes
instead of mixing them into a technical readiness verdict.

State the adoption consequence for every `conditional`, `poor fit`, or
`unknown` lane.

## Remediation ownership

Choose one primary class per issue. A secondary workaround may be listed, but
do not blur ownership.

| Class | Use when | Typical examples |
|---|---|---|
| `engine-config` | Source motion is acceptable and runtime interpretation or graph setup is the issue. | Avatar mapping, import scale setting, compression policy, root-motion toggle, mask asset, state-machine setup. |
| `animsmith-current-safe` | The captured version exposes a documented safe repair and dry-run confirms applicability. | Only the repairs actually shown by that version's help/docs. |
| `animsmith-current-declared` | The captured version exposes a mechanical operation but requires explicit project intent or selectors. | Slicing, hold extension, endpoint removal, gait anchoring, declared conversion/assembly/scale operations when available. |
| `animsmith-future-candidate` | The future-tool test below passes, but no current command solves it. | A fully declared, deterministic rewrite with a provable postcondition. |
| `artist-author` | Resolution requires creative, semantic, deformation-aware, or missing-data work. | Contact cleanup, new transition, pose/timing change, style match, hand correction, new additive base, deformation-aware retarget. |
| `vendor-license` | Delivery, source, support, or rights must change. | Missing promised files, broken archive, license ambiguity, redistribution restriction, vendor re-export. |
| `unknown` | Evidence is insufficient to select an owner. | Preview-only behavior, unavailable engine, missing skeleton, unclear clip intent. |

An available workaround does not prove the pack is fixed. Record whether it is
repeatable, lossless, project-specific, or artist-reviewed.

## Future AnimSmith test

Classify an issue as `animsmith-future-candidate` only when all answers are
yes:

1. Can the intended result be declared without guessing from appearance,
   character height, filenames, or genre conventions?
2. Can identical bytes, declarations, and tool version produce a deterministic
   candidate?
3. Can an independent proof or measurement verify the postcondition and the
   facts that must not change?
4. Is the behavior format-neutral at the core rather than policy for one game,
   engine, marketplace, or private consumer?
5. Can the operation refuse safely when evidence is incomplete or malformed?
6. Does it avoid inventing poses, contacts, timing, motion, or style?

If any answer is no, choose `artist-author`, `engine-config`, `vendor-license`,
or `unknown`. If all answers are yes, describe only potential feasibility.
Search current public AnimSmith issues before linking related work; absence of
an issue is not a roadmap commitment.

## Effort and confidence

Use effort bands with concrete tasks:

- `none`: no change beyond ordinary import/use;
- `low`: repeatable settings or one documented mechanical step;
- `moderate`: several per-pack operations, a custom contract, or limited art
  review;
- `high`: substantial DCC, retarget, content creation, or bespoke runtime work;
- `unknown`: evidence is insufficient for a responsible estimate.

Use confidence separately:

- `high`: exhaustive or risk-targeted direct evidence, reproducible commands,
  and target-engine confirmation;
- `medium`: strong file/tool evidence but incomplete runtime or pack coverage;
- `low`: sample, preview, vendor claims, or significant untested assumptions.

Do not convert effort bands into hours unless a named practitioner supplies an
estimate or the report states a scoped estimate and its assumptions.

## Issue severity

- `blocker`: prevents lawful acquisition, loading, required gameplay use, or a
  safe/reproducible pipeline;
- `major`: materially limits a core use case or requires high-cost remediation;
- `moderate`: usable with a meaningful caveat, workaround, or quality cost;
- `minor`: localized polish, organization, or optimization concern;
- `note`: useful context with no current adoption impact.

Severity describes impact for the stated game, not how easy a finding is to
detect or how severe the originating tool labels it.
