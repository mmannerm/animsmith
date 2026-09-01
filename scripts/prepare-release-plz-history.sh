#!/usr/bin/env bash
# Give release-plz a parseable, local view of one transiently invalid
# historical manifest blob. This never rewrites or pushes history:
# refs/replace is local to the release-pr checkout and runner.
set -euo pipefail

target_commit="977abd11b4f533cac7b5e15b8fead935326a06ac"
target_tree="95264e3bfe272eaaa9c7c5087b23535f40186de6"
target_parents="e518b2bdbed64c85e8dc91e09cefec4dff9d77cc 587a499fe0fc422c8a13f8ecbd147ae429febd97"
manifest_path="crates/animsmith/Cargo.toml"
bad_manifest_blob="ddc60e13f87db14e506649745885beed2cf248f7"
fixed_manifest_blob="c5d334ef32c8a4a5d43141adb6f5588bdc52f7b6"
mode="${1:-prepare}"

fail() {
  echo "release-plz history preparation refused: $*" >&2
  exit 1
}

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" \
  || fail "the checkout is not a Git repository"
cd "$repo_root"

git cat-file -e "$target_commit^{commit}" 2>/dev/null \
  || fail "required full-history commit $target_commit is unavailable"
git merge-base --is-ancestor "$target_commit" HEAD \
  || fail "historical commit $target_commit is not an ancestor of HEAD"

actual_tree="$(GIT_NO_REPLACE_OBJECTS=1 git rev-parse "$target_commit^{tree}")"
[[ "$actual_tree" == "$target_tree" ]] \
  || fail "historical tree changed: expected $target_tree, found $actual_tree"
actual_parents="$(GIT_NO_REPLACE_OBJECTS=1 git show -s --format=%P "$target_commit")"
[[ "$actual_parents" == "$target_parents" ]] \
  || fail "historical parents changed: expected '$target_parents', found '$actual_parents'"
actual_bad_blob="$(GIT_NO_REPLACE_OBJECTS=1 git rev-parse "$target_commit:$manifest_path")"
[[ "$actual_bad_blob" == "$bad_manifest_blob" ]] \
  || fail "historical manifest changed: expected blob $bad_manifest_blob, found $actual_bad_blob"
git cat-file -e "$fixed_manifest_blob^{blob}" 2>/dev/null \
  || fail "pinned repaired manifest blob $fixed_manifest_blob is unavailable"

all_replacements="$(git replace -l)"
existing_replacement="$(git replace -l "$bad_manifest_blob")"
if [[ -n "$all_replacements" && "$all_replacements" != "$bad_manifest_blob" ]]; then
  fail "unexpected replacement refs already exist in this checkout"
fi

if [[ "$mode" == "--remove" ]]; then
  if [[ -z "$existing_replacement" ]]; then
    echo "release-plz history preparation was already absent"
    exit 0
  fi
  replacement_target="$(git rev-parse "refs/replace/$bad_manifest_blob")"
  [[ "$replacement_target" == "$fixed_manifest_blob" ]] \
    || fail "refusing to remove an unexpected replacement for $bad_manifest_blob"
  git replace -d "$bad_manifest_blob"
  echo "removed local release-plz history preparation for $bad_manifest_blob"
  exit 0
fi
[[ "$mode" == "prepare" ]] || fail "usage: $0 [--remove]"

if [[ -n "$existing_replacement" ]]; then
  replacement_target="$(git rev-parse "refs/replace/$bad_manifest_blob")"
  [[ "$replacement_target" == "$fixed_manifest_blob" ]] \
    || fail "an unexpected replacement already exists for $bad_manifest_blob"
  echo "release-plz history already prepared for $bad_manifest_blob"
  exit 0
fi

head_before="$(git rev-parse HEAD)"
history_before="$(git log --format='%H %P %s' v0.9.0..HEAD | git hash-object --stdin)"
git replace "$bad_manifest_blob" "$fixed_manifest_blob"

replacement_target="$(git rev-parse "refs/replace/$bad_manifest_blob")"
[[ "$replacement_target" == "$fixed_manifest_blob" ]] \
  || fail "installed replacement does not match the pinned blob pair"
resolved_manifest="$(git show "$target_commit:$manifest_path" | git hash-object --stdin)"
[[ "$resolved_manifest" == "$fixed_manifest_blob" ]] \
  || fail "ordinary historical reads do not expose the pinned repaired manifest"
underlying_manifest="$(GIT_NO_REPLACE_OBJECTS=1 git show "$target_commit:$manifest_path" \
  | git hash-object --stdin)"
[[ "$underlying_manifest" == "$bad_manifest_blob" ]] \
  || fail "the underlying historical manifest object changed"
[[ "$(git rev-parse HEAD)" == "$head_before" ]] \
  || fail "history preparation changed HEAD"
history_after="$(git log --format='%H %P %s' v0.9.0..HEAD | git hash-object --stdin)"
[[ "$history_after" == "$history_before" ]] \
  || fail "history preparation changed release commit identities or messages"

echo "prepared local release-plz history blob $bad_manifest_blob -> $fixed_manifest_blob"
