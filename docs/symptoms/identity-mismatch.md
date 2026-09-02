# Files disagree about skeleton or clip identity

Two deliveries that were supposed to be the same character import as different
skeletons, or a report says `Take 001` and nobody can tell which of the
thirty files in the pack it came from.

<img src="../visuals/icons/identity-mismatch.svg" alt="Two different files each carrying a clip labelled with the same name" width="160" align="right">

Commands: [`animsmith inspect`](../cli.md#commands) ·
[`animsmith collection lint`](../cli.md#commands)

## Why it happens

Marketplace packs commonly ship one clip per file and reuse an embedded take
name such as `Take 001` across every one of them, so a clip's display name is
not an identifier. Skeletons drift the same way: two exports of "the same" rig
can differ in hierarchy or rest signature and stop being exact-skeleton
interchangeable. Both are properties of a *set* of files, and AnimSmith's
ordinary checks resolve clips inside one loaded document — so this symptom is
answered by retaining identity, not by a check firing.

## What AnimSmith measures

There is no chart here, because the evidence is identity rather than motion.
`inspect` reports the exact skeleton and the embedded clip names of one
physical file, and a collection manifest binds each `(file, clip)` pair to a
logical id that survives the display name.

The repository's own collection-boundary fixtures are shaped like the
marketplace case: `walk-a.gltf` and `walk-b.gltf` are distinct physical files
with identical bytes and the same embedded name.

```console
$ animsmith inspect crates/animsmith/testdata/collection-spike/source/walk-a.gltf
crates/animsmith/testdata/collection-spike/source/walk-a.gltf
rig profile: none detected
skeleton: 1 bones
  root
materials: 0
mesh instances: 0
clips: 1
  Take 001: 0.000s, 0 tracks, 0 keys max   # exits 0

$ animsmith inspect crates/animsmith/testdata/collection-spike/source/walk-b.gltf
crates/animsmith/testdata/collection-spike/source/walk-b.gltf
rig profile: none detected
skeleton: 1 bones
  root
materials: 0
mesh instances: 0
clips: 1
  Take 001: 0.000s, 0 tracks, 0 keys max   # exits 0
```

Two files, one name. A third file in the same set carries `Take 001` *and*
`Take 002`, so even the take index is only meaningful together with its file:

```console
$ animsmith inspect crates/animsmith/testdata/collection-spike/source/multi.gltf
crates/animsmith/testdata/collection-spike/source/multi.gltf
rig profile: none detected
skeleton: 1 bones
  root
materials: 0
mesh instances: 0
clips: 2
  Take 001: 0.000s, 0 tracks, 0 keys max
  Take 002: 0.000s, 0 tracks, 0 keys max   # exits 0
```

## What the evidence looks like

A collection manifest is what turns those rows into stable identities: each
source keeps its safe locator and expected digest, and each logical clip names
the source plus the exact take index and embedded name it came from.

```toml
[[sources]]
key = "walk-a"
path = "walk-a.gltf"
config = "fixture.animsmith.toml"
expected_sha256 = "277f55812602cc560dbb432dede43bb145b3caa6cb90493675442a8f5499f044"

[[clips]]
id = "com.example.collection-spike/locomotion/walk-a"
source = "walk-a"
take_index = 0
take_name = "Take 001"
```

`animsmith collection lint` runs every declared source and emits that retained
binding as machine-readable evidence — the source digest, the config basis,
the observed takes, and the runtime sets that cross file boundaries. The
[fixture set](../../crates/animsmith/testdata/collection-spike/README.md) above
is the worked example, and
[collection contracts](../collection-contracts.md) is the reference.

## What to do

1. **Record the physical file, not the display name.** A runtime-set member
   must preserve the exact source path plus the embedded clip name and index.
   Normalized display names are not reproducible identifiers.
2. **Reconcile the pack's own list separately.** State it as a separate fact
   when a bundled animation list uses different spelling, casing, or ranges
   from the files; do not silently rename to make them match.
3. **Do not force a skeleton reference.** A copied-avatar or
   skeleton-reference hierarchy mismatch means the referenced mapping is not
   evidence for that file. Use a compatible individual asset, or obtain an
   authoritative re-export.
4. **Keep retargeting a separate decision.** An engine humanoid or retarget
   profile may still make two hierarchies compatible, but only after every
   required chain maps and the target character's deformation, transitions,
   masks, sockets and root ownership pass in the intended runtime.

Who fixes it: the pack owner and the pipeline that ingests it. AnimSmith
retains identity and reports what each file actually contains; it does not
merge, rename, or infer set membership, and it does not retarget. The gate
closes when the recorded source-to-target mapping is complete and the target
character plays the intended clips with visual evidence.

## Config

Identity lives in the collection manifest rather than in `animsmith.toml`.
Every source keeps its own document-local config, so declaring a set never
changes what a single file means:

```toml
schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.collection-spike"
input_root = "source"

[[runtime_sets]]
id = "com.example.collection-spike/sets/cross-file-gait"
kind = "gait-group"
members = [
  "com.example.collection-spike/locomotion/walk-a",
  "com.example.collection-spike/locomotion/walk-b",
]
```

<details>
<summary>Precise contract: skeleton identity, file-scoped clip identity, and what stays per file</summary>

Two collection-level contracts are easy to confuse with single-clip rig
health:

- **Skeleton/retarget identity.** Different bone hierarchies or rest/bind
  signatures are not exact-skeleton interchangeable. An engine humanoid or
  retarget profile may still make them compatible, but only after every
  required chain maps and target-character deformation, transitions, masks,
  sockets, and root ownership pass in the intended runtime. A copied-avatar or
  skeleton-reference hierarchy mismatch means the referenced mapping is not
  evidence for that file; use a compatible individual asset or obtain an
  authoritative re-export instead of forcing the reference.
- **File-scoped clip identity.** Marketplace packs commonly put one clip in
  each file while reusing an embedded name such as `Take 001`. A runtime-set
  member must then preserve the exact source file/path plus embedded clip name;
  normalized display names are not reproducible identifiers. Reconcile report
  members against the retained manifest and state separately when a bundled
  animation list uses different spelling, casing, or ranges.

AnimSmith's gait and sync groups still resolve clips inside one loaded
document. The file-scoped collection identity and cross-file set contract are
recorded in [DESIGN.md Appendix F](../../DESIGN.md#appendix-f--decision-record-file-scoped-clip-identity-and-collections)
and emitted by the explicit collection command. The
[contact-fragment](../collection-contracts.md#contact-fragments-147) and
[transition-family](../collection-contracts.md#transition-families-148) additions
are interchange declarations. The strict contact-fragment producer now emits
one manifest-witnessed sidecar after reloading that source; it does not make
ordinary checks cross-file or infer membership, and it adds no transform or
runtime policy. Keep per-file evidence
and the collection manifest authoritative; do not merge, rename, or infer set
membership merely to make a check run.

</details>
