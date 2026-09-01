#!/usr/bin/env bash
# Give release-plz a parseable, local view of three transiently invalid
# historical commits. This never rewrites or pushes history: refs/replace is
# local to the release-pr checkout and runner. The replacements are
# commit-scoped so checking out later commits still exposes canonical blobs.
set -euo pipefail

manifest_path="crates/animsmith/Cargo.toml"
bad_manifest_blob="ddc60e13f87db14e506649745885beed2cf248f7"
fixed_manifest_blob="c5d334ef32c8a4a5d43141adb6f5588bdc52f7b6"
mode="${1:-prepare}"

# target commit | original tree | original parents | repaired tree | repaired commit
readarray -t repairs <<'REPAIRS'
e518b2bdbed64c85e8dc91e09cefec4dff9d77cc|4acff6acd7012804e6519017da52796ec7918405|48bd80f294f9ca1a54ae312e1cc0ab1f5175f4d8 a45f630b1c62626635ac2be41290a76b6b033afc|d9784512d32d16762fc93fbbffc3a31d09672aa6|0e79fb9e974dd05c4d651e016b611c6966c607fd
977abd11b4f533cac7b5e15b8fead935326a06ac|95264e3bfe272eaaa9c7c5087b23535f40186de6|e518b2bdbed64c85e8dc91e09cefec4dff9d77cc 587a499fe0fc422c8a13f8ecbd147ae429febd97|fd1468c6327edda591e5eb1c1cf96ed9029d1b58|f553d3d8415efda55985199fec97fd0f9821e868
64e1be84626b6180e332b212c2b9388f5dab6fcf|a7f197a189f5a1f188d88afaebcd19325c8e4538|977abd11b4f533cac7b5e15b8fead935326a06ac dd12e499c8fc6e538e318056c282b100c7db6f07|6ed5f17407a67de1bd70a4f1dcb66f98d4d69637|957a16ea0be754672b06b22dbdd75e5105e812a7
REPAIRS

fail() {
  echo "release-plz history preparation refused: $*" >&2
  exit 1
}

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" \
  || fail "the checkout is not a Git repository"
cd "$repo_root"
[[ "$mode" == "prepare" || "$mode" == "--remove" ]] \
  || fail "usage: $0 [--remove]"

git cat-file -e "$fixed_manifest_blob^{blob}" 2>/dev/null \
  || fail "pinned repaired manifest blob $fixed_manifest_blob is unavailable"

expected_refs="$(printf '%s\n' "${repairs[@]}" | cut -d'|' -f1 | sort)"
declare -a target_commits=()
declare -a fixed_commits=()

for repair in "${repairs[@]}"; do
  IFS='|' read -r target_commit target_tree target_parents fixed_tree fixed_commit \
    <<<"$repair"
  target_commits+=("$target_commit")
  fixed_commits+=("$fixed_commit")

  git cat-file -e "$target_commit^{commit}" 2>/dev/null \
    || fail "required full-history commit $target_commit is unavailable"
  git merge-base --is-ancestor "$target_commit" HEAD \
    || fail "historical commit $target_commit is not an ancestor of HEAD"

  actual_tree="$(GIT_NO_REPLACE_OBJECTS=1 git rev-parse "$target_commit^{tree}")"
  [[ "$actual_tree" == "$target_tree" ]] \
    || fail "historical tree changed for $target_commit: expected $target_tree, found $actual_tree"
  actual_parents="$(GIT_NO_REPLACE_OBJECTS=1 git show -s --format=%P "$target_commit")"
  [[ "$actual_parents" == "$target_parents" ]] \
    || fail "historical parents changed for $target_commit"
  actual_bad_blob="$(GIT_NO_REPLACE_OBJECTS=1 git rev-parse "$target_commit:$manifest_path")"
  [[ "$actual_bad_blob" == "$bad_manifest_blob" ]] \
    || fail "historical manifest changed for $target_commit: found blob $actual_bad_blob"

  original_animsmith_tree="$(GIT_NO_REPLACE_OBJECTS=1 \
    git rev-parse "$target_commit:crates/animsmith")"
  fixed_animsmith_tree="$(
    GIT_NO_REPLACE_OBJECTS=1 git ls-tree "$original_animsmith_tree" \
      | sed "s/^100644 blob $bad_manifest_blob$(printf '\t')Cargo.toml\$/100644 blob $fixed_manifest_blob$(printf '\t')Cargo.toml/" \
      | git mktree
  )"
  [[ "$(git rev-parse "$fixed_animsmith_tree:Cargo.toml")" == "$fixed_manifest_blob" ]] \
    || fail "could not construct the repaired manifest tree for $target_commit"

  original_crates_tree="$(GIT_NO_REPLACE_OBJECTS=1 git rev-parse "$target_commit:crates")"
  fixed_crates_tree="$(
    GIT_NO_REPLACE_OBJECTS=1 git ls-tree "$original_crates_tree" \
      | sed "s/^040000 tree $original_animsmith_tree$(printf '\t')animsmith\$/040000 tree $fixed_animsmith_tree$(printf '\t')animsmith/" \
      | git mktree
  )"
  constructed_fixed_tree="$(
    GIT_NO_REPLACE_OBJECTS=1 git ls-tree "$actual_tree" \
      | sed "s/^040000 tree $original_crates_tree$(printf '\t')crates\$/040000 tree $fixed_crates_tree$(printf '\t')crates/" \
      | git mktree
  )"
  [[ "$constructed_fixed_tree" == "$fixed_tree" ]] \
    || fail "repaired tree drifted for $target_commit: constructed $constructed_fixed_tree"

  constructed_fixed_commit="$(
    {
      printf 'tree %s\n' "$fixed_tree"
      GIT_NO_REPLACE_OBJECTS=1 git cat-file commit "$target_commit" | sed '1d'
    } | git hash-object -t commit -w --stdin
  )"
  [[ "$constructed_fixed_commit" == "$fixed_commit" ]] \
    || fail "repaired commit drifted for $target_commit: constructed $constructed_fixed_commit"
