# Collection-boundary spike fixtures

These files are self-authored, motion-free glTF fixtures released under the
repository's MIT OR Apache-2.0 license. They contain only a root node and empty
named animation declarations; no marketplace or other third-party asset bytes
are present.

`collection.toml` exercises the accepted V1 identity boundary:

- `walk-a.gltf` and `walk-b.gltf` are distinct physical sources with the same
  bytes and the repeated embedded name `Take 001`;
- `multi.gltf` carries `Take 001` at index 0 and `Take 002` at index 1;
- the gait and sync sets both cross physical file boundaries; and
- every source selects the same explicit config basis without changing its
  document-local selector meaning.

The proposal's one-batch preservation witness is:

| source key | safe locator | bytes | SHA-256 | config SHA-256 | logical clip | take witness |
|---|---|---:|---|---|---|---|
| `multi` | `multi.gltf` below `source/` | 372 | `959226925e607d368ea226ed68780cde249f75e73cfa90866db4afe9b2da2fe7` | `385b7a67171994d8099fb7d4623721fc7b84fcdbe8cba1b7883f72fbba75182e` | `com.example.collection-spike/multi/first` | index 0, `Take 001` |
| `multi` | `multi.gltf` below `source/` | 372 | `959226925e607d368ea226ed68780cde249f75e73cfa90866db4afe9b2da2fe7` | `385b7a67171994d8099fb7d4623721fc7b84fcdbe8cba1b7883f72fbba75182e` | `com.example.collection-spike/multi/second` | index 1, `Take 002` |
| `walk-a` | `walk-a.gltf` below `source/` | 274 | `277f55812602cc560dbb432dede43bb145b3caa6cb90493675442a8f5499f044` | `385b7a67171994d8099fb7d4623721fc7b84fcdbe8cba1b7883f72fbba75182e` | `com.example.collection-spike/locomotion/walk-a` | index 0, `Take 001` |
| `walk-b` | `walk-b.gltf` below `source/` | 274 | `277f55812602cc560dbb432dede43bb145b3caa6cb90493675442a8f5499f044` | `385b7a67171994d8099fb7d4623721fc7b84fcdbe8cba1b7883f72fbba75182e` | `com.example.collection-spike/locomotion/walk-b` | index 0, `Take 001` |

The decoded source and clip arrays sort by their stable keys/ids; the two set
records sort by id while retaining their declared member order. The equal walk
digests do not collapse their locators or logical ids. A future
`collection-output:1` result must preserve all four rows, the selected config
basis/digest, both runtime sets, and the exact manifest digest.

The three `invalid-*.toml` files pin distinct control-plane failures: a repeated
set member, a dangling member, and a source locator that escapes the manifest
root. They must fail before source execution with exit 2 and no collection
envelope. #545 turns these retained spike fixtures into parser/resolver tests;
#546 adds the valid one-batch output assertion.
