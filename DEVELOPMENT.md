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

The animation-pack skill gate also uses Python 3, PyYAML 6.x, and JSON Schema
4.x to validate skill metadata and the closed evaluation-model schema:

```console
$ python3 -m pip install "PyYAML>=6,<7" "jsonschema>=4.18,<5"
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
denied, no-default-features CLI tests and builds, the retained release CLI
proof, package readiness checks, and the animation-pack skill's behavioral and
published-report validation.

This author-side pre-push gate is distinct from the later PR audit. Once the
same commit is pushed, audit agents reuse the captured local result and the PR's
required checks; they do not rerun `just gates` independently.

The corresponding CI workflows also validate the same expectations on a
clean checkout. Coverage and the security scanners (Scorecard, CodeQL)
run only in CI and are informational, so they are not part of the local
gate (see below).

Pull requests also receive one separate `checks / animation-pack` result from
the reusable checks workflow. It runs the same animation-pack validator suite
once on the exact pull-request head, equivalent to the local
`just animation-pack-skill` recipe, covering the versioned skill validators and
every maintained report/appendix pair. Audit agents should retain and reuse
that exact-head result instead of rerunning the suite independently. The
check uses only repository-safe fixtures; licensed animation packs remain
outside the repository and CI.

## no-default-features

The default CLI build includes FBX support through `ufbx` and the HTML
report feature. The `--no-default-features` build must keep working as a
pure-Rust glTF-only binary:

```console
$ export CARGO_TARGET_DIR=target/no-default-features
$ cargo test -p animsmith --test cli_contract --no-default-features
$ cargo test -p animsmith --test foot_cycle_cli --no-default-features
$ cargo test -p animsmith --test scale_cli --no-default-features
$ cargo test -p animsmith --test transition_pose_cli --no-default-features
$ cargo build -p animsmith --no-default-features
$ cargo build -p animsmith --release --no-default-features
```

The redirected target directory is not optional bookkeeping. Without it these
builds land on `target/debug/animsmith`, `target/release/animsmith`, and
`target/doc`, and whichever feature set ran last is what stays there; see
Evaluation Artifacts below.

In that build, glTF inspect, measure, lint, transform, fix, scale, diff,
`generate addressability`, and `collection transform-foot-cycle` stay
available, as do `evaluate-transition-poses` and
`collection evaluate-transition-poses`. HTML reports require the `report`
feature; FBX input, `convert`, and `assemble` require the `fbx` feature.

`scale` and `collection transform-foot-cycle` are the minimal build's
evidence-emitting producers, so their shared atomic artifact/evidence and
generation publication helpers live in `crates/animsmith/src/publish.rs`
rather than inside the feature-gated `assembly` module. Both `scale_cli` and
`foot_cycle_cli` are part of the no-default-features gate.

## Evaluation Artifacts

A checkout holds more than one CLI, and they do not have the same
capabilities. Feature-variant builds write to `target/no-default-features/`,
so the conventional paths hold default-feature artifacts only:

| Path | Compiled features | Use |
|---|---|---|
| `target/release/animsmith` | `fbx`, `report` | external evaluation, after a green `just gates` |
| `target/debug/animsmith` | `fbx`, `report` | development and the local skill gates |
| `target/no-default-features/{debug,release}/animsmith` | none | proving the minimal build still works |
| a published release archive | `fbx`, `report` | external evaluation, verified per [RELEASING.md](RELEASING.md) |

`just release-cli` is what proves the first row rather than assuming it. It
builds both release variants, requires the retained
`target/release/animsmith` to admit the self-authored FBX fixture and expose
`report` while the isolated minimal binary refuses both, and prints the
provenance record to keep beside an evaluation's results:

```console
$ just release-cli
release-cli: provenance of the retained default-feature CLI
  binary:       target/release/animsmith
  sha256:       <sha256 of that file>
  version:      <the --version line>
  capabilities: <what the probes proved, not what the binary claims>
  commit:       <git rev-parse HEAD>
  describe:     <git describe --tags --dirty --always>
