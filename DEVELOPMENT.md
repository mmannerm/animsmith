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

That installs `sccache`, `cargo-deny`, `typos-cli`, and `cargo-llvm-cov`
if they are missing. Cargo still works with stock defaults.

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
community-file checks, spell checking, rustdoc, no-default-features CLI
tests and builds, release binary smoke checks, and package inventory
checks.

The corresponding CI workflows also validate the same expectations on a
clean checkout. Coverage and the security scanners (Scorecard, CodeQL)
run only in CI and are informational, so they are not part of the local
gate (see below).

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

## Spell Checking

`just gates` runs [`typos`](https://github.com/crate-ci/typos) over source,
comments, and docs. Run it alone with:

```console
$ just typos
```

Domain jargon that reads as a misspelling (and binary DCC fixtures that
embed vendor strings) is allow-listed in [`_typos.toml`](_typos.toml). Add
new project terms there rather than rewording correct code.

## Coverage

Line and region coverage come from
[`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) (source-based
LLVM instrumentation). Generate a local HTML report under
`target/llvm-cov/html`:

```console
$ just coverage
```

CI runs the same tool and uploads the lcov report to Codecov, which renders
the README badge and per-PR diff coverage. Coverage is informational and
never blocks a merge; the `codecov.yml` project and patch statuses are set
to `informational`. Enabling the repository on Codecov and adding the
`CODECOV_TOKEN` secret is a one-time maintainer step (see
[`.github/workflows/coverage.yml`](.github/workflows/coverage.yml)).

## Security And Supply-Chain Scans

Beyond `cargo audit` and `cargo deny` (the `audit` workflow), CI runs three
informational security scans that report to the GitHub Security tab and are
not wired into branch protection:

- **OpenSSF Scorecard** grades repository security posture weekly and backs
  the README badge.
- **CodeQL** performs static analysis of the Rust sources on `main` pushes
  and weekly, keeping results in the Security tab without adding another
  full-workspace compile to every PR.
- **Dependabot** ([`.github/dependabot.yml`](.github/dependabot.yml)) opens
  weekly PRs to bump Cargo dependencies and pinned Action versions. Minor
  and patch Cargo bumps and Action updates are grouped into one PR each;
  major Cargo bumps open individually so each breaking upgrade is reviewed
  on its own.

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
