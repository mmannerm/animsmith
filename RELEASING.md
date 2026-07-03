# Releasing

GitHub Releases are fully automated: every merge to `main` whose
conventional commits warrant a bump (breaking → major, `feat` → minor,
`fix`/`perf` → patch) gets a Release with git-cliff notes, published by
`.github/workflows/main.yml`. Nothing to do.

crates.io publishing is a gated job in the same workflow, using
[crates.io Trusted Publishing](https://crates.io/docs/trusted-publishing)
(GitHub OIDC, no long-lived token). It needs a one-time bootstrap
because Trusted Publishing can only be configured for crates that
already exist:

## One-time bootstrap

1. `cargo login` with a token from <https://crates.io/settings/tokens>
   (scope: `publish-new` + `publish-update`).
2. Publish the workspace in dependency order:

   ```console
   for crate in animsmith-core animsmith-gltf animsmith-fbx animsmith-report animsmith; do
     cargo publish -p "$crate"
   done
   ```

3. On crates.io, for **each** of the five crates: Settings → Trusted
   Publishing → add publisher — repository `mmannerm/animsmith`,
   workflow `main.yml`, no environment.
4. Arm the CI job:

   ```console
   gh variable set CRATES_IO_TRUSTED_PUBLISHING --body true
   ```

## Per-release afterwards

The publish job runs after every GitHub Release, but only actually
publishes when the Cargo workspace version matches the release tag —
otherwise it skips with a warning. So a release that should reach
crates.io needs a `chore(release): bump workspace version to X.Y.Z` PR
merged **before** the feat/fix PR that triggers the release (or re-run
the `main` workflow after landing the bump; the job is idempotent and
skips already-published versions).