```

`just gates` runs that recipe, and CI runs the same script after its release
builds. The script also refuses a justfile or reusable checks workflow whose
`--no-default-features` commands would write to the shared target directory,
so the isolation cannot be dropped without the gate saying so.

## Cross-platform determinism

Committed example assets and documentation visuals are byte-compared, and
CI runs that comparison on Linux, macOS, and Windows, so every number that
reaches those bytes has to be identical on x86-64 and arm64.

Model-space sampling is where that bites. `glam` is pinned to its
`scalar-math` and `libm` features in the workspace `Cargo.toml`, so the FK
that builds a pose grid uses scalar Rust `f32` arithmetic and the `libm`
crate's transcendentals rather than SIMD intrinsics and the platform's own
`libm`. Rust never contracts `a * b + c` into a fused multiply-add, so
scalar IEEE-754 arithmetic gives bit-identical results everywhere; glam's
NEON path on arm64 does fuse, and its SSE2 path on x86-64 does not, which
moves the last bits of every matrix product. A committed report embeds the
sampled grid as base64, so those last bits are the file.

Fixtures whose motion is translation-only never saw this — adding exact
values is bit-identical on any IEEE platform. The first committed report
whose model positions come from a rotation (`clip-dirty.report.html`) is
what surfaced it, as a macOS-only failure of
`committed_visuals_match_the_generator_output`.

Prefer fixing determinism at the source over loosening a byte comparison:
the comparison is what makes a committed picture evidence.

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

The same boundary governs what tooling may publish about a licensed clip.
Published evidence is limited to summary numbers, non-recoverable digests,
scrubbed labels, and single-series metric charts — one or two scalar series
per clip, such as foot height or root speed. Per-bone pose data is a
motion-bearing derivative and is never published, and that includes the HTML
report's sampled pose grid: the grid is the model-space position of every
bone on every judged frame, so a full report of a licensed clip carries the
clip. The artifact form that satisfies this rule is
`animsmith report --evidence-only`, which is what may be attached to an
issue, published, or sent to a vendor.

## Documentation Builds

Use `just doc` for rustdoc warnings-as-errors and missing-docs
enforcement. It renders workspace docs and the CLI crate without default
features with `-D warnings -D missing_docs`, so all six publishable crates
keep documented public surfaces.

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

### GitHub Pages preview

The Pages site uses the mdBook version pinned in `.mdbook-version`. Install it
locally before using the Pages commands:

```console
$ cargo install mdbook --version "$(tr -d '[:space:]' < .mdbook-version)" --locked
```

`just docs-check` stages tracked repository files outside the checkout (in a
deterministic sibling directory named after the checkout by default), generates
the book, builds it, parser-validates the staged Markdown destinations, and
rejects rendered local links without targets in the built artifact.
`just docs-serve` stages the same source and serves it locally. Set
`ANIMSMITH_DOCS_STAGE` to choose another external staging directory; it must not
overlap the checkout and must not be committed.

Navigation comes from the Category column of `docs/README.md`. A cell is either
`Part` or `Part › Group`: parts become mdBook part titles in first-appearance
order, and a group becomes a generated chapter at `_generated/groups/<slug>.md`
listing its member rows with their `Use it to…` text. Staging fails when a part
or a group is split by another one, when the same group name appears under two
parts, when two group names collapse to one slug, or when a group name has no
slug characters, so the sidebar always follows the table's order. The row
pointing at `docs/reports/README.md` nests the report and evidence pairs from
that table below itself. Chapters with children start collapsed
(`[output.html.fold]` at level 0).

Staging also resolves the media a page embeds: an `<img>` naming a tracked
drawing under `docs/visuals` becomes that drawing's own markup, so it reads the
page's theme tokens instead of guessing light or dark from the operating
system, and an `<iframe src>` becomes a site-absolute path, which mdBook never
rewrites and which the aggregated print page therefore needs. The repository
Markdown keeps the plain references GitHub renders.

Tracked `docs/site` files stage as mdBook's `theme/` override directory instead
of as book source, so the stylesheet, fonts, and favicons style the site without
becoming pages. `docs/site/animsmith.css` is required and wired as
`additional-css`; staging refuses a checkout that does not track it rather than
publishing an unstyled book. `docs/site/redirects.toml` is configuration rather
than a page: each `"/route.html" = "relative/target.html"` entry becomes an
`[output.html.redirect]` route, and the build fails when a redirect target is
missing from the artifact. Built `README.md` chapters receive compatibility
routes because mdBook renders them as `index.html`. Generated links to
repository source are pinned to the selected release tag at the release root and
to `main` below `/dev/`.

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
