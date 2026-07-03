# Releasing

Cargo's workspace version is the source of truth for crates.io. Do not
publish from a temporary `Cargo.toml`: crates.io archives the manifest
version, and that version cannot be overwritten after upload.

GitHub Releases are automated only when two things agree:

1. Conventional commits since the last release make `git-cliff` compute a
   new tag.
2. `[workspace.package] version` in `Cargo.toml` already matches that tag
   without the leading `v`.

If the commits warrant `v0.2.0` but `Cargo.toml` still says `0.1.0`, CI
prints a warning and creates no release. Land a
`chore(release): bump workspace version to 0.2.0` PR, then re-run or let
the next `main` run publish.

The CLI display version is separate from the package version. Published
crates and dependency resolution use `Cargo.toml`; the `animsmith`
binary embeds `ANIMSMITH_BUILD_VERSION` when that environment variable is
set. Otherwise, source-checkout builds use `git describe --tags --dirty
--always` only when the nearest tag matches `CARGO_PKG_VERSION`; if the
manifest is ahead of or behind the git tags, the binary displays the
manifest version plus the short git revision. Crates.io builds have no
git metadata and display `CARGO_PKG_VERSION`.

crates.io publishing is a gated job in the same workflow, using
[crates.io Trusted Publishing](https://crates.io/docs/trusted-publishing)
(GitHub OIDC, no long-lived token). It needs a one-time bootstrap
because Trusted Publishing can only be configured for crates that
already exist:

## One-time bootstrap

This repository already has GitHub Releases/tags (`v0.1.0` and later),
so the first crates.io publish should use the next intentional Cargo
workspace version, not an old tag. If `Cargo.toml` still says `0.1.0`
while the latest GitHub Release is newer, bump the workspace manifest
before publishing.

1. `cargo login` with a token from <https://crates.io/settings/tokens>
   (scope: `publish-new` + `publish-update`).
2. Publish the workspace in dependency order:

   ```console
   for crate in animsmith-core animsmith-gltf animsmith-fbx animsmith-report animsmith; do
     cargo publish -p "$crate"
   done
   ```

   During the very first bootstrap, dependent-crate dry-runs cannot fully
   verify until their earlier `animsmith-*` dependencies exist in the
   crates.io index. `animsmith-core` should pass `cargo publish --dry-run`
   first; then publish in the order above and let each subsequent crate
   resolve the crate that was just published.

3. After each crate is accepted by crates.io, docs.rs automatically
   queues documentation for that crate. Check the docs.rs page for each
   crate after publishing; the manifests set `documentation` links and
   `[package.metadata.docs.rs]` so pure-Rust crates get Linux, macOS,
   and Windows docs.rs target pages. C-dependent crates (`animsmith-fbx`
   and the all-features CLI) use the Linux default docs.rs target because
   docs.rs builds on Linux and cannot rely on a Windows/macOS C toolchain;
   CI remains the source of truth for Linux/macOS/Windows support.
4. On crates.io, for **each** of the five crates: Settings → Trusted
   Publishing → add publisher — repository `mmannerm/animsmith`,
   workflow `main.yml`, no environment.
5. Arm the CI job:

   ```console
   gh variable set CRATES_IO_TRUSTED_PUBLISHING --body true
   ```

## Per-release afterwards

1. Merge the feature/fix changes.
2. Before release, bump:
   - `[workspace.package] version`
   - every internal `[workspace.dependencies] animsmith-*` version
   - schema documentation/examples that include the release-tagged
     schema URL, when the machine-readable schema is still current
3. Merge that as `chore(release): bump workspace version to X.Y.Z`.

The next `main` run creates the GitHub Release and, once Trusted
Publishing is armed, publishes all workspace crates in dependency order.
The publish job is idempotent and skips already-published versions.
