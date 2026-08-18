# Development

This file is the canonical local setup and verification guide for a
source checkout. Contributor process lives in [CONTRIBUTING.md](CONTRIBUTING.md);
release operations live in [RELEASING.md](RELEASING.md); architecture and
crate boundaries live in [DESIGN.md](DESIGN.md).

## Toolchain

animsmith uses the Rust 2024 edition and MSRV `1.88`, declared in the
workspace `Cargo.toml`. CI checks stable Rust on Linux, macOS, and
Windows, plus the MSRV on Linux.

Develop on current stable; the MSRV is the floor the published crates
promise, not the compiler maintainers run. When that floor may move is
governed by the MSRV policy in [CONTRIBUTING.md](CONTRIBUTING.md).

`workspace.package.rust-version` is the single source of truth for the
number. The CI MSRV job reads it out of the manifest rather than pinning
a second copy, and two gates hold the prose to it: this file's MSRV
sentence via `scripts/check-github-community-files.sh`, and every crate
README and rustdoc header via `crates/animsmith/tests/msrv_docs.rs`. A
bump is therefore a one-line manifest edit plus whatever prose those
gates flag.

That same shell gate also rejects a version-shaped
`dtolnay/rust-toolchain@1.88` ref in any workflow, because Dependabot
rewrites those as if they were action tags.

Install the local tools used by the gates:

```console
$ just install-rust-tools
```

That installs `sccache`, `cargo-deny`, `typos-cli`, and `cargo-llvm-cov`
if they are missing. Cargo still works with stock defaults.

The animation-pack skill gate also uses Python 3 and PyYAML 6.x to validate
skill metadata semantically:

```console
$ python3 -m pip install "PyYAML>=6,<7"
```

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
$ just animation-pack-skill
$ just gates
```

`just gates` is the local PR gate and should be green on each candidate head
before pushing a non-trivial PR. It runs formatting, clippy, workspace tests,
golden skip marker verification, dependency checks, schema-id verification,
GitHub community-file checks, spell checking, rustdoc with missing public docs
denied, no-default-features CLI tests and builds, release binary smoke checks,
package readiness checks, and the animation-pack skill's behavioral and
published-report validation.

This author-side pre-push gate is distinct from the later PR audit. Once the
same commit is pushed, audit agents reuse the captured local result and the PR's
required checks; they do not rerun `just gates` independently.

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
$ cargo test -p animsmith --test scale_cli --no-default-features
$ cargo build -p animsmith --no-default-features
$ cargo build -p animsmith --release --no-default-features
```

In that build, glTF inspect, measure, lint, transform, fix, scale, and diff
stay available. HTML reports require the `report` feature; FBX input,
`convert`, and `assemble` require the `fbx` feature.

`scale` is the minimal build's evidence-emitting producer, so the atomic
artifact/evidence publication helpers live in `crates/animsmith/src/publish.rs`
rather than inside the feature-gated `assembly` module, and `scale_cli` is part
of the no-default-features gate.

## Golden Tests

Golden tests include an env-gated reference test against licensed assets
and CI-visible FBX mesh/skin/clip coverage from self-authored checked-in
fixtures. The reference test skips cleanly when `ANIMSMITH_GOLDEN_GLB`
is unset and prints the grep-able marker `ANIMSMITH_GOLDEN_SKIP`; CI and
`just gates` assert that marker is present.

```console
$ ANIMSMITH_GOLDEN_GLB=/path/to/reference-character.glb just golden
```

`ANIMSMITH_GOLDEN_GLB` is a developer-local, one-time reference input. Never
commit it or expose it through CI downloads, secrets, caches, logs, or
artifacts.

The committed-fixture rule applies repository-wide, not only under
`testdata/`: use synthetic/self-authored assets, CC0 assets, or other assets
whose license explicitly permits repository inclusion and CI
use/redistribution. Record provenance and license evidence for every
non-synthetic fixture. Commercial and other redistribution-restricted assets,
including excerpts and motion-bearing derivatives, stay outside the repository
and CI.

## Documentation Builds

Use `just doc` for rustdoc warnings-as-errors and missing-docs
enforcement. It renders workspace docs and the CLI crate without default
features with `-D warnings -D missing_docs`, so all five publishable
crates keep documented public surfaces.

When editing public docs, `cargo test --workspace` validates Markdown
link targets and `#anchor`s in the gated doc set (the `docs_links`
test); review GitHub forms and rendering by inspection. The root
`README.md` is also the crates.io front page for the `animsmith` CLI
crate, so keep its links absolute and keep CLI-user content first.

The `release_version_docs` workspace test also keeps the current dependency
snippets in the published READMEs and embedding guide aligned with the current
`tool.version` examples. It accepts the manifest version or exactly its next
patch or minor during pre-dispatch documentation staging; on a generated
`release-plz-*` branch, every current version claim must exactly match the
bumped workspace manifest. Historical changelog, bootstrap, and roadmap
versions are deliberately outside that inventory.

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

The package inventory gate protects the publishable crate contents
intended for crates.io:

```console
$ just package-inventory
```

It verifies each publishable manifest's docs.rs metadata and README
choice, checks that package inventories include the README, manifest,
and source entry point, and fully verifies `animsmith-core` with
`cargo package`. The shared include list carries workspace license and
notice files. The dependent crates cannot run full `cargo package`
verification until the matching `animsmith-*` dependency versions have
been published to crates.io, so the local gate uses
`cargo package --list` for them and the release flow publishes in
dependency order.

When the matching internal dependency versions already exist in the
registry, also dry-run package assembly for any affected dependent crate:

```console
$ cargo package -p animsmith-gltf
```

The release binary workflow packages CLI archives and detects the CLI
release tag through shared scripts (`scripts/package-release-binary.py`,
`scripts/select-cli-release-tag.sh`) so both are exercised locally rather
than only in CI:

```console
$ just release-packaging
```
