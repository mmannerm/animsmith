# Development

This file is the canonical local setup and verification guide for a
source checkout. Contributor process lives in [CONTRIBUTING.md](CONTRIBUTING.md);
release operations live in [RELEASING.md](RELEASING.md); architecture and
crate boundaries live in [DESIGN.md](DESIGN.md).

## Toolchain

animsmith uses the Rust 2024 edition and MSRV `1.88`, declared in the
workspace `Cargo.toml`. CI checks stable Rust on Linux, macOS, and
Windows, plus the MSRV on Linux.

Install the local tools used by the gates:

```console
$ just install-rust-tools
```

That installs `sccache` and `cargo-deny` if they are missing. Cargo still
works with stock defaults.

## sccache

Local `sccache` is optional. To configure a user-level Cargo
`rustc-wrapper` for faster repeated builds across worktrees:

```console
$ just configure-sccache
```

Use `RUSTC_WRAPPER=` on an individual command when you intentionally want
to bypass `sccache`.

CI uses GitHub Actions caching and `Swatinem/rust-cache`; it does not
depend on a private runner cache.

## Common Commands

```console
$ just build
$ just test
$ just doc
$ just gates
```

`just gates` is the local PR gate and should be green before pushing a
non-trivial PR. It runs formatting, clippy, workspace tests, golden skip
marker verification, dependency checks, schema-id verification, GitHub
community-file checks, rustdoc, no-default-features CLI tests and
builds, release binary smoke checks, and package inventory checks.

The corresponding CI workflows also validate the same expectations on a
clean checkout.

## no-default-features

The default CLI build includes FBX support through `ufbx` and the HTML
report feature. The `--no-default-features` build must keep working as a
pure-Rust glTF-only binary:

```console
$ cargo test -p animsmith --test cli_contract --no-default-features
$ cargo build -p animsmith --no-default-features
$ cargo build -p animsmith --release --no-default-features
```

In that build, glTF inspect, measure, lint, transform, fix, and diff stay
available. HTML reports require the `report` feature; FBX input and
`convert` require the `fbx` feature.

## Golden Tests

Golden tests include an env-gated reference test against licensed assets
and CI-visible FBX mesh/skin/clip coverage from self-authored checked-in
fixtures. The reference test skips cleanly when `ANIMSMITH_GOLDEN_GLB`
is unset and prints the grep-able marker `ANIMSMITH_GOLDEN_SKIP`; CI and
`just gates` assert that marker is present.

```console
$ ANIMSMITH_GOLDEN_GLB=/path/to/reference-character.glb just golden
```

Only CC0 or procedurally generated fixtures may be committed under
`testdata/`.

## Documentation Builds

Use `just doc` for rustdoc warnings-as-errors. It renders workspace docs
and the CLI crate without default features.

When editing public docs, also check Markdown links and GitHub forms by
inspection. The root `README.md` is also the crates.io front page for the
`animsmith` CLI crate, so keep its links absolute and keep CLI-user
content first.

## Package Readiness

The package inventory gate protects publishable crate contents:

```console
$ just package-inventory
```

Before release-oriented changes, also dry-run package assembly for the
affected crate. The core crate can be checked independently:

```console
$ cargo package -p animsmith-core
```

Dependent crates can only fully verify against crates.io once their
internal `animsmith-*` dependencies have been published.
