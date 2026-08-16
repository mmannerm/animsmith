# Releasing

Releases are automated with [release-plz](https://release-plz.dev). You
never hand-edit versions: when a maintainer manually dispatches the release
workflow, release-plz opens or updates a **release PR** that bumps the shared
workspace version, propagates the internal `animsmith-*` dependency versions,
and updates `CHANGELOG.md`. Merging that PR tags the release, publishes the
GitHub Release, and publishes the workspace crates intended for crates.io in
dependency order.

The workflow is `.github/workflows/release-plz.yml`; its behaviour is
configured by `release-plz.toml`. The changelog uses release-plz's
default Keep-a-Changelog format, derived from the Conventional Commit
history (accepted types live in `.commitlintrc.yml`).

## Repository prerequisite

In GitHub, enable **Settings → Actions → General → Workflow
permissions → Allow GitHub Actions to create and approve pull requests**.
The `release-pr` job's `pull-requests: write` permission is necessary but does
not override that repository setting. If the setting is disabled,
release-plz can push its release branch but PR creation fails with HTTP 403,
leaving the branch without a pull request.

## Per-release flow (steady state)

During release freeze, default new work to the next milestone. Promote
only reproducible correctness, safety, public-contract, or documentation
blockers, plus genuinely small mutation-strengthening tests; record why
shipping is riskier than changing.

1. Merge feature/fix PRs to `main` as usual (Conventional Commits).
2. Perform one release-wide documentation-freshness sweep over the root
   and crate READMEs, `docs/`, `examples/`, and current version/status
   claims. Preserve clearly historical references.
3. When `main` is ready to release, manually dispatch the release workflow:

   ```console
   gh workflow run release-plz.yml --ref main
   ```

   The workflow first runs the shared checks, then release-plz opens or
   updates the release PR. It computes the next version from the commits
   since the last release — one shared version across the publishable crates
   (`version_group`), so the whole workspace moves together — and writes the
   changelog. Ordinary pushes to `main` never create or update this PR.
   If another change merges to `main` while the release PR is open, dispatch
   the workflow again before merging so the version and changelog include it.
4. Review that PR. The glTF writer records the package version in
   `asset.generator`, so regenerate and commit the version-stamped example
   assets from the release PR branch before merging:

   ```console
   cargo run -p animsmith --example gen_example_assets
   cargo test -p animsmith --test examples_cookbook
   ```

   The cookbook drift guard fails if the committed bytes do not match the
   release version. When you merge the PR, the `release` job runs on the
   resulting push to `main`, tags the release, creates the GitHub Release, and
   publishes every crate to crates.io in dependency order (`animsmith-core` →
   `-gltf`/`-fbx`/`-report` → `animsmith`).
   The follow-on `release_binaries` job calls `release-binaries.yml`,
   builds CLI archives from the tag, and uploads the archives plus
   matching `.sha256` files to that GitHub Release.

Supported CLI archive targets live in `release-targets.json`.
`scripts/release-targets.py` renders the generated workflow matrix block in
`.github/workflows/release-binaries.yml` and the install table in
`docs/cli.md`. After changing release targets, run
`scripts/release-targets.py write` and then `just release-packaging` so the
generated docs and workflow matrix stay in sync.

crates.io publishing uses
[Trusted Publishing](https://crates.io/docs/trusted-publishing) (GitHub
OIDC): the `release` job holds `id-token: write` and release-plz mints a
short-lived token itself — there is no long-lived `CARGO_REGISTRY_TOKEN`.

The publish step is idempotent: a re-run skips versions already on the
registry. If publishing fails partway through, use **Re-run failed jobs** on
that original push-triggered workflow run. A new manual dispatch creates or
updates the next release PR; it does not retry publishing.

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

## Schema identities

Output and measurement schemas use immutable protocol URNs, independent of
package releases. Do not rewrite `$id` during a release. A breaking contract
gets a new URN and schema file. `scripts/check-schema-id.sh` checks that each
schema, the CLI, and `docs/output.md` reference the same identities; repository
links are only retrieval locations.

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

7. Arm the release automation. Both the manually dispatched `release-pr` job
   and the push-triggered `release` job are gated on `vars.RELEASE_PLZ_ARMED`,
   so the whole flow stays inert until this is set — no release PRs and no
   publish attempts before the manual `0.1.0` above:

   ```console
   gh variable set RELEASE_PLZ_ARMED --body true
   ```

After the bootstrap, every subsequent release (`0.2.0`+) starts with the
manual workflow dispatch and then goes through the release-plz PR flow above —
no manual `cargo publish`, no manual version edits, one repo-level `vX.Y.Z`
tag and Release per version.

## Known caveat: CI on the release PR

PRs opened with the default `GITHUB_TOKEN` do **not** trigger
`on: pull_request` workflows, so the release-plz PR will not get its own
CI run. The manual dispatch does run the full shared checks against `main`
before opening or updating the PR, and the post-merge `checks` job runs them
again before publishing. The dispatch-time run checks the pre-bump `main`
tree, not release-plz's generated version, lockfile, or changelog changes, so
review those files in the release PR. If branch protection also requires a
check attached to the release PR itself, give the `release-pr` job a PAT or
GitHub App token via the release-plz `token` input instead of
`secrets.GITHUB_TOKEN`.
