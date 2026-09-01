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

extract_workflow_job() {
  local job="$1"
  local workflow="${2:-.github/workflows/release-plz.yml}"
  awk -v header="  ${job}:" '
    $0 == header { capture = 1 }
    capture && $0 != header && $0 ~ /^  [[:alnum:]_-]+:$/ { exit }
    capture { print }
  ' "$workflow"
}

release_pr_job="$work/release-pr-job.yml"
release_job="$work/release-job.yml"
docs_package_job="$work/docs-package-job.yml"
extract_workflow_job release-pr >"$release_pr_job"
extract_workflow_job release >"$release_job"
extract_workflow_job docs-package .github/workflows/checks.yml >"$docs_package_job"
[[ -s "$release_pr_job" ]] || fail "release-pr workflow job is missing"
[[ -s "$release_job" ]] || fail "release publication workflow job is missing"
[[ -s "$docs_package_job" ]] || fail "docs-package workflow job is missing"

grep -Fq \
  "if: \${{ github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/main' && vars.RELEASE_PLZ_ARMED == 'true' }}" \
  "$release_pr_job" \
  || fail "release-pr dispatch must be restricted to refs/heads/main"
echo "ok: release-pr dispatch is restricted to main"

grep -Fq 'id: release_plz' "$release_pr_job" \
  || fail "release-pr action must expose its generated-PR outputs"
grep -Fq 'ref: ${{ fromJSON(steps.release_plz.outputs.pr).head_branch }}' \
  "$release_pr_job" \
  || fail "release-pr job must check out the exact generated branch"
grep -Fq 'ANIMSMITH_RELEASE_PR: "true"' "$release_pr_job" \
  || fail "generated release branch must enable strict version-doc validation"
grep -Fq 'run: cargo test -p animsmith --test release_version_docs' "$release_pr_job" \
  || fail "release-pr job must run the version-doc gate after generation"
echo "ok: generated release branch runs strict version-doc validation"

history_step='        run: bash scripts/prepare-release-plz-history.sh'
[[ "$(grep -Fxc "$history_step" "$release_pr_job")" == "1" ]] \
  || fail "release-pr must prepare the pinned historical tree exactly once"
history_line="$(grep -Fxn "$history_step" "$release_pr_job" | cut -d: -f1)"
release_plz_line="$(grep -Fn 'id: release_plz' "$release_pr_job" | cut -d: -f1)"
[[ "$history_line" -lt "$release_plz_line" ]] \
  || fail "historical preparation must run before release-plz"
cleanup_step='        run: bash scripts/prepare-release-plz-history.sh --remove'
[[ "$(grep -Fxc "$cleanup_step" "$release_pr_job")" == "1" ]] \
  || fail "release-pr must remove its local history preparation exactly once"
cleanup_line="$(grep -Fxn "$cleanup_step" "$release_pr_job" | cut -d: -f1)"
[[ "$cleanup_line" -gt "$release_plz_line" ]] \
  || fail "historical preparation cleanup must run after release-plz"
grep -Fq "if: \${{ always() && steps.history_prepare.outcome == 'success' }}" \
  "$release_pr_job" \
  || fail "historical preparation cleanup must run after release-plz failures"
grep -Fq \
  'uses: release-plz/action@2eb1d8bcb770b4c48ccfaad919734b38b51958c9 # v0.5.131' \
  "$release_pr_job" \
  || fail "release-pr must pin the reviewed release-plz action commit"
[[ "$(grep -Fxc '          version: 0.3.160' "$release_pr_job")" == "1" ]] \
  || fail "release-pr must pin the release-plz binary whose history-copy behavior was proved"
[[ "$(grep -Fxc \
  '      - uses: dtolnay/rust-toolchain@4be7066ada62dd38de10e7b70166bc74ed198c30 # stable' \
  "$release_pr_job")" == "1" ]] \
  || fail "release-pr must pin the reviewed Rust toolchain action commit"
[[ "$(grep -Fc \
  'actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7.0.0' \
  "$release_pr_job")" == "2" ]] \
  || fail "both release-pr checkouts must pin the reviewed checkout action commit"
if grep -Fq 'prepare-release-plz-history.sh' "$release_job"; then
  fail "release publication must never use the local history preparation"
