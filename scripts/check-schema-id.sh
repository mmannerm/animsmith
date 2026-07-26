#!/usr/bin/env bash
# Verify that each published contract uses its immutable protocol identity and
# that the shared core contract and its documentation reference the same ids.
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

for removed_schema in \
  docs/schemas/output-v1.schema.json \
  docs/schemas/output-v2-preview.schema.json; do
  if [ -e "$removed_schema" ]; then
    fail "$removed_schema is a removed alpha contract and must not be restored"
  fi
done

# Cutover-only #204 guard: scan every tracked file until output v2 has its first
# public release, then remove this name tombstone. Behavioural tests separately
# prove that old report inputs are rejected.
legacy=$(git grep -nE \
  'JsonV2Preview|json-v2-preview|run_checks|as_diagnostic|legacy_diagnostic|enum Readiness|Finding::diagnostic|output-v2-preview|presentation_findings|assert_required_properties|CheckOutput::complete|CheckOutput::partial|CheckOutput::not_evaluated|CheckOutput::complete_scoped' \
  -- ':!scripts/check-schema-id.sh' || true)
if [ -n "$legacy" ]; then
  fail "removed v1/preview API or format remains:\n$legacy"
fi

contract_duplicates=$(git grep -nE \
  'struct (EnvelopeHeader|ReportEnvelope|(Measure|Lint)?FileReport|MeasurementContract)|serde_json::Value' \
  -- crates/animsmith/src/main.rs || true)
if [ -n "$contract_duplicates" ]; then
  fail "CLI reimplements shared contract structure instead of consuming animsmith-core:\n$contract_duplicates"
fi

legacy_envelope_awk='
  function brace_delta(text, i, ch, escaped, in_string, delta) {
    escaped = 0
    in_string = 0
    delta = 0
    for (i = 1; i <= length(text); i++) {
      ch = substr(text, i, 1)
      if (in_string) {
        if (escaped) {
          escaped = 0
        } else if (ch == "\\") {
          escaped = 1
        } else if (ch == "\"") {
          in_string = 0
        }
      } else if (ch == "\"") {
        in_string = 1
      } else if (ch == "{") {
        delta++
      } else if (ch == "}") {
        delta--
      }
    }
    return delta
  }
  BEGIN {
    depth = 0
  }
  {
    if (match($0, /"schema_version"[[:space:]]*:[[:space:]]*1([[:space:]]*[,}]|[[:space:]]*$)/)) {
      field_depth = depth + brace_delta(substr($0, 1, RSTART - 1))
      version_line[field_depth] = NR
      version_text[field_depth] = $0
    }
    if (match($0, /"command"[[:space:]]*:/)) {
      field_depth = depth + brace_delta(substr($0, 1, RSTART - 1))
      command_line[field_depth] = NR
    }
    for (candidate_depth in version_line) {
      if ((candidate_depth in command_line) && !(candidate_depth in reported)) {
        print FILENAME ":" version_line[candidate_depth] ":" version_text[candidate_depth]
        reported[candidate_depth] = 1
      }
    }
    depth += brace_delta($0)
    for (candidate_depth in version_line) {
      if (candidate_depth > depth) {
        delete version_line[candidate_depth]
        delete version_text[candidate_depth]
        delete reported[candidate_depth]
      }
    }
    for (candidate_depth in command_line) {
      if (candidate_depth > depth) {
        delete command_line[candidate_depth]
      }
    }
  }
'

# Pin the scanner against a normal outer envelope whose schema/tool fields sit
# between its version and command. Also prove that nested measurements v1 in a
# current output-v2 envelope are not mistaken for an outer legacy contract.
legacy_scanner_regression=$(
  printf '%s\n' \
    '{' \
    '  "schema_version": 1,' \
    '  "schema": "urn:animsmith:schema:output:1",' \
    '  "tool": { "name": "animsmith" },' \
    '  "command": "lint"' \
    '}' \
    | awk "$legacy_envelope_awk"
)
if [ -z "$legacy_scanner_regression" ]; then
  fail "legacy-envelope scanner missed its non-adjacent command regression fixture"
fi

legacy_reverse_order_regression=$(
  printf '%s\n' \
    '{' \
    '  "command": "lint",' \
    '  "schema": "urn:animsmith:schema:output:1",' \
    '  "files": [],' \
    '  "schema_version": 1' \
    '}' \
    | awk "$legacy_envelope_awk"
)
if [ -z "$legacy_reverse_order_regression" ]; then
  fail "legacy-envelope scanner missed its command-first, version-last regression fixture"
fi

modern_scanner_regression=$(
  printf '%s\n' \
    '{' \
    '  "schema_version": 2,' \
    '  "command": "measure",' \
    '  "files": [{ "measurements": {' \
    '    "schema_version": 1,' \
    '    "schema": "urn:animsmith:schema:measurements:1"' \
    '  }}]' \
    '}' \
    | awk "$legacy_envelope_awk"
)
if [ -n "$modern_scanner_regression" ]; then
  fail "legacy-envelope scanner misclassified nested measurements v1"
fi

legacy_envelope=$(
  while IFS= read -r file; do
    awk "$legacy_envelope_awk" "$file"
  done < <(git grep -lE \
    '"schema_version"[[:space:]]*:[[:space:]]*1([[:space:]]*[,}]|[[:space:]]*$)' \
    -- ':!scripts/check-schema-id.sh' || true)
)
if [ -n "$legacy_envelope" ]; then
  fail "removed outer output-v1 envelope remains:\n$legacy_envelope"
fi

if [ "$failures" -ne 0 ]; then
  exit 1
fi
