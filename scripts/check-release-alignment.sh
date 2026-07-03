#!/usr/bin/env bash
set -euo pipefail

failures=0

fail() {
  echo "release-alignment: $*" >&2
  failures=$((failures + 1))
}

workspace_version=$(
  awk -F '"' '
    $0 == "[workspace.package]" { in_section = 1; next }
    /^\[/ { in_section = 0 }
    in_section && /^[[:space:]]*version[[:space:]]*=/ { print $2; exit }
  ' Cargo.toml
)

if [ -z "$workspace_version" ]; then
  fail "could not read [workspace.package] version from Cargo.toml"
fi

for crate in animsmith-core animsmith-gltf animsmith-fbx animsmith-report; do
  line=$(grep -E "^[[:space:]]*$crate[[:space:]]*=" Cargo.toml || true)
  if [ -z "$line" ]; then
    fail "missing workspace dependency for $crate"
    continue
  fi
  dep_version=$(printf '%s\n' "$line" \
    | sed -nE 's/.*version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p')
  if [ "$dep_version" != "$workspace_version" ]; then
    fail "$crate dependency version $dep_version != workspace version $workspace_version"
  fi
done

schema_id=$(sed -nE 's/.*"\$id"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' \
  docs/schemas/output-v1.schema.json | head -1)
if [ -z "$schema_id" ]; then
  fail "docs/schemas/output-v1.schema.json has no \$id"
elif printf '%s\n' "$schema_id" | grep -Eq '/v[0-9]+\.[0-9]+\.[0-9]+/docs/schemas/output-v1\.schema\.json$'; then
  schema_version=$(printf '%s\n' "$schema_id" \
    | sed -nE 's#.*/v([0-9]+\.[0-9]+\.[0-9]+)/docs/schemas/output-v1\.schema\.json$#\1#p')
  if [ "$schema_version" != "$workspace_version" ]; then
    fail "schema \$id tag v$schema_version != workspace version v$workspace_version"
  fi
elif ! printf '%s\n' "$schema_id" | grep -Eq '/main/docs/schemas/output-v1\.schema\.json$'; then
  fail "schema \$id must point at main or at a matching v$workspace_version tag"
fi

for file in crates/animsmith/src/main.rs docs/output.md; do
  if ! grep -Fq "$schema_id" "$file"; then
    fail "$file does not reference schema \$id $schema_id"
  fi
done

if [ "$failures" -ne 0 ]; then
  exit 1
fi