fi
grep -Fq \
  'https://github.com/release-plz/release-plz/releases/download/release-plz-v0.3.160/release-plz-x86_64-unknown-linux-gnu.tar.gz' \
  .github/workflows/checks.yml \
  || fail "hosted history traversal must download the pinned release-plz 0.3.160 asset"
grep -Fq '2263c4f95eac1513da96a114a77fde20ea038742a8c8050f7514b8f93b828646' \
  .github/workflows/checks.yml \
  || fail "hosted history traversal must verify the pinned release-plz archive digest"
grep -Fq 'RELEASE_PLZ_BIN: ${{ runner.temp }}/release-plz' \
  .github/workflows/checks.yml \
  || fail "hosted release packaging contract must execute the pinned release-plz binary"
[[ "$(grep -Fxc '          fetch-depth: 0' "$docs_package_job")" == "1" ]] \
  || fail "hosted release packaging contract requires complete Git history"

history_repo="$work/history-repo"
git clone --quiet --no-hardlinks . "$history_repo"
cp scripts/prepare-release-plz-history.sh \
  "$history_repo/scripts/prepare-release-plz-history.sh"
before_commits="$(git -C "$history_repo" rev-list --count v0.9.0..HEAD)"
before_log="$(git -C "$history_repo" log --format='%H %P %s' v0.9.0..HEAD)"
before_status="$(git -C "$history_repo" status --porcelain=v1)"
(
  cd "$history_repo"
  bash scripts/prepare-release-plz-history.sh
) >"$work/history-preparation.out"
expected_replacements="$(printf '%s\n' \
  64e1be84626b6180e332b212c2b9388f5dab6fcf \
  977abd11b4f533cac7b5e15b8fead935326a06ac \
  e518b2bdbed64c85e8dc91e09cefec4dff9d77cc | sort)"
[[ "$(git -C "$history_repo" replace -l | sort)" == "$expected_replacements" ]] \
  || fail "history preparation did not install exactly the pinned commit replacements"
while IFS='|' read -r original replacement; do
  [[ "$(git -C "$history_repo" rev-parse "refs/replace/$original")" == "$replacement" ]] \
    || fail "history preparation installed an unexpected replacement for $original"
  [[ "$(git -C "$history_repo" show \
    "$original:crates/animsmith/Cargo.toml" | git -C "$history_repo" hash-object --stdin)" == \
    "c5d334ef32c8a4a5d43141adb6f5588bdc52f7b6" ]] \
    || fail "history preparation did not expose the repaired manifest for $original"
  [[ "$(GIT_NO_REPLACE_OBJECTS=1 git -C "$history_repo" show \
    "$original:crates/animsmith/Cargo.toml" | git -C "$history_repo" hash-object --stdin)" == \
    "ddc60e13f87db14e506649745885beed2cf248f7" ]] \
    || fail "history preparation changed the underlying manifest for $original"
done <<'REPLACEMENTS'
e518b2bdbed64c85e8dc91e09cefec4dff9d77cc|0e79fb9e974dd05c4d651e016b611c6966c607fd
977abd11b4f533cac7b5e15b8fead935326a06ac|f553d3d8415efda55985199fec97fd0f9821e868
64e1be84626b6180e332b212c2b9388f5dab6fcf|957a16ea0be754672b06b22dbdd75e5105e812a7
REPLACEMENTS
after_commits="$(git -C "$history_repo" rev-list --count v0.9.0..HEAD)"
[[ "$after_commits" == "$before_commits" ]] \
  || fail "history preparation changed the release commit range"
after_log="$(git -C "$history_repo" log --format='%H %P %s' v0.9.0..HEAD)"
[[ "$after_log" == "$before_log" ]] \
  || fail "history preparation changed release commit identities or messages"
[[ "$(git -C "$history_repo" status --porcelain=v1)" == \
  "$before_status" ]] \
  || fail "history preparation changed checkout status"

original_head="$(git -C "$history_repo" rev-parse HEAD)"
for historical_commit in \
  e518b2bdbed64c85e8dc91e09cefec4dff9d77cc \
  977abd11b4f533cac7b5e15b8fead935326a06ac \
  64e1be84626b6180e332b212c2b9388f5dab6fcf; do
  git -C "$history_repo" checkout --quiet --detach "$historical_commit"
  cargo metadata --manifest-path "$history_repo/Cargo.toml" --no-deps \
    --format-version 1 >/dev/null
done
git -C "$history_repo" checkout --quiet --detach "$original_head"
[[ -z "$(git -C "$history_repo" status --porcelain=v1)" ]] \
  || fail "historical checkout traversal did not restore a clean HEAD"
