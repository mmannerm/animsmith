# Task runner for animsmith. `just gates` green locally == PR CI green.

worktree_root := parent_directory(justfile_directory()) / "animsmith-worktrees"

# Install local Rust build tools used by this workspace. `RUSTC_WRAPPER=`
# is intentional: this bootstraps sccache before Cargo can use it as the
# configured rustc wrapper.
install-rust-tools:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v sccache >/dev/null; then
      RUSTC_WRAPPER= cargo install sccache --locked
    fi

require-sccache:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v sccache >/dev/null || {
      echo "sccache not found; run 'just install-rust-tools' before building." >&2
      exit 1
    }

# Debug build of the whole workspace.
build: require-sccache
    cargo build --workspace

# Full test suite.
test: require-sccache
    cargo test --workspace

# Render public docs with rustdoc warnings denied.
doc: require-sccache
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
    RUSTDOCFLAGS="-D warnings" cargo doc -p animsmith --no-default-features --no-deps

# Check the crate package inventories that CI validates before release.
package-inventory: require-sccache
    #!/usr/bin/env bash
    set -euo pipefail
    for crate in animsmith-core animsmith-gltf animsmith-fbx animsmith-report animsmith; do
      cargo package --list -p "$crate" --allow-dirty >/dev/null
    done

# Fast local PR gate. The GitHub workflow also verifies package assembly
# on a clean checkout.
gates: require-sccache
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
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
golden: require-sccache
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
