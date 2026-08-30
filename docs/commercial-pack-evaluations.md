# Commercial-pack evaluation guide

The maintained [animation-pack evaluation reports](reports/README.md) index
the current Mixamo and Protofactor technical-report/evidence-appendix pairs.
They are scoped technical snapshots of authorized inputs, not redistribution
advice, a license opinion, or approval for a different target game.

## Read a pair in order

1. Read the **Technical decision** for the scoped verdict and its conditions.
2. Read **Capability coverage** and **Runtime sets and authored motion** to
   learn which logical clips, blend/transition/mask/contact sets, and gaps were
   actually evaluated.
3. Read **Engine status** as bounded evidence for the named engine procedure;
   it is not a transfer of proof to another engine, version, character, graph,
   or project.
4. Read the [**Technical issue register**](reports/protofactor-basic-locomotion.md#technical-issue-register)
   in the selected technical report to identify whether a source/vendor,
   DCC, engine, gameplay, or visual owner must act.
5. Open the paired **Detailed evidence** appendix for input identity,
   evaluator/version, commands, digests, coverage, and repeatable procedure.

The report index mechanically links every maintained report to its appendix.
Use the pair rather than an isolated headline when deciding whether a pack is
worth an engine trial.

## What to watch for

| Question | Where to read it | What it does not establish |
|---|---|---|
| Is the delivery technically useful for the named scope? | Technical decision and fit/limitations | A general “game-ready” certification. |
| Which clips and runtime sets have evidence? | Capability coverage and runtime sets | That undeclared, missing, or untested members behave the same way. |
| What needs work, and who owns it? | [Technical issue register](reports/protofactor-basic-locomotion.md#technical-issue-register) and evidence appendix | That a tool can automatically retarget, author a graph, or make an artistic choice. |
| Which engine behavior was observed? | Engine status and appendix procedure | Import, playback, blend, mask/contact, or visual success in another project. |
| Why might current and retained evidence differ? | `Changes between AnimSmith versions` | A current workflow rule or a new verdict by itself. |

Keep evaluator-change explanations in each report's `Changes between AnimSmith
versions` section. The workflow above is intentionally current-state routing:
for a new game, use the [game-developer intake workflow](game-developer-intake-workflow.md)
to establish its own engine-observed and visual gates.

Workflow pages contain only current reader steps. Tracker chronology and
internal rationale are excluded; evaluator changes stay in each report's
`Changes between AnimSmith versions` section.

Commercial files, excerpts, motion-bearing derivatives, and generated engine
projects stay outside the repository and CI. The checked-in reports preserve
scrubbed, reproducible evidence conventions while the source remains in its
authorized location.
