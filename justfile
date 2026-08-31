# Task runner for animsmith. `just gates` green locally == PR CI green.

worktree_root := parent_directory(justfile_directory()) / "animsmith-worktrees"
# Keep generated documentation beside, never inside, this checkout.  This is
# portable and avoids sharing a mutable staging directory between worktrees.
docs_stage := env_var_or_default("ANIMSMITH_DOCS_STAGE", justfile_directory() + "-docs-site")

# Install local Rust build tools used by this workspace. `RUSTC_WRAPPER=`
# is intentional: this bootstraps sccache even when the user's Cargo
# config already enables it as the rustc wrapper.
install-rust-tools:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v sccache >/dev/null; then
      RUSTC_WRAPPER= cargo install sccache --locked
    fi
    if ! command -v cargo-deny >/dev/null; then
      RUSTC_WRAPPER= cargo install cargo-deny --locked
    fi
    if ! command -v typos >/dev/null; then
      RUSTC_WRAPPER= cargo install typos-cli --locked
    fi
    if ! command -v cargo-llvm-cov >/dev/null; then
      RUSTC_WRAPPER= cargo install cargo-llvm-cov --locked
    fi

configure-sccache: require-sccache
    #!/usr/bin/env bash
    set -euo pipefail
    cargo_home="${CARGO_HOME:-$HOME/.cargo}"
    cargo_config="$cargo_home/config.toml"
    if [ ! -e "$cargo_config" ] && [ -e "$cargo_home/config" ]; then
      cargo_config="$cargo_home/config"
    fi
    mkdir -p "$(dirname "$cargo_config")"
    touch "$cargo_config"
    if grep -Eq '^[[:space:]]*rustc-wrapper[[:space:]]*=' "$cargo_config"; then
      echo "$cargo_config already configures rustc-wrapper"
      exit 0
    fi
    if grep -Eq '^[[:space:]]*\[build\][[:space:]]*$' "$cargo_config"; then
      echo "$cargo_config already has a [build] table." >&2
      echo "Add these entries there:" >&2
      echo '  rustc-wrapper = "sccache"' >&2
      echo '  incremental = false' >&2
      exit 1
    fi
    {
      printf '\n[build]\n'
      printf 'rustc-wrapper = "sccache"\n'
      printf 'incremental = false\n'
    } >> "$cargo_config"
    echo "Configured Cargo to use sccache in $cargo_config"

require-sccache:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v sccache >/dev/null || {
      echo "sccache not found; run 'just install-rust-tools' before building." >&2
      exit 1
    }

require-cargo-deny:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v cargo-deny >/dev/null || {
      echo "cargo-deny not found; run 'just install-rust-tools' before running gates." >&2
      exit 1
    }

require-typos:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v typos >/dev/null || {
      echo "typos not found; run 'just install-rust-tools' before running gates." >&2
      exit 1
    }

# Debug build of the whole workspace.
build:
    cargo build --workspace

# Full test suite.
test:
    cargo test --workspace

# Render public docs with rustdoc warnings and missing docs denied.
doc:
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --workspace --no-deps
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc -p animsmith --no-default-features --no-deps

# Stage canonical tracked Markdown and generate navigation from docs/README.md.
docs-stage:
    python3 scripts/build-docs-site.py --stage "{{docs_stage}}"

# Build a clean, parser-validated Pages preview. mdBook must match .mdbook-version.
docs-check:
    python3 scripts/build-docs-site.py --stage "{{docs_stage}}" --build
    python3 scripts/test_build_docs_site.py
    cargo test -p animsmith --test docs_pages

# Serve the same staged Pages book locally at http://localhost:3000.
docs-serve:
    python3 scripts/build-docs-site.py --stage "{{docs_stage}}"
    cd "{{docs_stage}}" && mdbook serve -d book

schema-id:
    scripts/check-schema-id.sh

# Validate GitHub community files and issue-form contracts.
github-community:
    bash scripts/check-github-community-files.sh

# Spell-check source, comments, and docs (allow-list in _typos.toml).
typos: require-typos
    typos

# Line/region coverage via cargo-llvm-cov. Prints the summary table and
# writes an HTML report under target/llvm-cov/html; CI uploads the lcov
# form to Codecov (see .github/workflows/coverage.yml).
coverage:
    cargo llvm-cov --workspace --html

# Check the publishable crate package readiness rules that CI validates.
package-inventory:
    bash scripts/check-package-inventory.sh

# Verify the isolated exact-Bevy lock remains bound by the normal-workspace contract.
bevy-readback-lock:
    bash scripts/check-bevy-readback-lock.sh

