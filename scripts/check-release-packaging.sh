#!/usr/bin/env bash
# Local contract coverage for the release binary workflow (issue #113).
#
# Exercises the release-binaries.yml / release-plz.yml automation paths that
# would otherwise only ever run in CI:
#   1. package-release-binary.py: archive contents + matching .sha256.
#   2. select-cli-release-tag.sh: release-present / no-release-skip /
#      missing-CLI-tag detection branches.
#   3. release-targets.py: one canonical release target list for workflow
#      matrices and user-facing archive docs.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

python="${PYTHON:-python3}"
package_script="scripts/package-release-binary.py"
select_script="scripts/select-cli-release-tag.sh"
targets_script="scripts/release-targets.py"

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

# Verify a `<digest>  <name>` sidecar with the standard checksum tool the
# way a downstream user would (cwd must hold both sidecar and archive).
sha256_verify() {
  if command -v sha256sum >/dev/null; then
    sha256sum -c "$1" >/dev/null
  else
    shasum -a 256 -c "$1" >/dev/null
  fi
}

# Docs bundled into every release archive, mirroring release-binaries.yml.
extras=(README.md LICENSE-APACHE LICENSE-MIT THIRD-PARTY.md)

# --- release target metadata: workflow + docs ---------------------------

"$python" "$targets_script" check
echo "ok: release target workflow matrix and docs match release-targets.json"

README=README.md DOCS=docs/cli.md "$python" - <<'PY'
import os
import re
from pathlib import Path

readme = Path(os.environ["README"]).read_text(encoding="utf-8")
docs = Path(os.environ["DOCS"]).read_text(encoding="utf-8")
match = re.search(r"\[CLI guide\]\(([^)]+)\)", readme)
if not match:
    raise SystemExit("README.md must link supported archives to the CLI guide")
if match.group(1) != "https://github.com/mmannerm/animsmith/blob/main/docs/cli.md#install":
    raise SystemExit("README.md CLI guide link must target docs/cli.md#install")
if "\n## Install\n" not in f"\n{docs}":
    raise SystemExit("docs/cli.md must expose a ## Install anchor for README.md")
PY
echo "ok: README install link has a matching docs/cli.md anchor"

target_fixture="$work/release-targets.json"
docs_fixture="$work/cli.md"
workflow_fixture="$work/release-binaries.yml"
cat >"$target_fixture" <<'JSON'
{
  "release_targets": [
    {
      "platform": "Example OS",
      "os": "ubuntu-latest",
      "target": "example-target",
      "binary": "animsmith",
      "archive_extension": "tar.gz",
      "python": "python3"
    }
  ]
}
JSON
cat >"$docs_fixture" <<'EOF'
# fixture

before
<!-- release-targets:start -->
stale
<!-- release-targets:end -->
after
EOF

if "$python" "$targets_script" --manifest "$target_fixture" --docs "$docs_fixture" check-docs \
  >/dev/null 2>"$work/stale-docs.err"; then
  fail "check-docs accepted a stale release target table"
fi
grep -Fq "release target table is stale" "$work/stale-docs.err" \
  || fail "check-docs stale error did not name the stale table: $(cat "$work/stale-docs.err")"
grep -Fq "scripts/release-targets.py write" "$work/stale-docs.err" \
  || fail "check-docs stale error did not name the write remedy: $(cat "$work/stale-docs.err")"
echo "ok: check-docs rejects stale release target tables"

"$python" "$targets_script" --manifest "$target_fixture" --docs "$docs_fixture" write-docs
expected_docs="$(
  cat <<'EOF'
# fixture

before
<!-- release-targets:start -->
| Platform | Archive |
|---|---|
| Example OS | `animsmith-vX.Y.Z-example-target.tar.gz` |
<!-- release-targets:end -->
after
EOF
)"
actual_docs="$(cat "$docs_fixture")"
[[ "$actual_docs" == "$expected_docs" ]] \
  || fail "write-docs did not regenerate the release target block from the manifest"
