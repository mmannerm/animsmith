#!/usr/bin/env bash
# Verify that each published contract uses its immutable protocol identity and
# that the emitting CLI and contract documentation reference the same ids.
set -euo pipefail

failures=0

fail() {
  echo "schema-id: $*" >&2
  failures=$((failures + 1))
}

check_schema() {
  file=$1
  expected=$2
  schema_id=$(sed -nE 's/.*"\$id"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "$file" | head -1)
  schema_const=$(sed -nE \
    '/"schema"[[:space:]]*:/,/}/ s/.*"const"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' \
    "$file" | head -1)

  if [ "$schema_id" != "$expected" ]; then
    fail "$file \$id must be $expected (found ${schema_id:-none})"
  fi
  if [ "$schema_const" != "$expected" ]; then
    fail "$file properties.schema const must be $expected (found ${schema_const:-none})"
  fi
  for reference in crates/animsmith-core/src/contract.rs docs/output.md; do
    if ! grep -Fq "$expected" "$reference"; then
      fail "$reference does not reference schema identity $expected"
    fi
  done
}

check_schema docs/schemas/output-v2.schema.json urn:animsmith:schema:output:2
check_schema docs/schemas/measurements-v1.schema.json urn:animsmith:schema:measurements:1

# Cutover-only #204 guard: scan every tracked file until output v2 has its first
# public release, then remove this name tombstone. Behavioural tests separately
# prove that old report inputs are rejected.
legacy=$(git grep -nE \
  'JsonV2Preview|json-v2-preview|run_checks|as_diagnostic|legacy_diagnostic|enum Readiness|Finding::diagnostic|output-v2-preview|presentation_findings|assert_required_properties|CheckOutput::complete|CheckOutput::partial|CheckOutput::not_evaluated|CheckOutput::complete_scoped' \
  -- ':!scripts/check-schema-id.sh' || true)
if [ -n "$legacy" ]; then
  fail "removed v1/preview API or format remains:\n$legacy"
fi

if [ "$failures" -ne 0 ]; then
  exit 1
fi
