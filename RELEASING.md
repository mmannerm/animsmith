# Releasing

Releases are automated with [release-plz](https://release-plz.dev). You
never hand-edit versions: release-plz opens a **release PR** that bumps
the shared workspace version, propagates the internal `animsmith-*`
dependency versions, and updates `CHANGELOG.md`. Merging that PR tags the
release, publishes the GitHub Release, and publishes all five crates to
crates.io in dependency order.

The workflow is `.github/workflows/release-plz.yml`; its behaviour is
configured by `release-plz.toml` (which reuses the existing `cliff.toml`
for changelog styling).

## Per-release flow (steady state)

1. Merge feature/fix PRs to `main` as usual (Conventional Commits).
2. release-plz keeps a `release-plz` PR open and up to date. It computes
   the next version from the commits since the last release — one shared
   version across all five crates (`version_group`), so the whole
   workspace moves together — and writes the changelog.
3. Review that PR. When you merge it, the `release` job tags, creates the
   GitHub Release, and publishes every crate to crates.io in dependency
   order (`animsmith-core` → `-gltf`/`-fbx`/`-report` → `animsmith`).

crates.io publishing uses
[Trusted Publishing](https://crates.io/docs/trusted-publishing) (GitHub
OIDC): the `release` job holds `id-token: write` and release-plz mints a
short-lived token itself — there is no long-lived `CARGO_REGISTRY_TOKEN`.

The publish step is idempotent: a re-run skips versions already on the
registry.

## Bumping schema `$id` on a release (if applicable)

The machine-readable output schema's `$id`
(`docs/schemas/output-v1.schema.json`) normally points at `/main/`, which
is version-independent and needs no per-release change. `scripts/check-schema-id.sh`
(run in CI) enforces that the CLI (`crates/animsmith/src/main.rs`) and
`docs/output.md` reference the current `$id`, and that a version-pinned
`$id` (`/vX.Y.Z/…`) matches the workspace version. If you ever pin the
schema URL to a release tag, update it in the same release PR.

## One-time bootstrap

This repository starts its public release history from a clean slate: the
pre-publication development tags (`v0.1.0`–`v0.7.0`, never on crates.io)
were deleted on 2026-07-04, so the first crates.io publish, the first
GitHub Release, and `CHANGELOG.md` all begin together at the `0.1.0` in
`Cargo.toml`.

**Trusted Publishing cannot publish a crate that does not yet exist**, so
the very first publish of each crate is manual; TP takes over afterwards.

1. `cargo login` with a token from <https://crates.io/settings/tokens>
   (scope: `publish-new` + `publish-update`).
2. Publish the workspace once, in dependency order (each dependent crate
   resolves against the crate just published):

   ```console
   for crate in animsmith-core animsmith-gltf animsmith-fbx animsmith-report animsmith; do
     cargo publish -p "$crate"
   done
   ```

   `animsmith-core` should pass `cargo publish --dry-run` first; the
   dependent crates can only fully verify once their `animsmith-*`
   dependencies exist in the index.
3. After each crate is accepted, docs.rs queues its documentation. Check
   each crate's docs.rs page; the manifests set `documentation` links and
   `[package.metadata.docs.rs]` so pure-Rust crates get Linux/macOS/Windows
   pages, while the C-dependent crates (`animsmith-fbx`, all-features CLI)
   use the Linux default target.
4. On crates.io, for **each** of the five crates: Settings → Trusted
   Publishing → add publisher — repository `mmannerm/animsmith`, workflow
   `release-plz.yml`, no environment.

After the bootstrap, every subsequent release goes through the release-plz
PR flow above — no manual `cargo publish`, no manual version edits.

## Known caveat: CI on the release PR

PRs opened with the default `GITHUB_TOKEN` do **not** trigger
`on: pull_request` workflows, so the release-plz PR will not get its own
CI run. If branch protection requires a passing CI check before you can
merge it, give the `release-pr` job a PAT or GitHub App token via the
release-plz `token` input instead of `secrets.GITHUB_TOKEN`. (The
post-merge `checks` job in `release-plz.yml` still runs the full test
matrix on `main` before publishing regardless.)
