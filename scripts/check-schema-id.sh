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

gap_codes=$(sed -nE 's/.*Self\("([^"]+)"\);/\1/p' crates/animsmith-core/src/evaluation.rs)
scope_codes=$(grep -RhoE 'EvaluationScope::new\("[^"]+"' crates/animsmith-core/src/checks \
  | sed -E 's/.*EvaluationScope::new\("([^"]+)"/\1/' \
  | sort -u)
for code in $gap_codes $scope_codes; do
  if ! grep -Fq "\`$code\`" docs/output.md; then
    fail "docs/output.md does not document built-in gap/scope code $code"
  fi
done

for removed_schema in \
  docs/schemas/output-v1.schema.json \
  docs/schemas/output-v2-preview.schema.json; do
  if [ -e "$removed_schema" ]; then
    fail "$removed_schema is a removed alpha contract and must not be restored"
  fi
done

legacy=$(git grep -nE \
  'JsonV2Preview|json-v2-preview|run_checks|as_diagnostic|legacy_diagnostic|enum Readiness|Finding::diagnostic|output-v2-preview' \
  -- ':!scripts/check-schema-id.sh' || true)
if [ -n "$legacy" ]; then
  fail "removed v1/preview API or format remains:\n$legacy"
fi

legacy_envelope=$(
  while IFS= read -r file; do
    awk '
      previous ~ /"schema_version"[[:space:]]*:[[:space:]]*1,/ &&
        /"command"[[:space:]]*:/ {
          print FILENAME ":" NR - 1 ":" previous
        }
      {
        previous = $0
      }
    ' "$file"
  done < <(git grep -lE \
    '"schema_version"[[:space:]]*:[[:space:]]*1,' \
    -- ':!scripts/check-schema-id.sh' || true)
)
if [ -n "$legacy_envelope" ]; then
  fail "removed outer output-v1 envelope remains:\n$legacy_envelope"
fi

if [ "$failures" -ne 0 ]; then
  exit 1
fi
