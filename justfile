# Task runner for animsmith. `just gates` green locally == PR CI green.

worktree_root := parent_directory(justfile_directory()) / "animsmith-worktrees"

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

# Debug build of the whole workspace.
build:
    cargo build --workspace

# Full test suite.
test:
    cargo test --workspace

# Render public docs with rustdoc warnings denied.
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
    RUSTDOCFLAGS="-D warnings" cargo doc -p animsmith --no-default-features --no-deps

release-alignment:
    scripts/check-release-alignment.sh

# Check the crate package inventories that CI validates before release.
package-inventory:
    #!/usr/bin/env bash
    set -euo pipefail
    for crate in animsmith-core animsmith-gltf animsmith-fbx animsmith-report animsmith; do
      cargo package --list -p "$crate" --allow-dirty >/dev/null
    done

# Full local PR gate, matching CI (includes release builds — expect
# minutes, not seconds). The GitHub workflow also verifies package
# assembly on a clean checkout.
gates: require-cargo-deny
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo check --workspace --examples
    cargo test --workspace
    cargo deny check
    just release-alignment
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
    RUSTDOCFLAGS="-D warnings" cargo doc -p animsmith --no-default-features --no-deps
    cargo test -p animsmith --test cli_contract --no-default-features
    cargo build -p animsmith --no-default-features
    cargo build -p animsmith --release
    cargo run -p animsmith --release -- --version
    cargo build -p animsmith --release --no-default-features
    cargo run -p animsmith --release --no-default-features -- --version
    just package-inventory

# See .agent-instructions/shared.md for the required env vars.
# Env-gated reference tests against licensed assets.
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