(
  cd "$history_repo"
  bash scripts/prepare-release-plz-history.sh --remove
) >"$work/history-removal.out"
[[ -z "$(git -C "$history_repo" replace -l)" ]] \
  || fail "history preparation cleanup left a replacement ref"

for mutation in tree parents manifest; do
  mutant="$work/prepare-release-plz-history-$mutation.sh"
  cp scripts/prepare-release-plz-history.sh "$mutant"
  case "$mutation" in
    tree)
      old='4acff6acd7012804e6519017da52796ec7918405'
      ;;
    parents)
      old='48bd80f294f9ca1a54ae312e1cc0ab1f5175f4d8 a45f630b1c62626635ac2be41290a76b6b033afc'
      ;;
    manifest)
      old='ddc60e13f87db14e506649745885beed2cf248f7'
      ;;
  esac
  MUTANT="$mutant" OLD="$old" "$python" - <<'PY'
import os
from pathlib import Path

path = Path(os.environ["MUTANT"])
text = path.read_text(encoding="utf-8")
old = os.environ["OLD"]
if old not in text:
    raise SystemExit(f"mutation target is missing: {old}")
path.write_text(text.replace(old, "0" * 40, 1), encoding="utf-8")
PY
  if (cd "$history_repo" && bash "$mutant") >"$work/$mutation.out" 2>"$work/$mutation.err"; then
    fail "history preparation accepted a drifted $mutation identity"
  fi
done

untracked_mutant="$work/prepare-release-plz-history-untracked.sh"
cp scripts/prepare-release-plz-history.sh "$untracked_mutant"
MUTANT="$untracked_mutant" "$python" - <<'PY'
import os
from pathlib import Path

path = Path(os.environ["MUTANT"])
text = path.read_text(encoding="utf-8")
needle = 'status_before="$(git status --porcelain=v1)"'
replacement = needle + '\n: > .release-plz-history-untracked-drift'
if needle not in text:
    raise SystemExit("status snapshot insertion point is missing")
path.write_text(text.replace(needle, replacement, 1), encoding="utf-8")
PY
untracked_repo="$work/untracked-drift-repo"
git clone --quiet --no-hardlinks . "$untracked_repo"
if (cd "$untracked_repo" && bash "$untracked_mutant") \
  >"$work/untracked-drift.out" 2>"$work/untracked-drift.err"; then
  fail "history preparation accepted untracked checkout drift"
fi
grep -Fq 'history preparation changed the checkout status' \
  "$work/untracked-drift.err" \
  || fail "untracked checkout drift did not reach the status invariant"
[[ -z "$(git -C "$untracked_repo" replace -l)" ]] \
  || fail "untracked checkout drift left replacement refs behind"

original_head="$(git -C "$history_repo" rev-parse HEAD)"
git -C "$history_repo" checkout --quiet --detach v0.9.0
if (cd "$history_repo" && bash scripts/prepare-release-plz-history.sh) \
  >"$work/non-ancestor.out" 2>"$work/non-ancestor.err"; then
  fail "history preparation accepted a HEAD that does not descend from the pinned commit"
fi
git -C "$history_repo" checkout --quiet --detach "$original_head"

git -C "$history_repo" replace \
  c5d334ef32c8a4a5d43141adb6f5588bdc52f7b6 \
  ddc60e13f87db14e506649745885beed2cf248f7
if (cd "$history_repo" && bash scripts/prepare-release-plz-history.sh) \
  >"$work/unexpected-replace.out" 2>"$work/unexpected-replace.err"; then
  fail "history preparation accepted an unrelated replacement ref"
fi
git -C "$history_repo" replace -d c5d334ef32c8a4a5d43141adb6f5588bdc52f7b6 >/dev/null

if (
  cd "$history_repo"
  trap 'bash scripts/prepare-release-plz-history.sh --remove >/dev/null' EXIT
  bash scripts/prepare-release-plz-history.sh >/dev/null
  false
); then
  fail "simulated release-plz failure unexpectedly succeeded"
fi
[[ -z "$(git -C "$history_repo" replace -l)" ]] \
  || fail "failure-path cleanup left a replacement ref"
echo "ok: release-pr preserves full history and parses the pinned transient merge tree"