"$python" "$targets_script" --manifest "$target_fixture" --docs "$docs_fixture" check-docs
echo "ok: write-docs regenerates the CLI archive table"

cat >"$workflow_fixture" <<'EOF'
jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          # release-targets:start
          - target: stale-target
          # release-targets:end
EOF

if "$python" "$targets_script" --manifest "$target_fixture" --docs "$docs_fixture" --workflow "$workflow_fixture" check \
  >/dev/null 2>"$work/stale-check.err"; then
  fail "check accepted a stale release target matrix"
fi
grep -Fq "release target matrix is stale" "$work/stale-check.err" \
  || fail "check stale error did not name the stale matrix: $(cat "$work/stale-check.err")"
grep -Fq "scripts/release-targets.py write" "$work/stale-check.err" \
  || fail "check stale error did not name the write remedy: $(cat "$work/stale-check.err")"
echo "ok: check rejects stale release target workflow matrices"

if "$python" "$targets_script" --manifest "$target_fixture" --workflow "$workflow_fixture" check-workflow \
  >/dev/null 2>"$work/stale-workflow.err"; then
  fail "check-workflow accepted a stale release target matrix"
fi
grep -Fq "release target matrix is stale" "$work/stale-workflow.err" \
  || fail "check-workflow stale error did not name the stale matrix: $(cat "$work/stale-workflow.err")"
grep -Fq "scripts/release-targets.py write" "$work/stale-workflow.err" \
  || fail "check-workflow stale error did not name the write remedy: $(cat "$work/stale-workflow.err")"

"$python" "$targets_script" --manifest "$target_fixture" --workflow "$workflow_fixture" write-workflow
expected_workflow="$(
  cat <<'EOF'
jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          # release-targets:start
          - os: "ubuntu-latest"
            target: "example-target"
            binary: "animsmith"
            archive_extension: "tar.gz"
            python: "python3"
          # release-targets:end
EOF
)"
actual_workflow="$(cat "$workflow_fixture")"
[[ "$actual_workflow" == "$expected_workflow" ]] \
  || fail "write-workflow did not regenerate the release target matrix from the manifest"
"$python" "$targets_script" --manifest "$target_fixture" --workflow "$workflow_fixture" check-workflow
echo "ok: write-workflow regenerates the release matrix"

matrix_comment_fixture="$work/release-binaries-commented-matrix.yml"
cp "$workflow_fixture" "$matrix_comment_fixture"
cat >>"$matrix_comment_fixture" <<'EOF'
    # ${{ matrix.ext }}
    name: build # ${{ matrix.bin }}
EOF
"$python" "$targets_script" --manifest "$target_fixture" --workflow "$matrix_comment_fixture" check-workflow
echo "ok: check-workflow ignores commented build job matrix references"

matrix_scope_fixture="$work/release-binaries-upload-matrix.yml"
cp "$workflow_fixture" "$matrix_scope_fixture"
cat >>"$matrix_scope_fixture" <<'EOF'

  upload:
    runs-on: ${{ matrix.ext }}
EOF
"$python" "$targets_script" --manifest "$target_fixture" --workflow "$matrix_scope_fixture" check-workflow
echo "ok: check-workflow scopes matrix field checks to the build job"

matrix_contract_fixture="$work/release-binaries-unknown-matrix.yml"
cp "$workflow_fixture" "$matrix_contract_fixture"
cat >>"$matrix_contract_fixture" <<'EOF'
    name: ${{ format('{0}-{1}-{2}', matrix.ext, matrix.bin, matrix['archive-ext']) }}
EOF
if "$python" "$targets_script" --manifest "$target_fixture" --docs "$docs_fixture" --workflow "$matrix_contract_fixture" check \
  >/dev/null 2>"$work/unknown-matrix-check.err"; then
  fail "check accepted a build job matrix reference that is not generated"
