# Task runner for animsmith. `just gates` green locally == PR CI green.

worktree_root := parent_directory(justfile_directory()) / "animsmith-worktrees"

# Debug build of the whole workspace.
build:
    cargo build --workspace

# Full test suite.
test:
    cargo test --workspace

# Everything PR CI runs, in the same order (.github/workflows/ci.yml).
gates:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo build -p animsmith --no-default-features

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