if [[ -n "${RELEASE_PLZ_BIN:-}" ]]; then
  [[ -x "$RELEASE_PLZ_BIN" ]] || fail "RELEASE_PLZ_BIN is not executable"
  "$RELEASE_PLZ_BIN" --version | grep -Fq 'release-plz 0.3.160' \
    || fail "release-plz traversal probe must use version 0.3.160"
  release_plz_repo="$work/release-plz-repo"
  git clone --quiet --no-hardlinks . "$release_plz_repo"
  cp scripts/prepare-release-plz-history.sh \
    "$release_plz_repo/scripts/prepare-release-plz-history.sh"
  (
    cd "$release_plz_repo"
    bash scripts/prepare-release-plz-history.sh
    "$RELEASE_PLZ_BIN" update --config release-plz.toml
    bash scripts/prepare-release-plz-history.sh --remove >/dev/null
  ) >"$work/release-plz-update.out"
  for package in \
    animsmith-core animsmith-engine animsmith-gltf animsmith-fbx \
    animsmith-report animsmith; do
    grep -Fq "\`$package\`: 0.9.0 -> 0.10.0" "$work/release-plz-update.out" \
      || fail "release-plz traversal did not compute 0.10.0 for $package"
  done
  [[ -z "$(git -C "$release_plz_repo" replace -l)" ]] \
    || fail "release-plz traversal probe cleanup left a replacement ref"
  echo "ok: pinned release-plz 0.3.160 traverses the repaired history and computes 0.10.0"
else
  echo "skip: set RELEASE_PLZ_BIN to exercise the pinned release-plz traversal"
fi

grep -Fq \
  "if: \${{ github.event_name == 'push' && vars.RELEASE_PLZ_ARMED == 'true' }}" \
  "$release_job" \
  || fail "release publishing must remain restricted to push events"
echo "ok: release publishing is restricted to push events"

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

matrix_known_fixture="$work/release-binaries-known-matrix.yml"
cp "$workflow_fixture" "$matrix_known_fixture"
cat >>"$matrix_known_fixture" <<'EOF'
    name: ${{ matrix.os }}-${{ matrix.target }}
EOF
"$python" "$targets_script" --manifest "$target_fixture" --workflow "$matrix_known_fixture" check-workflow
echo "ok: check-workflow accepts generated build job matrix fields"

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
    name: ${{ format('{0}-{1}-{2}-{3}', matrix.ext, matrix.bin, matrix['archive-ext'], matrix.archive_extension-extra) }}
EOF
if "$python" "$targets_script" --manifest "$target_fixture" --docs "$docs_fixture" --workflow "$matrix_contract_fixture" check \
  >/dev/null 2>"$work/unknown-matrix-check.err"; then
  fail "check accepted a build job matrix reference that is not generated"
fi
grep -Fq "matrix.archive-ext" "$work/unknown-matrix-check.err" \
  || fail "top-level unknown matrix field error did not name archive-ext: $(cat "$work/unknown-matrix-check.err")"
grep -Fq "matrix.archive_extension-extra" "$work/unknown-matrix-check.err" \
  || fail "top-level unknown matrix field error did not name archive_extension-extra: $(cat "$work/unknown-matrix-check.err")"
grep -Fq "matrix.bin" "$work/unknown-matrix-check.err" \
  || fail "top-level unknown matrix field error did not name bin: $(cat "$work/unknown-matrix-check.err")"
grep -Fq "matrix.ext" "$work/unknown-matrix-check.err" \
  || fail "top-level unknown matrix field error did not name ext: $(cat "$work/unknown-matrix-check.err")"
grep -Fq "scripts/release-targets.py write" "$work/unknown-matrix-check.err" \
  || fail "top-level unknown matrix field error did not name the write remedy: $(cat "$work/unknown-matrix-check.err")"
if "$python" "$targets_script" --manifest "$target_fixture" --workflow "$matrix_contract_fixture" check-workflow \
  >/dev/null 2>"$work/unknown-matrix.err"; then
  fail "check-workflow accepted a build job matrix reference that is not generated"
fi
grep -Fq "matrix.archive-ext" "$work/unknown-matrix.err" \
  || fail "unknown matrix field error did not name archive-ext: $(cat "$work/unknown-matrix.err")"
grep -Fq "matrix.archive_extension-extra" "$work/unknown-matrix.err" \
  || fail "unknown matrix field error did not name archive_extension-extra: $(cat "$work/unknown-matrix.err")"
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