fi
grep -Fq "matrix.archive-ext" "$work/unknown-matrix-check.err" \
  || fail "top-level unknown matrix field error did not name archive-ext: $(cat "$work/unknown-matrix-check.err")"
grep -Fq "matrix.bin" "$work/unknown-matrix-check.err" \
  || fail "top-level unknown matrix field error did not name bin: $(cat "$work/unknown-matrix-check.err")"
grep -Fq "matrix.ext" "$work/unknown-matrix-check.err" \
  || fail "top-level unknown matrix field error did not name ext: $(cat "$work/unknown-matrix-check.err")"
if "$python" "$targets_script" --manifest "$target_fixture" --workflow "$matrix_contract_fixture" check-workflow \
  >/dev/null 2>"$work/unknown-matrix.err"; then
  fail "check-workflow accepted a build job matrix reference that is not generated"
fi
grep -Fq "matrix.archive-ext" "$work/unknown-matrix.err" \
  || fail "unknown matrix field error did not name archive-ext: $(cat "$work/unknown-matrix.err")"
grep -Fq "matrix.bin" "$work/unknown-matrix.err" \
  || fail "unknown matrix field error did not name bin: $(cat "$work/unknown-matrix.err")"
grep -Fq "matrix.ext" "$work/unknown-matrix.err" \
  || fail "unknown matrix field error did not name ext: $(cat "$work/unknown-matrix.err")"
grep -Fq "only generates" "$work/unknown-matrix.err" \
  || fail "unknown matrix field error did not describe the generated contract: $(cat "$work/unknown-matrix.err")"
echo "ok: check-workflow rejects build job matrix fields the generator does not emit"

cat >"$work/missing-start.md" <<'EOF'
# fixture

<!-- release-targets:end -->
EOF
if "$python" "$targets_script" --manifest "$target_fixture" --docs "$work/missing-start.md" check-docs \
  >/dev/null 2>"$work/missing-start.err"; then
  fail "check-docs accepted a table with a missing start marker"
fi
grep -Fq "missing <!-- release-targets:start -->" "$work/missing-start.err" \
  || fail "missing-start error did not name the missing marker: $(cat "$work/missing-start.err")"
grep -Fq "scripts/release-targets.py write" "$work/missing-start.err" \
  || fail "missing-start error did not name the write remedy: $(cat "$work/missing-start.err")"

cat >"$work/missing-end.md" <<'EOF'
# fixture

<!-- release-targets:start -->
EOF
if "$python" "$targets_script" --manifest "$target_fixture" --docs "$work/missing-end.md" check-docs \
  >/dev/null 2>"$work/missing-end.err"; then
  fail "check-docs accepted a table with a missing end marker"
fi
grep -Fq "missing <!-- release-targets:end -->" "$work/missing-end.err" \
  || fail "missing-end error did not name the missing marker: $(cat "$work/missing-end.err")"
grep -Fq "scripts/release-targets.py write" "$work/missing-end.err" \
  || fail "missing-end error did not name the write remedy: $(cat "$work/missing-end.err")"
echo "ok: check-docs rejects missing release target markers"

check_bad_manifest() {
  local name="$1"
  local expected="$2"
  local manifest="$work/bad-$name.json"
  local err="$work/bad-$name.err"

  cat >"$manifest"
  if "$python" "$targets_script" --manifest "$manifest" check-docs >/dev/null 2>"$err"; then
    fail "$name: invalid manifest unexpectedly passed"
  fi
  grep -Fq "$expected" "$err" \
    || fail "$name: expected error containing '$expected', got: $(cat "$err")"
  echo "ok: invalid manifest rejected ($name)"
}

check_bad_manifest missing-field "missing python" <<'JSON'
{
  "release_targets": [
    {
      "platform": "Example OS",
      "os": "ubuntu-latest",
      "target": "example-target",
      "binary": "animsmith",
      "archive_extension": "tar.gz"
    }
  ]
}
JSON