done

all_replacements="$(git replace -l | sort)"
if [[ -n "$all_replacements" && "$all_replacements" != "$expected_refs" ]]; then
  fail "replacement refs are partial or include an unexpected object"
fi

if [[ "$mode" == "--remove" ]]; then
  if [[ -z "$all_replacements" ]]; then
    echo "release-plz history preparation was already absent"
    exit 0
  fi
  for index in "${!target_commits[@]}"; do
    target_commit="${target_commits[$index]}"
    fixed_commit="${fixed_commits[$index]}"
    [[ "$(git rev-parse "refs/replace/$target_commit")" == "$fixed_commit" ]] \
      || fail "refusing to remove an unexpected replacement for $target_commit"
  done
  git replace -d "${target_commits[@]}" >/dev/null
  echo "removed local release-plz history preparation"
  exit 0
fi

if [[ -n "$all_replacements" ]]; then
  for index in "${!target_commits[@]}"; do
    [[ "$(git rev-parse "refs/replace/${target_commits[$index]}")" == \
      "${fixed_commits[$index]}" ]] \
      || fail "an unexpected replacement exists for ${target_commits[$index]}"
  done
  echo "release-plz history was already prepared"
  exit 0
fi

head_before="$(git rev-parse HEAD)"
history_before="$(git log --format='%H %P %s' v0.9.0..HEAD | git hash-object --stdin)"
status_before="$(git status --porcelain=v1)"
installed=()
rollback() {
  local status="$?"
  if ((${#installed[@]})); then
    git replace -d "${installed[@]}" >/dev/null 2>&1 || true
  fi
  return "$status"
}
trap rollback EXIT
for index in "${!target_commits[@]}"; do
  git replace "${target_commits[$index]}" "${fixed_commits[$index]}"
  installed+=("${target_commits[$index]}")
done

for index in "${!target_commits[@]}"; do
  target_commit="${target_commits[$index]}"
  fixed_commit="${fixed_commits[$index]}"
  [[ "$(git rev-parse "refs/replace/$target_commit")" == "$fixed_commit" ]] \
    || fail "installed replacement does not match the pinned commit pair"
  [[ "$(git show "$target_commit:$manifest_path" | git hash-object --stdin)" == \
    "$fixed_manifest_blob" ]] \
    || fail "ordinary historical reads do not expose the repaired manifest"
  [[ "$(GIT_NO_REPLACE_OBJECTS=1 git show "$target_commit:$manifest_path" \
    | git hash-object --stdin)" == "$bad_manifest_blob" ]] \
    || fail "the underlying historical manifest object changed"
done
[[ "$(git rev-parse HEAD)" == "$head_before" ]] \
  || fail "history preparation changed HEAD"
[[ "$(git status --porcelain=v1)" == "$status_before" ]] \
  || fail "history preparation changed the checkout status"
history_after="$(git log --format='%H %P %s' v0.9.0..HEAD | git hash-object --stdin)"
[[ "$history_after" == "$history_before" ]] \
  || fail "history preparation changed release commit identities or messages"
trap - EXIT

echo "prepared local release-plz history for ${#target_commits[@]} commits"
