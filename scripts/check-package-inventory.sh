#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

crate_local_readmes=(
  animsmith-core
  animsmith-gltf
  animsmith-fbx
  animsmith-report
)
all_crates=("${crate_local_readmes[@]}" animsmith)

for crate in "${crate_local_readmes[@]}"; do
  manifest="crates/$crate/Cargo.toml"
  readme="crates/$crate/README.md"
  grep -qx 'readme = "README.md"' "$manifest" || {
    echo "$manifest must point at its crate-local README.md" >&2
    exit 1
  }
  grep -qx "# $crate" "$readme" || {
    echo "$readme must identify the crate-local README for $crate" >&2
    exit 1
  }
done

grep -qx 'readme.workspace = true' crates/animsmith/Cargo.toml || {
  echo "crates/animsmith/Cargo.toml must keep the workspace/root README" >&2
  exit 1
}

for crate in "${all_crates[@]}"; do
  inventory="$(cargo package --list -p "$crate" --allow-dirty)"
  test -n "$inventory"
  printf '%s\n' "$inventory" | grep -qx 'README.md' || {
    echo "$crate package is missing README.md" >&2
    exit 1
  }
done
