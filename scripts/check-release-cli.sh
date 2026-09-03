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
# Two independent things keep that from recurring:
#
#   1. a structural assertion that every feature-variant cargo command in the
#      justfile and in the reusable checks workflow redirects CARGO_TARGET_DIR
#      to the isolated directory, so the conventional paths cannot be
#      overwritten in the first place;
#   2. capability probes that discriminate the two artifacts by behavior. Both
#      binaries must admit glTF, which every build reads; only the retained one
#      may admit FBX or expose `report`, and the isolated minimal one must
#      refuse both. Running the same probes both ways is what proves the check
#      can tell the variants apart rather than passing on anything that
#      executes.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

export LC_ALL=C

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# The isolated directory feature-variant builds write to, and the exact token
# each file must carry on such a command line. The justfile spells it through
# its `no_default_target` variable; the workflow spells it literally.
NO_DEFAULT_TARGET="target/no-default-features"
JUSTFILE_ISOLATION='CARGO_TARGET_DIR="{{no_default_target}}"'
WORKFLOW_ISOLATION="CARGO_TARGET_DIR=${NO_DEFAULT_TARGET}"
WORKFLOW=".github/workflows/checks.yml"

# Self-authored fixtures, so the admission probes need no licensed asset.
FBX_FIXTURE="crates/animsmith-fbx/testdata/rigged_triangle.fbx"
GLTF_FIXTURE="examples/assets/clip.glb"

fail() {
  echo "release-cli: $*" >&2
  exit 1
}

# ---------------------------------------------------------------------------
# Structural contract: feature-variant builds stay out of the shared paths.
# ---------------------------------------------------------------------------

# Every assertion below matches whole lines, and a Windows checkout can hold
# CRLF, so read these files with the carriage returns removed.
lines_of() {
  tr -d '\r' <"$1"
}

# The cargo command lines in $1 that select a non-default feature set.
feature_variant_lines() {
  lines_of "$1" | grep -n -- '--no-default-features' | grep -F -- 'cargo ' || true
}

# Print every way $1 fails the isolation contract; return 1 when it fails any.
# This returns rather than exits so the self-test below can assert the
# rejection cases through the same code path the gate runs.
isolation_errors() {
  local file="$1" expected="$2" variant offenders minimal_release status=0

  variant="$(feature_variant_lines "$file")"
  offenders="$(grep -Fv -- "$expected" <<<"$variant" | grep -v '^$' || true)"
  if [ -n "$offenders" ]; then
    printf 'release-cli: %s runs a feature-variant cargo command without %s, so it can overwrite the default-feature artifact:\n%s\n' \
      "$file" "$expected" "$offenders" >&2
    status=1
  fi

  minimal_release="$(grep -F -- '--release --no-default-features' <<<"$variant" || true)"
  case "$minimal_release" in
  *"$expected"*) ;;
  *)
    printf 'release-cli: %s must build the release CLI without default features into %s, which is the artifact this check probes\n' \
      "$file" "$NO_DEFAULT_TARGET" >&2
    status=1
    ;;
  esac

  return "$status"
}

expect_rejection() {
  local file="$1" expected="$2" description="$3"

  if isolation_errors "$file" "$expected" >/dev/null 2>&1; then
    fail "self-test: the isolation contract accepted $description"
  fi
}

# Mutate the tracked files the three ways this contract exists to catch and
# require each mutant to be rejected, so the rule cannot silently degenerate
# into one that accepts anything. The tracked files themselves are the
# acceptance case, checked just above. A mutation that stops applying -- after
# the just variable is renamed, say -- leaves the mutant equal to its accepted
# source and fails here rather than passing quietly.
self_test_isolation() {
  lines_of justfile \
    | sed 's|CARGO_TARGET_DIR="{{no_default_target}}" cargo test|cargo test|' \
      >"$work/justfile-shared-target"
  lines_of "$WORKFLOW" \
    | sed "s|CARGO_TARGET_DIR=${NO_DEFAULT_TARGET} cargo build|cargo build|" \
      >"$work/checks-shared-target.yml"
  lines_of justfile \
    | grep -v -- '--release --no-default-features' >"$work/justfile-no-minimal-release"

  expect_rejection "$work/justfile-shared-target" "$JUSTFILE_ISOLATION" \
    "a justfile whose no-default-features test commands lost their target directory"
  expect_rejection "$work/checks-shared-target.yml" "$WORKFLOW_ISOLATION" \
    "a workflow whose no-default-features builds lost their target directory"
  expect_rejection "$work/justfile-no-minimal-release" "$JUSTFILE_ISOLATION" \
    "a justfile that stopped building the minimal release CLI"
}