# Opt-in Bevy 0.19 isolated compile/runtime contract matrix; not part of CI.
bevy-readback-test:
    bash scripts/test-bevy-readback.sh

# Contract coverage for release binary packaging + CLI tag detection.
release-packaging:
    bash scripts/check-release-packaging.sh

# Behavioral and published-report checks for the reusable animation-pack skill.
animation-pack-skill:
    cargo build -p animsmith --bin animsmith
    ANIMSMITH_TEST_BINARY=target/debug/animsmith PYTHONDONTWRITEBYTECODE=1 python3 .agents/skills/evaluate-animation-packs/scripts/test_validators.py

report-browser:
    #!/usr/bin/env bash
    set -euo pipefail
    report_path="$(mktemp)"
    trap 'rm -f "${report_path}"' EXIT
    cargo run -q -p animsmith -- --config examples/report-comparison.animsmith.toml report \
      examples/assets/report-comparison-before.glb \
      --compare-after examples/assets/report-comparison-after.glb \
      --before-clip acceptance-matrix --after-clip acceptance-matrix \
      --output "${report_path}"
    node scripts/test-comparison-viewer.js "${report_path}"

# Full local PR gate, matching CI (includes release builds — expect
# minutes, not seconds). The GitHub workflow also verifies package
# assembly on a clean checkout.
gates: require-cargo-deny require-typos
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo check --workspace --examples
    # Dev-only schema validation also enables this serde_json feature during
    # tests. Pin it independently in the shipped glTF dependency graph.
    cargo tree -p animsmith-gltf --edges features,no-dev | grep -F 'serde_json feature "float_roundtrip"'
    cargo test --workspace
    just bevy-readback-lock
    bash scripts/check-golden-skip-marker.sh
    cargo deny check
    just schema-id
    just github-community
    just docs-check
    typos
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --workspace --no-deps
    RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc -p animsmith --no-default-features --no-deps
    cargo test -p animsmith --test cli_contract --no-default-features
    cargo test -p animsmith --test foot_cycle_cli --no-default-features
    cargo test -p animsmith --test measure_mesh --no-default-features
    cargo test -p animsmith --test scale_cli --no-default-features
    cargo build -p animsmith --no-default-features
    cargo build -p animsmith --release
    cargo run -p animsmith --release -- --version
    cargo build -p animsmith --release --no-default-features
    cargo run -p animsmith --release --no-default-features -- --version
    just package-inventory
    just release-packaging
    just animation-pack-skill
    just report-browser

# See .agent-instructions/shared.md for the required env var.
# Env-gated reference tests against licensed assets plus CI-visible FBX coverage.
golden:
    cargo test -p animsmith-gltf --test golden -- --nocapture
    cargo test -p animsmith --test convert_mesh -- --nocapture

# One worktree per substantial task; parallel agents don't collide.
# New worktree on a fresh branch off freshly fetched origin/main.
worktree branch:
    #!/usr/bin/env bash
    set -euo pipefail
    branch="{{branch}}"
    dir="{{worktree_root}}/${branch}"
    if git show-ref --quiet --verify "refs/heads/${branch}"; then
        echo "Branch '${branch}' already exists. Pick a new name or remove it first." >&2
        exit 1
    fi
    git fetch origin main
    git worktree add -b "${branch}" "${dir}" origin/main
    echo
    echo "Worktree ready: ${dir}"
    echo "  branch '${branch}' off freshly fetched origin/main"

# Uncommitted changes are reported and kept, never deleted.
# Remove worktrees whose branch has merged and is gone from the remote.
worktree-prune:
    #!/usr/bin/env bash
    set -euo pipefail
    git fetch --prune origin
    git worktree list --porcelain | awk '/^worktree /{print $2}' | while read -r dir; do
        [ "$dir" = "{{justfile_directory()}}" ] && continue
        branch=$(git -C "$dir" branch --show-current || true)
        [ -z "$branch" ] && continue
        case "$branch" in main|master) continue;; esac
        if ! git -C "$dir" diff --quiet || ! git -C "$dir" diff --cached --quiet; then
            echo "KEEP  $dir ($branch): uncommitted changes"
            continue
        fi
        if ! git show-ref --quiet --verify "refs/remotes/origin/${branch}" \
           || [ "$(git merge-base "origin/main" "$branch")" = "$(git rev-parse "$branch")" ]; then
            echo "PRUNE $dir ($branch)"
            git worktree remove "$dir"
            git branch -D "$branch" 2>/dev/null || true
        fi
    done
    git worktree prune