check_bad_manifest duplicate-target "duplicate release target example-target" <<'JSON'
{
  "release_targets": [
    {
      "platform": "Example OS",
      "os": "ubuntu-latest",
      "target": "example-target",
      "binary": "animsmith",
      "archive_extension": "tar.gz",
      "python": "python3"
    },
    {
      "platform": "Example OS 2",
      "os": "ubuntu-latest",
      "target": "example-target",
      "binary": "animsmith",
      "archive_extension": "tar.gz",
      "python": "python3"
    }
  ]
}
JSON

check_bad_manifest unsupported-extension "unsupported archive_extension '7z'" <<'JSON'
{
  "release_targets": [
    {
      "platform": "Example OS",
      "os": "ubuntu-latest",
      "target": "example-target",
      "binary": "animsmith",
      "archive_extension": "7z",
      "python": "python3"
    }
  ]
}
JSON

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

  # A packed member round-trips byte-for-byte, not just by name.
  local extracted="$work/extracted-$ext"
  ARCHIVE="$archive" EXT="$ext" MEMBER="$stem/animsmith-fake" OUT="$extracted" \
    "$python" - <<'PY'
import os
import tarfile
import zipfile

archive = os.environ["ARCHIVE"]
ext = os.environ["EXT"]
member = os.environ["MEMBER"]
out = os.environ["OUT"]
if ext == "tar.gz":
    with tarfile.open(archive, "r:gz") as tar:
        data = tar.extractfile(member).read()
elif ext == "zip":
    with zipfile.ZipFile(archive) as zf:
        data = zf.read(member)
else:
    raise SystemExit(f"unsupported archive extension: {ext}")
with open(out, "wb") as fh:
    fh.write(data)
PY
  cmp -s "$binary" "$extracted" \
    || fail "$ext: packed binary differs from the staged input"

  # The sidecar verifies with the standard checksum tool. This exercises
  # the actual download-verification contract and guards the two-space
  # `<digest>  <name>` format that `awk` above would silently tolerate.
  ( cd "$out_dir" && sha256_verify "$stem.$ext.sha256" ) \
    || fail "$ext: sidecar failed sha256 verification"

  # Mutating the archive must break verification (the digest is over the
  # archive bytes, not the staged tree).
  printf 'x' >>"$archive"
  if ( cd "$out_dir" && sha256_verify "$stem.$ext.sha256" ) 2>/dev/null; then
    fail "$ext: sidecar still verified after the archive was mutated"
  fi

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

# Several releases for the CLI package in one run -> the latest (last) wins.
tag="$(
  RELEASES_CREATED=true \
  RELEASES='[{"package_name":"animsmith","tag":"animsmith-v1.2.3"},{"package_name":"animsmith","tag":"animsmith-v1.2.4"}]' \
    "$select_script"
)"
[[ "$tag" == "animsmith-v1.2.4" ]] \
  || fail "detection: expected the latest CLI tag animsmith-v1.2.4, got '$tag'"
echo "ok: detection picks the latest CLI tag"

# Release cut but nothing for the CLI package -> hard error naming the package.
# `if err=$(...)` captures stderr while testing the exit code without set -e
# aborting on the expected failure.
if err="$(RELEASES_CREATED=true \
  RELEASES='[{"package_name":"animsmith-core","tag":"animsmith-core-v9.9.9"}]' \
  "$select_script" 2>&1 1>/dev/null)"; then
  fail "detection: expected failure when no CLI release tag is present"
fi
[[ "$err" == *animsmith* ]] \
  || fail "detection: missing-package error should name the CLI package, got '$err'"
echo "ok: detection fails when a release omits the CLI package"

# CLI record present but with no tag field -> jq emits "null"; must error,
# never leak a bare 'null' as a real tag.
if RELEASES_CREATED=true RELEASES='[{"package_name":"animsmith"}]' \
   "$select_script" >/dev/null 2>&1; then
  fail "detection: expected failure when the CLI release has no tag field"
fi
echo "ok: detection fails (no bare 'null') when the CLI record has no tag"

echo "release packaging contract checks passed"