grep -Fxq "no_default_target := \"${NO_DEFAULT_TARGET}\"" <(lines_of justfile) \
  || fail "justfile must define no_default_target := \"${NO_DEFAULT_TARGET}\""
isolation_errors justfile "$JUSTFILE_ISOLATION" \
  || fail "justfile does not isolate its feature-variant builds"
isolation_errors "$WORKFLOW" "$WORKFLOW_ISOLATION" \
  || fail "$WORKFLOW does not isolate its feature-variant builds"
self_test_isolation
echo "ok: feature-variant builds in justfile and $WORKFLOW write to $NO_DEFAULT_TARGET"

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
  || fail "target/release/animsmith is missing; run 'just release-cli' or 'cargo build -p animsmith --release'"
minimal="$(binary_in "$NO_DEFAULT_TARGET/release")" \
  || fail "$NO_DEFAULT_TARGET/release/animsmith is missing; run 'just release-cli'"

for fixture in "$FBX_FIXTURE" "$GLTF_FIXTURE"; do
  test -f "$fixture" || fail "$fixture is missing; the admission probes need it"
done

# Each probe names a capability for the diagnostics and proves it by executing
# the binary, never by reading its version line.
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
# also keeps the negative branch honest: a minimal binary that failed every
# probe because it is broken rather than because it lacks the features would
# fail here.
universal_probes=(
  "glTF admission (inspect names the example clip's skeleton and its 'swing' clip):probe_gltf_admission"
)

# The capabilities only the default feature set provides.
default_feature_probes=(
  "FBX admission (inspect names the fixture's skeleton and its 'take' clip):probe_fbx_admission"
  "report subcommand (report --help):probe_report_command"
)

# The version line names the compiled features, but a label the build stamps on
# itself is not a capability -- the probes below are the evidence. Assert only
# that both artifacts report this checkout's version.
manifest_versions="$(sed -n 's/^version = "\(.*\)"$/\1/p' <(lines_of Cargo.toml))"
manifest_version="${manifest_versions%%$'\n'*}"
test -n "$manifest_version" || fail "Cargo.toml has no workspace version"

require_version_line() {
  local binary="$1" reported line
  reported="$("$binary" --version)" || fail "$binary cannot report --version"
  line="${reported%%$'\n'*}"
  case "$line" in
  "animsmith $manifest_version"*) printf '%s\n' "$line" ;;
  *) fail "$binary reports '$line'; expected it to start with 'animsmith $manifest_version'" ;;
  esac
}

retained_version="$(require_version_line "$retained")"
require_version_line "$minimal" >/dev/null

proven=""
for entry in "${universal_probes[@]}" "${default_feature_probes[@]}"; do
  capability="${entry%%:*}"
  probe="${entry##*:}"
  "$probe" "$retained" \
    || fail "$retained does not provide $capability, so it is not the default-feature build a released CLI ships"
  proven="${proven:+$proven; }$capability"
done

for entry in "${universal_probes[@]}"; do
  capability="${entry%%:*}"
  probe="${entry##*:}"
  "$probe" "$minimal" \
    || fail "$minimal does not provide $capability, which every build owes, so its refusals below would not mean 'this feature is absent'"
done

for entry in "${default_feature_probes[@]}"; do
  capability="${entry%%:*}"
  probe="${entry##*:}"
  if "$probe" "$minimal"; then
    fail "$minimal provides $capability, so these probes cannot tell a default-feature build from a minimal one"
  fi
done

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
  describe:     $(git describe --tags --dirty --always 2>/dev/null || echo "unavailable outside a Git checkout")
RECORD
