#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

fail() {
  echo "package-inventory: $*" >&2
  exit 1
}

require_file() {
  test -f "$1" || fail "$1 is missing"
}

require_fixed_line() {
  local path="$1"
  local expected="$2"
  local message="$3"

  require_file "$path"
  grep -Fxq "$expected" "$path" || fail "$message"
}

workspace_members=()
while IFS= read -r member; do
  workspace_members+=("$member")
done < <(
  awk '
    $0 ~ /^[[:space:]]*members[[:space:]]*=[[:space:]]*\[/ { in_members = 1; next }
    in_members && /^[[:space:]]*\]/ { in_members = 0; next }
    in_members {
      gsub(/[",]/, "")
      gsub(/^[[:space:]]+|[[:space:]]+$/, "")
      if (length > 0) print
    }
  ' Cargo.toml
)

publishable_crates=()
publishable_manifests=()
publishable_members=()
published_readmes=()
published_doc_sources=()

for member in "${workspace_members[@]}"; do
  manifest="$member/Cargo.toml"
  require_file "$manifest"

  crate="$(
    sed -nE 's/^name[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "$manifest" | head -1
  )"
  test -n "$crate" || fail "$manifest must define package.name"

  if grep -Eq '^[[:space:]]*publish[[:space:]]*=[[:space:]]*false' "$manifest"; then
    continue
  fi

  publishable_crates+=("$crate")
  publishable_manifests+=("$manifest")
  publishable_members+=("$member")
done

test "${#publishable_crates[@]}" -gt 0 || fail "workspace has no publishable crates"

for ((i = 0; i < ${#publishable_crates[@]}; i++)); do
  crate="${publishable_crates[$i]}"
  manifest="${publishable_manifests[$i]}"
  member="${publishable_members[$i]}"
  readme=""

  if grep -Fxq 'readme = "README.md"' "$manifest"; then
    readme="$member/README.md"
    require_fixed_line \
      "$readme" \
      "# $crate" \
      "$readme must identify the crate-local README for $crate"
  elif grep -Fxq 'readme.workspace = true' "$manifest"; then
    readme="README.md"
    require_fixed_line README.md "# animsmith" "README.md must identify the CLI package README"
  else
    fail "$manifest must choose README.md explicitly or inherit the workspace README"
  fi
  published_readmes+=("$readme")

  if test -f "$member/src/lib.rs"; then
    published_doc_sources+=("$member/src/lib.rs")
  elif test -f "$member/src/main.rs"; then
    published_doc_sources+=("$member/src/main.rs")
  else
    fail "$member must provide src/lib.rs or src/main.rs for rustdoc"
  fi

  require_fixed_line \
    "$manifest" \
    "documentation = \"https://docs.rs/$crate\"" \
    "$manifest must set its docs.rs documentation URL"
  require_fixed_line \
    "$manifest" \
    "[package.metadata.docs.rs]" \
    "$manifest must declare docs.rs build metadata"
  require_fixed_line \
    "$manifest" \
    "include.workspace = true" \
    "$manifest must use the shared publish include list"
done

bad_repo_links="$(
  grep -Eho 'https://github\.com/mmannerm/animsmith/(blob|tree)/[^)[:space:]]+' \
    "${published_readmes[@]}" \
    "${published_doc_sources[@]}" \
    DESIGN.md \
    | grep -Ev 'https://github\.com/mmannerm/animsmith/(blob|tree)/main/' || true
)"
if [ -n "$bad_repo_links" ]; then
  fail "published README, rustdoc, and design repository links must use /main/ while pre-1.0 drift is accepted: $bad_repo_links"
fi

for crate in "${publishable_crates[@]}"; do
  echo "checking package inventory for $crate"
  inventory="$(cargo package --list -p "$crate" --allow-dirty)"
  test -n "$inventory"

  for path in Cargo.toml README.md; do
    printf '%s\n' "$inventory" | grep -Fxq "$path" || {
      fail "$crate package is missing $path"
    }
  done

  printf '%s\n' "$inventory" | grep -Eq '^src/(lib|main)\.rs$' || {
    fail "$crate package is missing its source entry point"
  }
done

# Dependent packages cannot run full `cargo package` verification until the
# matching internal animsmith-* dependency versions are in the crates.io index.
# The dependency root can and should fully verify.
cargo package -p animsmith-core --allow-dirty
