#!/usr/bin/env bash
# Local contract coverage for the release binary workflow (issue #113).
#
# Exercises the two pieces of release-binaries.yml / release-plz.yml that
# would otherwise only ever run in CI:
#   1. package-release-binary.py: archive contents + matching .sha256.
#   2. select-cli-release-tag.sh: release-present / no-release-skip /
#      missing-CLI-tag detection branches.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

python="${PYTHON:-python3}"
package_script="scripts/package-release-binary.py"
select_script="scripts/select-cli-release-tag.sh"

command -v "$python" >/dev/null || {
  echo "python3 not found; required for release packaging coverage" >&2
  exit 1
}
command -v jq >/dev/null || {
  echo "jq not found; required for release tag detection coverage" >&2
  exit 1
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

# Docs bundled into every release archive, mirroring release-binaries.yml.
extras=(README.md LICENSE-APACHE LICENSE-MIT THIRD-PARTY.md)

# --- packaging: archive contents + .sha256 ------------------------------

check_packaging() {
  local ext="$1"
  local stem="animsmith-vtest-target-${ext//./-}"
  local out_dir="$work/dist-$ext"
  local binary="$work/animsmith-fake"

  mkdir -p "$out_dir"
  printf 'not a real binary\n' >"$binary"

  "$python" "$package_script" \
    --binary "$binary" \
    --stem "$stem" \
    --ext "$ext" \
    --out-dir "$out_dir" \
    "${extras[@]}"

  local archive="$out_dir/$stem.$ext"
  local checksum="$archive.sha256"
  [[ -f "$archive" ]] || fail "$ext: archive not produced at $archive"
  [[ -f "$checksum" ]] || fail "$ext: checksum sidecar not produced at $checksum"

  # Archive holds exactly the binary + docs under a single <stem>/ prefix.
  local expected members
  expected="$(printf '%s\n' \
    "$stem/animsmith-fake" \
    "$stem/README.md" \
    "$stem/LICENSE-APACHE" \
    "$stem/LICENSE-MIT" \
    "$stem/THIRD-PARTY.md" | sort)"
  members="$(
    ARCHIVE="$archive" EXT="$ext" "$python" - <<'PY'
import os
import tarfile
import zipfile

archive = os.environ["ARCHIVE"]
ext = os.environ["EXT"]
if ext == "tar.gz":
    with tarfile.open(archive, "r:gz") as tar:
        names = [m.name for m in tar.getmembers() if m.isfile()]
elif ext == "zip":
    with zipfile.ZipFile(archive) as zf:
        names = [i.filename for i in zf.infolist() if not i.is_dir()]
else:
    raise SystemExit(f"unsupported archive extension: {ext}")
print("\n".join(sorted(names)))
PY
  )"
  [[ "$members" == "$expected" ]] || fail "$ext: archive contents mismatch
expected:
$expected
got:
$members"

  # Sidecar is `<sha256>  <archive name>` and the digest matches the bytes.
  local sidecar_name sidecar_digest actual_digest
  sidecar_name="$(awk '{print $2}' "$checksum")"
  sidecar_digest="$(awk '{print $1}' "$checksum")"
  [[ "$sidecar_name" == "$stem.$ext" ]] \
    || fail "$ext: checksum names '$sidecar_name', expected '$stem.$ext'"
  actual_digest="$(
    ARCHIVE="$archive" "$python" - <<'PY'
import hashlib
import os

print(hashlib.sha256(open(os.environ["ARCHIVE"], "rb").read()).hexdigest())
PY
  )"
  [[ "$sidecar_digest" == "$actual_digest" ]] \
    || fail "$ext: checksum digest mismatch ($sidecar_digest != $actual_digest)"

  echo "ok: packaging $ext -> $stem.$ext (+ .sha256)"
}

check_packaging tar.gz
check_packaging zip

# --- detection: release-present / skip / missing-CLI-tag ----------------

# Release cut for the CLI package -> emit that tag.
tag="$(
  RELEASES_CREATED=true \
  RELEASES='[{"package_name":"animsmith-core","tag":"animsmith-core-v9.9.9"},{"package_name":"animsmith","tag":"animsmith-v1.2.3"}]' \
    "$select_script"
)"
[[ "$tag" == "animsmith-v1.2.3" ]] \
  || fail "detection: expected animsmith-v1.2.3, got '$tag'"
echo "ok: detection selects CLI tag when a release is present"

# No release cut this run -> no tag, skip binaries (exit 0).
tag="$(RELEASES_CREATED=false RELEASES='[]' "$select_script")"
[[ -z "$tag" ]] || fail "detection: expected empty tag on no-release, got '$tag'"
echo "ok: detection skips (no tag) when no release was created"

# Empty releases_created is also a skip.
tag="$(RELEASES_CREATED='' RELEASES='' "$select_script")"
[[ -z "$tag" ]] || fail "detection: expected empty tag on empty input, got '$tag'"
echo "ok: detection skips (no tag) when releases_created is unset"

# Release cut but nothing for the CLI package -> hard error.
if RELEASES_CREATED=true \
   RELEASES='[{"package_name":"animsmith-core","tag":"animsmith-core-v9.9.9"}]' \
   "$select_script" >/dev/null 2>&1; then
  fail "detection: expected failure when no CLI release tag is present"
fi
echo "ok: detection fails when a release omits the CLI package"

echo "release packaging contract checks passed"
