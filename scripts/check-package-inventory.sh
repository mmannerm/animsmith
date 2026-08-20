#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
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

# Cargo is a native Windows executable under Git Bash, so its metadata paths
# use `C:\...` while `pwd -P` uses `/c/...`. Compare the same mixed absolute
# form on MSYS/Cygwin and leave native Unix paths unchanged.
normalize_path_for_compare() {
  local path="$1"
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -am "$path"
  else
    printf '%s\n' "$path"
  fi
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
unpublished_crates=()
published_readmes=()
published_source_entries=()

# docs.rs always attempts `cargo rustdoc --lib`. The CLI package is
# intentionally bin-only, so its docs.rs landing/source/features pages are
# useful package metadata, but a rustdoc build is explicitly exempt until a
# meaningful public library target is introduced.
bin_only_docs_rs_exemptions=(animsmith)

for member in "${workspace_members[@]}"; do
  manifest="$member/Cargo.toml"
  require_file "$manifest"

  crate="$(
    sed -nE 's/^name[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "$manifest" | head -1
  )"
  test -n "$crate" || fail "$manifest must define package.name"

  if grep -Eq '^[[:space:]]*publish[[:space:]]*=[[:space:]]*false' "$manifest"; then
    unpublished_crates+=("$crate")
    continue
  fi

  publishable_crates+=("$crate")
  publishable_manifests+=("$manifest")
  publishable_members+=("$member")
done

test "${#publishable_crates[@]}" -gt 0 || fail "workspace has no publishable crates"

command -v jq >/dev/null || fail "jq is required to inspect Cargo target metadata"
workspace_metadata="$(cargo metadata --format-version 1 --no-deps)"

# Cargo target metadata is the authority for what rustdoc can build. File names
# and TOML headers alone are insufficient because explicit targets can redirect
# either source entry point.
for ((i = 0; i < ${#publishable_crates[@]}; i++)); do
  crate="${publishable_crates[$i]}"
  manifest="${publishable_manifests[$i]}"
  member="${publishable_members[$i]}"
  library_sources=()
  while IFS= read -r source; do
    library_sources+=("$source")
  done < <(
    jq -r --arg crate "$crate" '
      .packages[]
      | select(.name == $crate)
      | .targets[]
      | select(.kind | any(
          . == "lib" or . == "rlib" or . == "dylib" or . == "cdylib"
          or . == "staticlib" or . == "proc-macro"
        ))
      | .src_path
    ' <<<"$workspace_metadata"
  )
  binary_sources=()
  while IFS= read -r source; do
    binary_sources+=("$source")
  done < <(
    jq -r --arg crate "$crate" '
      .packages[]
      | select(.name == $crate)
      | .targets[]
      | select(.kind | index("bin"))
      | .src_path
    ' <<<"$workspace_metadata"
  )

  if printf '%s\n' "${bin_only_docs_rs_exemptions[@]}" | grep -Fxq "$crate"; then
    test ! -f "$member/src/lib.rs" || {
      fail "$crate docs.rs bin-only exemption must not have a library source"
    }
    test "${#library_sources[@]}" -eq 0 || {
      fail "$crate docs.rs bin-only exemption must not have a Cargo library target"
    }
    test -f "$member/src/main.rs" || {
      fail "$crate is exempt from docs.rs rustdoc only when its bin source exists"
    }
    grep -Eq '^[[:space:]]*\[\[bin\]\][[:space:]]*$' "$manifest" || {
      fail "$crate docs.rs bin-only exemption requires an explicit [[bin]] target"
    }
    test "${#binary_sources[@]}" -eq 1 || {
      fail "$crate docs.rs bin-only exemption requires exactly one Cargo binary target"
    }
    actual_source="$(normalize_path_for_compare "${binary_sources[0]}")"
    expected_source="$(normalize_path_for_compare "$repo_root/$member/src/main.rs")"
    test "$actual_source" = "$expected_source" || {
      fail "$crate docs.rs bin-only target must use $member/src/main.rs"
    }
    published_source_entries+=("$member/src/main.rs")
  else
    test "${#library_sources[@]}" -eq 1 || {
      fail "$crate must have exactly one Cargo library target or use an explicit bin-only docs.rs exemption"
    }
    test -f "$member/src/lib.rs" || {
      fail "$crate library target requires $member/src/lib.rs"
    }
    actual_source="$(normalize_path_for_compare "${library_sources[0]}")"
    expected_source="$(normalize_path_for_compare "$repo_root/$member/src/lib.rs")"
    test "$actual_source" = "$expected_source" || {
      fail "$crate library target must use $member/src/lib.rs"
    }
    published_source_entries+=("$member/src/lib.rs")
  fi
done

if [ "${ANIMSMITH_PACKAGE_INVENTORY_TARGET_POLICY_ONLY:-0}" = "1" ]; then
  exit 0
fi

# Cargo retains versioned dev-dependencies in a published manifest. Internal
# fixture crates that are deliberately not published must therefore be used
# only as path-only dev-dependencies, which Cargo omits from the package.
for manifest in "${publishable_manifests[@]}"; do
  for unpublished_crate in "${unpublished_crates[@]}"; do
    invalid_dependency="$(
      awk -v dependency="$unpublished_crate" '
        /^\[[^]]+\]$/ { section = $0 }
        $0 ~ "^[[:space:]]*" dependency "[[:space:]]*=" {
          is_dev = section ~ /dev-dependencies\]$/
          has_path = $0 ~ /path[[:space:]]*=/
          has_workspace = $0 ~ /workspace[[:space:]]*=/
          has_version = $0 ~ /version[[:space:]]*=/
          if (!is_dev || !has_path || has_workspace || has_version) {
            print section ": " $0
          }
        }
      ' "$manifest"
    )"

    if test -n "$invalid_dependency"; then
      fail "$manifest must reference unpublished workspace crate $unpublished_crate only as a path-only dev-dependency: $invalid_dependency"
    fi
  done
done

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
    "${published_source_entries[@]}" \
    DESIGN.md \
    | grep -Ev 'https://github\.com/mmannerm/animsmith/(blob|tree)/main/' || true
)"
if [ -n "$bad_repo_links" ]; then
  fail "published README, source entry, and design repository links must use /main/ while pre-1.0 drift is accepted: $bad_repo_links"
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

workspace_version="$({
  sed -nE '/^\[workspace\.package\]$/,/^\[/ s/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' Cargo.toml
} | head -1)"
test -n "$workspace_version" || fail "Cargo.toml must define workspace.package.version"

# A published crate may be vendored beneath another workspace whose root also
# has a Cargo.toml. Prove repository-doc tests recognize the exact animsmith
# source layout rather than treating that unrelated consumer root as ours.
relocated_root="$(mktemp -d "${TMPDIR:-/tmp}/animsmith-package-relocated.XXXXXX")"
trap 'rm -rf "$relocated_root"' EXIT
mkdir -p "$relocated_root/crates"
cp -R \
  "target/package/animsmith-core-$workspace_version" \
  "$relocated_root/crates/animsmith-core"
printf '%s\n' \
  '[workspace]' \
  'members = []' \
  'exclude = ["crates/animsmith-core"]' \
  'resolver = "3"' \
  > "$relocated_root/Cargo.toml"
cargo test --locked --all-features --manifest-path \
  "target/package/animsmith-core-$workspace_version/Cargo.toml"
cargo test --locked --all-features --manifest-path \
  "$relocated_root/crates/animsmith-core/Cargo.toml"
