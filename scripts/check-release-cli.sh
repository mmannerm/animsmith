#!/usr/bin/env bash
# Prove that the CLI left at the conventional release path is the
# default-feature build, and print the provenance record an external
# evaluation should retain beside its results (issue #653).
#
# `just gates` and the reusable checks workflow both build the CLI twice: once
# with default features and once with `--no-default-features`. When both wrote
# to `target/release/animsmith`, the retained artifact was whichever variant
# ran last, and `--version` printed the same line either way -- so a pack
# evaluation could select a binary that silently rejects FBX.
#
# `scripts/check-github-community-files.sh` keeps the two builds writing to
# different directories. This script judges what a completed run actually left
# at each path, by behavior rather than by the version line: both binaries must
# admit glTF, which every build reads, while only the retained one may admit
# FBX or expose `report` and the isolated minimal one must refuse both. Running
# the same probes both ways is what shows the check can tell the variants apart
# rather than passing on anything that executes.
#
# It builds nothing. `just gates` runs it last of all, and the CI test job runs
# it as that job's final step, so it attests to the artifacts the whole run
# leaves behind rather than to something it just produced itself.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

export LC_ALL=C

# Where every `--no-default-features` command sends its artifacts, and
# therefore where the minimal binary this script probes is found.
NO_DEFAULT_TARGET="target/no-default-features"

# Self-authored fixtures, so the admission probes need no licensed asset.
FBX_FIXTURE="crates/animsmith-fbx/testdata/rigged_triangle.fbx"
GLTF_FIXTURE="examples/assets/clip.glb"

fail() {
  echo "release-cli: $*" >&2
  exit 1
}

# ---------------------------------------------------------------------------
# Capability probes: which variant is actually at each path.
# ---------------------------------------------------------------------------

binary_in() {
  local dir="$1"
  if [ -x "$dir/animsmith" ]; then
    printf '%s\n' "$dir/animsmith"
  elif [ -x "$dir/animsmith.exe" ]; then
    printf '%s\n' "$dir/animsmith.exe"
  else
    return 1
  fi
}

retained="$(binary_in target/release)" \
  || fail "target/release/animsmith is missing; run 'cargo build -p animsmith --release' first"
minimal="$(binary_in "$NO_DEFAULT_TARGET/release")" \
  || fail "$NO_DEFAULT_TARGET/release/animsmith is missing; run 'cargo build -p animsmith --release --no-default-features --target-dir target/no-default-features' first"

for fixture in "$FBX_FIXTURE" "$GLTF_FIXTURE"; do
  test -f "$fixture" || fail "$fixture is missing; the admission probes need it"
done

# Each probe proves a capability by running the binary, never by reading its
# version line, and names that capability for the diagnostics.
probe_gltf_admission() {
  local inspected
  inspected="$("$1" inspect "$GLTF_FIXTURE" 2>/dev/null)" || return 1
  grep -Fq 'skeleton: 3 bones' <<<"$inspected" && grep -Fq 'swing: 1.000s' <<<"$inspected"
}

probe_fbx_admission() {
  local inspected
  inspected="$("$1" inspect "$FBX_FIXTURE" 2>/dev/null)" || return 1
  grep -Fq 'skeleton: 3 bones' <<<"$inspected" && grep -Fq 'take: 1.000s' <<<"$inspected"
}

probe_report_command() {
  "$1" report --help >/dev/null 2>&1
}

# glTF is the format every build reads, so both artifacts must admit it. That
# is also what keeps the negative branch below honest: a minimal binary that
# failed every probe because it is broken rather than because it lacks the
# features would fail here instead of confirming the check.
universal_probes=(probe_gltf_admission)
universal_capabilities=(
  "glTF admission (inspect names the example clip's skeleton and its 'swing' clip)"
)

# The capabilities only the default feature set provides.
default_feature_probes=(probe_fbx_admission probe_report_command)
default_feature_capabilities=(
  "FBX admission (inspect names the fixture's skeleton and its 'take' clip)"
  "report subcommand (report --help)"
)

retained_probes=("${universal_probes[@]}" "${default_feature_probes[@]}")
retained_capabilities=("${universal_capabilities[@]}" "${default_feature_capabilities[@]}")

proven=""
for index in "${!retained_probes[@]}"; do
  capability="${retained_capabilities[index]}"
  "${retained_probes[index]}" "$retained" \
    || fail "$retained does not provide $capability, so it is not the default-feature build a released CLI ships"
  proven="${proven:+$proven; }$capability"
done

for index in "${!universal_probes[@]}"; do
  capability="${universal_capabilities[index]}"
  "${universal_probes[index]}" "$minimal" \
    || fail "$minimal does not provide $capability, which every build owes, so its refusals below would not mean 'this feature is absent'"
done

for index in "${!default_feature_probes[@]}"; do
  capability="${default_feature_capabilities[index]}"
  if "${default_feature_probes[index]}" "$minimal"; then
    fail "$minimal provides $capability, so these probes cannot tell a default-feature build from a minimal one"
  fi
done

# The version line is recorded, not trusted: `cli_contract.rs` pins its shape
# under both feature sets, and the probes above are what establish capability.
reported_version="$("$retained" --version)" || fail "$retained cannot report --version"
retained_version="${reported_version%%$'\n'*}"

# `--always` would degrade a tag-less checkout to a bare hash that reads like a
# describe, which is exactly wrong in a provenance record: CI checks out
# shallow and without tags, so the field would silently stop naming a release.
# Say so instead.
describe_of_head() {
  git describe --tags --dirty 2>/dev/null && return 0
  printf 'no tag reachable (shallow or untagged checkout)\n'
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d' ' -f1
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$1" | sed 's/.*= *//'
  else
    echo "unavailable (no sha256sum, shasum, or openssl on PATH)"
  fi
}

printf 'ok: %s admits glTF and FBX and exposes report; %s admits glTF and refuses the rest\n' "$retained" "$minimal"
cat <<RECORD
release-cli: provenance of the retained default-feature CLI
  binary:       $retained
  sha256:       $(sha256_of "$retained")
  version:      $retained_version
  capabilities: $proven
  commit:       $(git rev-parse HEAD 2>/dev/null || echo "unavailable outside a Git checkout")
  describe:     $(describe_of_head)
RECORD
