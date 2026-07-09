# Releasing

Releases are automated with [release-plz](https://release-plz.dev). You
never hand-edit versions: release-plz opens a **release PR** that bumps
the shared workspace version, propagates the internal `animsmith-*`
dependency versions, and updates `CHANGELOG.md`. Merging that PR tags the
release, publishes the GitHub Release, and publishes the workspace crates
intended for crates.io in dependency order.

The workflow is `.github/workflows/release-plz.yml`; its behaviour is
configured by `release-plz.toml`. The changelog uses release-plz's
default Keep-a-Changelog format, derived from the Conventional Commit
history (accepted types live in `.commitlintrc.yml`).

## Per-release flow (steady state)

1. Merge feature/fix PRs to `main` as usual (Conventional Commits).
2. release-plz keeps a `release-plz` PR open and up to date. It computes
   the next version from the commits since the last release — one shared
   version across the publishable crates (`version_group`), so the whole
   workspace moves together — and writes the changelog.
3. Review that PR. When you merge it, the `release` job tags, creates the
   GitHub Release, and publishes every crate to crates.io in dependency
   order (`animsmith-core` → `-gltf`/`-fbx`/`-report` → `animsmith`).
   The follow-on `release_binaries` job calls `release-binaries.yml`,
   builds CLI archives from the tag, and uploads the archives plus
   matching `.sha256` files to that GitHub Release.

crates.io publishing uses
[Trusted Publishing](https://crates.io/docs/trusted-publishing) (GitHub
OIDC): the `release` job holds `id-token: write` and release-plz mints a
short-lived token itself — there is no long-lived `CARGO_REGISTRY_TOKEN`.

The publish step is idempotent: a re-run skips versions already on the
registry.

### Version-bump policy

Configured in `release-plz.toml`:

- **`feat`** bumps the minor, **`fix`/`perf`** the patch, even on `0.x`
  (`features_always_increment_minor = true`, porting the old
  `cliff.toml` bump rule).
- **Breaking changes** follow semver — on `0.x` that is a **minor** bump
  (`0.1.0` → `0.2.0`), not a major. This is the one place the old
  `cliff.toml` differed: it forced breaking → major (`→ 1.0.0`), but
  release-plz has no equivalent setting. If you want to go to `1.0.0`,
  bump the version explicitly in the release PR.
- Only `feat`/`fix`/`perf`/`revert` appear in the changelog and release
  notes; `chore`/`ci`/`docs`/`style`/`refactor`/`test`/`build` and merge
  commits are skipped (`[changelog].commit_parsers`) — they still count
  toward whether a release is warranted.

## Bumping schema `$id` on a release (if applicable)

The machine-readable output schema's `$id`
(`docs/schemas/output-v1.schema.json`) normally points at `/main/`, which
is version-independent and needs no per-release change. `scripts/check-schema-id.sh`
(run in CI) enforces that the CLI (`crates/animsmith/src/main.rs`) and
`docs/output.md` reference the current `$id`, and that a version-pinned
`$id` (`/vX.Y.Z/…`) matches the workspace version. If you ever pin the
schema URL to a release tag, update it in the same release PR.

## Published README and docs links

The crate READMEs are included in the crates published to crates.io.
During pre-1.0 development, those READMEs intentionally link deeper
repository docs to latest `main` with
`github.com/mmannerm/animsmith/blob/main/...` or `/tree/main/...` URLs.
That means an older published crate can send readers to newer source
docs. For now this is accepted so reference docs stay simple while the
API is still settling; the machine-readable JSON schema remains protected
separately by `scripts/check-schema-id.sh`.

If a future release needs version-pinned README links, update the
READMEs, this section, and `scripts/check-package-inventory.sh` in the
same release-oriented PR. Do not add release-time rewriting without a
mechanical check that proves the packaged README links and the release
tag agree.

## One-time bootstrap

This repository starts its public release history from a clean slate: the
pre-publication development tags (`v0.1.0`–`v0.7.0`, never on crates.io)
were deleted on 2026-07-04, so the first crates.io publish, the first
GitHub Release, and `CHANGELOG.md` all begin together at the `0.1.0` in
`Cargo.toml`.

**The entire first `0.1.0` release is manual** — crates.io publish, the
`v0.1.0` tag, the GitHub Release, and the initial `CHANGELOG.md`. Two
constraints force this, and they compose:

- Trusted Publishing cannot publish a crate that does not yet exist, so
  the first crates.io publish of each crate must use a token.
- release-plz `release` only acts on *unpublished* packages. Once `0.1.0`
  is on crates.io it will (correctly) no-op — so it will **not** create
  the first `v0.1.0` tag/Release for you. The manual `v0.1.0` tag is also
  the baseline release-plz needs to compute the next version.

So automation begins at `0.2.0`; `0.1.0` is done by hand, once:

1. `cargo login` with a token from <https://crates.io/settings/tokens>
   (scope: `publish-new` + `publish-update`).
2. **Generate and commit the changelog first — before publishing.**
   `release-plz update` compares the local crates against the registry to
   find unreleased changes, so it only produces the `0.1.0` changelog
   while the crates are still unpublished. Run it now (its Keep-a-Changelog
   format matches every later release):

   ```console
   release-plz update          # writes CHANGELOG.md
   git add CHANGELOG.md && git commit -m "chore(release): 0.1.0"
   git push
   ```
3. Publish the workspace from that release commit, in dependency order
   (each dependent crate resolves against the crate just published):

   ```console
   for crate in animsmith-core animsmith-gltf animsmith-fbx animsmith-report animsmith; do
     cargo publish -p "$crate"
   done
   ```

   `animsmith-core` should pass `cargo publish --dry-run` first; the
   dependent crates can only fully verify once their `animsmith-*`
   dependencies exist in the index.
4. After each crate is accepted, docs.rs queues its documentation. Check
   each crate's docs.rs page; the manifests set `documentation` links and
   `[package.metadata.docs.rs]` so pure-Rust crates get Linux/macOS/Windows
   pages, while the C-dependent crates (`animsmith-fbx`, all-features CLI)
   use the Linux default target.
5. On crates.io, for **each publishable crate**: Settings → Trusted
   Publishing → add publisher — repository `mmannerm/animsmith`, workflow
   `release-plz.yml`, no environment.
6. Tag the release commit and publish the GitHub Release from the `0.1.0`
   changelog section (release-plz won't create it — `0.1.0` is already
   published — so the notes are extracted from that same section):

   ```console
   git tag v0.1.0 && git push origin v0.1.0
   gh release create v0.1.0 --title v0.1.0 \
     --notes-file <(awk '/^## \[0\.1\.0\]/{f=1;next} /^## \[/{f=0} f' CHANGELOG.md)
   ```

   Then dispatch the binary packaging workflow against `main` so the
   manually created release gets the same archives and checksums as later
   automated releases:

   ```console
   gh workflow run release-binaries.yml --ref main -f tag=v0.1.0
   ```

7. Arm the release automation. Both the `release-pr` and `release` jobs
   are gated on `vars.RELEASE_PLZ_ARMED`, so the whole flow stays inert
   until this is set — no release PRs and no publish attempts before the
   manual `0.1.0` above:

   ```console
   gh variable set RELEASE_PLZ_ARMED --body true
   ```

After the bootstrap, every subsequent release (`0.2.0`+) goes through the
release-plz PR flow above — no manual `cargo publish`, no manual version
edits, one repo-level `vX.Y.Z` tag and Release per version.

## Known caveat: CI on the release PR

PRs opened with the default `GITHUB_TOKEN` do **not** trigger
`on: pull_request` workflows, so the release-plz PR will not get its own
CI run. If branch protection requires a passing CI check before you can
merge it, give the `release-pr` job a PAT or GitHub App token via the
release-plz `token` input instead of `secrets.GITHUB_TOKEN`. (The
post-merge `checks` job in `release-plz.yml` still runs the full test
matrix on `main` before publishing regardless.)
