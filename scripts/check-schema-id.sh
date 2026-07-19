#!/usr/bin/env bash
# Verify the machine-readable output schemas' $ids are internally
# consistent: stable v1 points at `main` (or a matching release tag), the
# preview points at its exact incubating URL, and the CLI/docs reference
# those exact ids.
#
# Crate version bumps and internal animsmith-* dependency alignment are
# now owned by release-plz (see #37); this check is purely about the
# schema URL and the places that must reference it.
set -euo pipefail

failures=0

fail() {
  echo "schema-id: $*" >&2
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

preview_schema_id=$(sed -nE 's/.*"\$id"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' \
  docs/schemas/output-v2-preview.schema.json | head -1)
expected_preview_schema_id="https://raw.githubusercontent.com/mmannerm/animsmith/main/docs/schemas/output-v2-preview.schema.json"
if [ -z "$preview_schema_id" ]; then
  fail "docs/schemas/output-v2-preview.schema.json has no \$id"
elif [ "$preview_schema_id" != "$expected_preview_schema_id" ]; then
  fail "preview schema \$id must be $expected_preview_schema_id"
fi

preview_schema_const=$(sed -nE \
  '/"schema"[[:space:]]*:/,/}/ s/.*"const"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' \
  docs/schemas/output-v2-preview.schema.json | head -1)
if [ -z "$preview_schema_const" ]; then
  fail "preview schema properties.schema has no const"
elif [ "$preview_schema_const" != "$preview_schema_id" ]; then
  fail "preview schema properties.schema const does not match \$id"
fi

for file in crates/animsmith/src/main.rs docs/output.md; do
  if ! grep -Fq "$preview_schema_id" "$file"; then
    fail "$file does not reference preview schema \$id $preview_schema_id"
  fi
done

if [ "$failures" -ne 0 ]; then
  exit 1
fi
