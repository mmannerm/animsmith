# Bevy 0.19.0 readback probe

This owner-run tool is deliberately outside the AnimSmith workspace and CI.
The official [Bevy 0.19.0 Cargo manifest](https://docs.rs/crate/bevy/0.19.0/source/Cargo.toml.orig)
declares Rust `1.95.0`; AnimSmith's published crates keep their Rust `1.88`
MSRV. Keeping this executable isolated prevents package,
docs.rs, all-feature, and ordinary CI builds from silently pulling Bevy.

It uses Bevy's stock `AssetServer` and `GltfPlugin` without a window, GPU, or
renderer. It is a narrow conformance probe for the exact `bevy` / revision-3 /
`0.19.0` / `gltf-asset-loader` tuple, not an asset-readiness or gameplay
certification.

## Exact tuple

The facade dependency is exactly `bevy = 0.19.0`; the committed `Cargo.lock`
is the authority for its resolved graph, including internal Bevy `0.19.1`
crates where Cargo resolves them. The harness records the lock identity and
the engine-neutral reader pins it with a drift test. The tool uses Bevy's
`gltf_animation` feature: `bevy_animation` alone does not enable the optional
animation fields on `bevy_gltf::Gltf`.

Run it only with an owner-authorized asset root. The tool emits JSON to stdout;
redirect it to a local file rather than committing it. Do not pass commercial
or private assets through CI, issue text, logs, or public reports.

```console
cargo +1.95.0 run --manifest-path tools/bevy-readback/Cargo.toml -- \
  --asset-root /owner-authorized/assets --asset character.glb \
  --prediction character.addressability-v2.json > character.bevy-readback.json
```

The asset argument must be a relative `.gltf` or `.glb` path under
`--asset-root`. The prediction must be AnimSmith's strict rich addressability
V2 output for the same immutable primary input and dependency closure.

Before Bevy starts, the probe canonicalizes the owner-provided root, hashes the
primary bytes it will ask `AssetServer` to load, and requires that identity to
equal both the V2 root input and its V4 provenance primary input. For a
`.gltf`, it also re-hashes every complete-closure external-resource key below
that root. A stale or cross-asset prediction exits `1` before engine
observation.

The JSON root is `urn:animsmith:schema:bevy-readback:1`, is self-identifying,
and contains no local paths or formatted loader errors. Exit `0` means every
applicable available prediction fact agreed; exit `1` means a mismatch,
required-unavailable prediction, or load/work-limit failure; exit `2` is an
operator, setup, or strict-input-read error.

Warnings retain only bounded tracing target and level metadata. When the
capture has omitted one or more later events, the canonical observation sets
`warnings_truncated: true`; it never treats the retained prefix as complete.

The first compilation is intentionally not budgeted for ordinary CI. Measure
the exact toolchain, target, compile time, and runtime before proposing an
owner-approved opt-in workflow lane.
