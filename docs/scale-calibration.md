# Scale proof calibration

This document owns the reproducible calibration populations, measured demand,
historical policy comparisons, and machine-local timing notes behind
`ScaleTolerancePolicy::APPENDIX_D_V6`. The normative operation, tolerance,
provenance, work-budget, proof, and publication contracts remain in
[DESIGN.md Appendix D](../DESIGN.md#appendix-d--decision-record-skinned-restbind-scale-canonicalization).

The calibration is evidence for the fixed policy, not another policy authority.
Changing an accepted bound, work budget, magnitude definition, sampling rule, or
proof comparison requires a new policy identity and corresponding normative
design change; editing the measurements here cannot change runtime behavior.

## Rounding calibration sweep

`4` is measured, not assumed, and the measurement is checked in:
`calibrate_f32_rounding_ulps` in
`crates/animsmith-core/src/scale/tests.rs`. Run it with

```text
cargo test -p animsmith-core --release --lib \
    calibrate_f32_rounding_ulps -- --ignored --nocapture
```

It builds and proves 360_000 correct candidates over 144 cells: the cross
product of nine operations (rest/bind at root scale `3190`, and whole-document
conversion at `{1e-4, 0.01, 0.1, 1.5, 7.3, 100, 3190, 1e6}` — both directions
of the factor), four slot compositions (analytic binds, where `abs(W * B)` is
`1`, and composed slots at `abs(W * B) = {1e-3, 1, 1e3}`), two blends (two
slots that oppose on the swept vertex and cancel it to the origin under
balanced weights, and two independent slots that do not), and two explicit
weight profiles. The balanced profile uses `[0.5, 0.5]`. In the mismatched
profile, every vertex whose two production influence bases differ gives the
larger base a log-uniform weight in `[1e-20, 1e-2]` and the smaller base weight
`1`. The measured population contains 274_670 such vertices — 74_085 with slot
0 larger and 200_585 with slot 1 larger — and the sweep asserts strong nonzero
floors for both orientations rather than merely counting profile labels. Each
cell draws 2500 candidates. Joint locals and vertex positions are drawn
log-uniformly over six and eight decades in random directions, every joint
carries a random rotation, and half of every cell's trials carry a parent chain
that cancels. The generator is a written-out SplitMix64 whose seed is derived
from each cell's own coordinates — conversion, composition, blend and weight
profile — rather than from its position in the loop, so a cell's population
does not move when a neighbouring cell is added, removed or reordered.

The *bit stream* is identical on every machine and every run; the rigs drawn
from it are not quite. The draws run `f64::powf` for the magnitude decades,
`f64::sin`/`cos` for the directions, and glam's `f32` sine and cosine for the
rotations, none of which any platform is required to round identically. Two
machines can therefore differ in the last ulp of a joint local and in the last
printed digit of the worst demand. The table below records the reference run
for this policy revision; the checked test asserts platform-tolerant
non-silence floors and the strict `< 4` safety ceiling rather than pretending
those libm-dependent last digits are normative.

| quantity | worst over the population |
|---|---|
| skinned bounds | `2.768` |
| the skin equation | `2.213` |
| rest translation | `2.583` |
| refused correct candidates | `0` of `360_000` |

The quantity is `residual / (magnitude * 2^-23)`: the raw ulp count, *not* net
of the scalar band that is paid first. It therefore overstates what the count
is actually asked for, by the whole scalar band, and a worst case under `4`
measured this way is a worst case under `4` however the two terms are split. An
earlier revision of this section reported the net demand instead, from
populations that were never checked in and so could not be re-derived; the
figures above replace them.

The 144-cell phase remains intentionally shallow. A separate deep-chain phase
proves 80 correct animated candidates at depths
`8, 16, 32, 64, 128, 192, 256, 512`. At every
depth it covers rest/bind and the eight whole-document conversions; the
whole-document cases use the literal `170`-degree chain, while rest/bind uses
an exact half turn because repeating the approximate quaternion eventually
leaves the operation's supported affine-input class. Eight additional
whole-document cases use a literal step that closes a 192-link ring. In test
builds the central production comparison records each raw ULP demand at the
exact point its residual and provenance meet. Both shallow and deep calibration
fold those private maxima; the deep phase also folds production-owned counts.
The released proof layout and runtime path are unchanged, and calibration no
longer rebuilds a parallel pose/slot/bounds walk or divides unrelated
obligation-wide maxima.

| deep-chain quantity | worst over 80 cases | comparisons |
|---|---:|---:|
| rest translation | `0.715` | `12_488` |
| sampled trajectory | `0.715` | `24_976` |
| the skin equation | `0.143` | `240` |
| skinned bounds | `0.149` | `1_440` |
| refused correct candidates | `0` | — |

The harness retains and prints a separate record for every declared depth,
rather than folding the individual demands immediately into the aggregate
above. The depth-192 row also includes the eight ring cases; every other row
contains rest/bind plus the eight whole-document conversions.

| depth | rest | trajectory | skin equation | bounds | comparisons (rest / trajectory / skin / bounds) |
|---:|---:|---:|---:|---:|---:|
| `8` | `0.578` | `0.578` | `0.143` | `0.149` | `81 / 162 / 27 / 162` |
| `16` | `0.578` | `0.578` | `0.067` | `0.073` | `153 / 306 / 27 / 162` |
| `32` | `0.578` | `0.578` | `0.076` | `0.078` | `297 / 594 / 27 / 162` |
| `64` | `0.578` | `0.578` | `0.063` | `0.063` | `585 / 1_170 / 27 / 162` |
| `128` | `0.578` | `0.578` | `0.037` | `0.039` | `1_161 / 2_322 / 27 / 162` |
| `192` | `0.715` | `0.715` | `0.034` | `0.033` | `3_281 / 6_562 / 51 / 306` |
| `256` | `0.578` | `0.578` | `0.034` | `0.034` | `2_313 / 4_626 / 27 / 162` |
| `512` | `0.578` | `0.578` | `0.019` | `0.019` | `4_617 / 9_234 / 27 / 162` |

An unaffected instance's binds demand `0`, and always will: a correctly built
candidate leaves both effective matrices equal, including when one side stores
an explicit identity and the other uses the format-defined identity default.

`4` is the next power of two above every figure above, **measured over these
declared populations**. It is not an analytic bound. An earlier revision
reasoned from one four-term matrix inner product; v4 exposed the missing
dimension because one composition happens per link. Its maximum over links
stayed flat while coherent rounding accumulated with depth, and correctly
built candidates began refusing at depth 180.

**Historical v5 calibration of the recurrence retained by v6.** The v5 policy
replaced the depth-flat maximum with the additive provenance recurrence that
v6 still ships. Its exact formula, scope, and fixed-stage semantics have one
normative owner in [DESIGN.md Appendix D](../DESIGN.md#appendix-d--decision-record-skinned-restbind-scale-canonicalization).
This note retains the evidence for that choice. A 512-link zero-translation
chain adds nothing below a translated root; the corresponding chain of
underflowed nonzero translations carries only its vanishing contribution
instead of charging the million-unit parent 512 times. An uneven chain accrues
its actual per-link bases rather than `depth * max`, and the literal 192-link
ring plus the `170`-degree chain prove through depth 512. The checked-in shallow
and deep calibration and the isolated floors below delimit the empirical
envelope retained by v6.

A residual above the count is evidence about the *magnitude* before it is
evidence about the count. Three revisions have now found the magnitude wrong
rather than the count too small — the skinned extent alone, which missed the
`W * B` composition; `abs(W) * abs(B)` alone, which missed what the parent
chain had already cancelled; and `abs(p)` alone, which missed the other factor
of the transform it was named for — and in all three the measured excess was
hundreds or hundreds of thousands of ulps, not a factor of two. Each was
invisible because the calibration population could not express the shape:
no rig in it composed a slot to anything but the identity. That is what the
checked-in sweep is for, and why its cells name the shapes rather than only
the magnitudes.

**The cost is real and it is not bounded.** The four chain-dominant fixtures
search their declared positive-binary32 intervals in the normal suite, sample
4097 evenly spaced bit coordinates as a gross non-monotonicity guard, and pin
the adjacent accepted/refused transition found by bisection plus the exact
refused residual/tolerance bits. This is not an exhaustive monotonicity proof.
The four searches remain comfortably sub-second in the debug reference run,
so a second ignored-only search path would add structure without buying
practical test time. The v4 values are frozen historical measurements from the
exact merged commit `0a253228dc2d557a9030cfd72f2b15326f4853bd`, independently
reproduced during audit with the same fixtures and mutation axes. The v5 suite
compares its production endpoints with those recorded bits; it does not carry
a second implementation of the retired policy.

| obligation / mutation | v4 refused transition | v5 adjacent accepted / refused | v5 refused-endpoint observed / tolerance |
|---|---:|---:|---:|
| RestTranslation, unskinned rest x | `3.1875` (`0x404c0000`) | `4.5625` / `4.5625005` (`0x40920000` / `0x40920001`) | `4.5686544` / `4.5633805` |
| Trajectory, both unskinned key x values | `3.1875` (`0x404c0000`) | `4.5625` / `4.5625005` (`0x40920000` / `0x40920001`) | `4.5686544` / `4.5633805` |
| SkinMatrix, cancelling-joint inverse-bind x | `3.5713947` (`0x406491bb`) | `5.3570914` / `5.357092` (`0x40ab6d4b` / `0x40ab6d4c`) | `4.5633788` / `4.5633783` |
| Bounds, chain-dominant point y | `2.9826486` (`0x403ee3b7`) | `4.503774` / `4.5037746` (`0x40901eeb` / `0x40901eec`) | `4.5633788` / `4.5633786` |

Those are fixture-local transitions, not global smallest defects. The typed
refused endpoint isolates the named obligation: the rest/trajectory bone is not
skinned, the inverse-bind mutation reaches SkinMatrix before Bounds, and the
million-unit x coordinate gives MeshPosition a wider band than the Bounds y
mutation. Reverting the recurrence to v4's maximum reproduces the recorded v4
boundary and fails the strict v5 comparison.

The general cost remains unbounded relative to the compared quantity.
`4 * 2^-23` is `4.77e-7` of **the magnitude the arithmetic ran on**,
which is `4.77e-7` of the compared quantity only when the two coincide and is
otherwise `4.77e-7 * (operand magnitude / compared magnitude)` of it. That
ratio has no upper bound, and it is exactly the ratio the term exists to
cover, so the term and the compared quantity diverge precisely as far as the
operand/result cancellation documented above.

Concretely, on the far-joint rig of
`a_joint_far_from_the_geometry_it_carries_still_proves_its_bounds` — joints
`3.2e6` from the origin carrying geometry within one unit of themselves, so
`W * B` is near-identity — the term is `4.44` against a compared magnitude of
`1.0`, and the largest inverse-bind `x` shift still *accepted* is `4.09375`
units — the smallest refused is the next binary32 above it. A regenerated bind
wrong by four units is accepted there. The bracket is pinned by
`the_far_joint_rig_admits_a_four_unit_bind_shift_and_refuses_the_next_one_up`,
because a floor quoted in prose and nowhere held to drifts: an earlier
revision of this section stated `4.09`, which is on the accepted side of the
real floor and so described a bracket that does not exist. As a rule: for a rig
whose joints sit `k` times further from the origin than the geometry they
carry, `SkinMatrix` and `Bounds` lose discriminating power in proportion to
`k`.

Folding the parent chain into the magnitude does not move that number: on the
far-joint rig `abs(W) * abs(B)` already reads `6.4e6` against the chain's
`3.2e6`, so the max is unchanged and the floor is still `4.09375` units.
The chain widens the magnitude only where a chain
actually cancelled, always in proportion to what it cancelled, and leaves it
untouched everywhere else. Buying the same admissions by raising the count
instead would have cost the whole factor on *every* slot, including the ones
that lost nothing, which is why the correction is to the magnitude and the
count stays at `4`. That is the general rule this section applies each time:
`a_parent_chain_whose_translations_cancel_still_proves_its_skin` needs
`524288` ulps of the base without the chain and `0.08` of it with, and no
count between those two is a policy anyone could defend.

**This is a property of the `f32` inputs, not of the policy, and precision
does not fix it.** The dominant error is not the composition's own rounding.
It is the stored inverse bind's translation column — accurate only to its own
ulp — amplified by `W`'s linear part into a product that cancellation has made
near-identity. Composing `W * B` in `f64` from the same `f32` stored values
was measured over a 30_000-candidate rest/bind population: it moves the worst skin
residual from `2.50` to `2.06` ulps and the worst bounds residual from `1.68`
to `0.90`, an improvement of under a factor of two, and leaves the worst
residual at `86 %` of the compared product's own magnitude against a `1e-5`
relative band. On the far-joint rig it moves the floor from `4.09375` units to
about `4.16` at the same four ulps, or about `3.16` at the three ulps that
residual would then permit — those two are approximate, being measurements of
a composition this tree does not perform and so not pinned by any test here. The term covers a quantization the file itself
already performed, and no amount of proof-side precision can undo it.

What the term does buy is the direction that matters: no obligation is held to
a band tighter than the rounding its own arithmetic genuinely incurs, which
none of them was ever sound doing. The converse — that each obligation refuses
every error larger than its rounding — is a stronger claim than anything here
establishes. What is pinned is one adjacent bracket per named mutation axis,
including the far-joint inverse-bind bracket at `4.09375` accepted and the next
binary32 value refused.

Transform-only rounding and overflow/refusal semantics are current policy, not
calibration conclusions, and therefore live only in
[DESIGN.md Appendix D](../DESIGN.md#appendix-d--decision-record-skinned-restbind-scale-canonicalization).
Their checked evidence includes
`a_rig_whose_skinned_extent_passes_the_square_root_of_f32_max_still_proves`,
`a_rig_whose_composition_operands_overflow_f32_still_proves`, and
`a_parent_chain_whose_operand_sums_overflow_f32_still_proves`.


## Sampling-budget timing history

`4e8` is a wall-time ceiling expressed in work units. What that ceiling costs
in seconds is a property of the machine, not of this design. Historical v3
measurements were **linear in the charge** across four doublings of the budget
in both shapes, which is the evidence that the charge is a proxy for real work
rather than an arbitrary count. Both shapes are fully specified here, so a
reader can rebuild them and measure their own machine: the
slot-dominated one is 200 instances of a 99-joint skin list with one vertex
each, and the vertex-dominated one is a single instance of that skin list with
a 10_000-vertex primitive, each with as many sample times as `4e8` admits, and
**every vertex in both carries exactly one non-zero influence**.

The released arithmetic also gives reproducible sizing landmarks independent
of machine timing. Appendix D's 200-bone, 100k-vertex example costs `201_000`
units per sample, so `4e8` admits 1,990 samples; the same rig with 10k vertices
costs `21_000` and admits 19,047. A 100k-sample clip on the first rig asks for
`2.01e10` units and is refused. Its 30-second, 30-fps example costs
`180_900_000`, leaving about `2.2x` headroom, or roughly 66 seconds at 30 fps
for that exact shape. These are consequences of the released formula, not
wall-time measurements.

The one-influence condition above is part of the shape and not an incidental
detail. The budget charges per *vertex*, but the `f32`-rounding term's stage-1
work runs once per non-zero *influence* — the skinning loop `continue`s on a
zero weight — so a
rig whose vertices carry four influences pays that stage four times per vertex
against the same charge. The seconds below therefore describe a one-influence
shape and understate a four-influence one; the work units are identical for
both. Appendix D §D.1 names that vertex-versus-influence discrepancy as part of
the normative work-budget definition so a rebuilder does not have to infer it
from this historical timing population.

On one developer machine under v3 the slot-dominated shape at the ceiling
measured `6.7s` and the vertex-dominated one `3.3s`, a ratio of `2.0`. Neither
number is a bound this design guarantees or a v4 measurement, and an earlier
revision of this section
claimed one — "stays inside five seconds" — against a measurement taken on
different hardware and a different tree. The ratio moved too: before
`f32_rounding_ulps` the same machine measured `7.1s` and `2.0s`, a ratio of
`3.5`. The absolute seconds are what a reader cannot check; the linearity and
the ordering are what they can.

**Historical v3 cost of the `f32`-rounding term.** Deriving each obligation's
magnitude was per-slot and per-vertex work that the tree before
`appendix-d-v3` did not do, and the vertex-dominated shape is where it showed.
Two baselines are quoted here and they are not the same baseline, so both are
named. Against **no rounding term at all** — the `appendix-d-v2` tree — on one
machine, over twelve runs of each shape and taking minima, that shape went from
`2.02s` to `3.31s`, `+64 %`, for the same 1_932_084 skin and 117_096 bounds
comparisons.

Against **the term with stage 1 read as `abs(p)`** — the form this design
carried mid-revision, before stage 1 was corrected to the transform's operand
product — the correction alone costs `+27 %`. Measured in one session on the
same shape and machine, minimum of five runs: `2.57s` with no stage-1 term at
all, `2.87s` reading `position.length()`, `3.64s` reading
`column_operand_magnitude`. That is `+12 %` for having any stage-1 term,
`+27 %` for correcting it, and `+42 %` for both together. The absolute seconds
differ from the paragraph above because they are a different session of the
same shape on the same machine; only ratios within one session are
comparable.

Within the first v3 baseline, the cost was dominated by taking the length of
each skinned position, 390 million of them at the ceiling. Widening that
length unconditionally to `f64` cost `+83 %` instead; making the `f64` square
a fallback behind a finite `f32` one recovers about a quarter of the
regression and none of the correctness, since the fallback is still what
admits an extent past `sqrt(f32::MAX)`. The remaining `+64 %` is the `f32`
square root itself, which is work this obligation did not previously do at
all.

The slot-dominated shape did not regress in that v3 session: `7.10s` before
the term and `6.70s` after. Folding the parent chain into the magnitude added
nothing measurable on
either shape — one `Mat4 * Vec4` per bone per document side per sample time,
accumulated in the walk that already composes the world matrices, against work
that is per *slot* and per *vertex*.

Those timings are retained as history, not carried forward as v4 evidence. v4
removes the per-vertex L2/square-root stage and widens its weighted numerator
and denominator to binary64, so the old cost attribution and which shape is
slowest may have changed. The budget is nevertheless unchanged: it bounds the
same proof passes and cardinalities, still admits the production-sized example
in Appendix D, and remains a conservative resource limit. This design makes no v4
wall-time or shape-ordering claim without a v4 benchmark.
