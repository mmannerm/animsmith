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
  shift 2
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
  for reference in "$@"; do
    if ! grep -Fq "$expected" "$reference"; then
      fail "$reference does not reference schema identity $expected"
    fi
  done
}

check_schema docs/schemas/output-v2.schema.json urn:animsmith:schema:output:2 crates/animsmith-core/src/contract.rs docs/output.md
check_schema docs/schemas/measurements-v2.schema.json urn:animsmith:schema:measurements:2 crates/animsmith-core/src/contract.rs docs/output.md
check_schema docs/schemas/conversion-evidence-v1.schema.json urn:animsmith:schema:conversion-evidence:1 docs/output.md
check_schema docs/schemas/conversion-evidence-v2.schema.json urn:animsmith:schema:conversion-evidence:2 docs/output.md docs/cli.md

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

contract_duplicate_pattern='struct[[:space:]]+(EnvelopeHeader|ReportEnvelope|(Measure|Lint)?FileReport|MeasurementContract)([^[:alnum:]_]|$)|serde_json[[:space:]]*::[[:space:]]*(Value|\{[^}]*Value)([^[:alnum:]_]|$)'
contract_duplicates=$(git grep -nE \
  "$contract_duplicate_pattern" \
  -- crates/animsmith/src || true)
if [ -n "$contract_duplicates" ]; then
  fail "CLI reimplements shared contract structure instead of consuming animsmith-core:\n$contract_duplicates"
fi

if ! printf '%s\n' 'struct  EnvelopeHeader {' | grep -Eq "$contract_duplicate_pattern"; then
  fail "CLI contract-duplication guard missed flexible struct whitespace"
fi
if ! printf '%s\n' 'use serde_json::{json, Value};' | grep -Eq "$contract_duplicate_pattern"; then
  fail "CLI contract-duplication guard missed a grouped serde_json::Value import"
fi
if printf '%s\n' 'struct EnvelopeHeaderExtra {' 'serde_json::ValueError' \
  | grep -Eq "$contract_duplicate_pattern"; then
  fail "CLI contract-duplication guard matched a longer, unrelated identifier"
fi

legacy_envelope_awk='
  function clear_object(object_depth) {
    delete version_seen[object_depth]
    delete version_line[object_depth]
    delete version_text[object_depth]
    delete command_seen[object_depth]
    delete reported[object_depth]
  }
  function maybe_report(object_depth) {
    if (version_seen[object_depth] && command_seen[object_depth] &&
        !reported[object_depth]) {
      print FILENAME ":" version_line[object_depth] ":" version_text[object_depth]
      reported[object_depth] = 1
    }
  }
  function mark_version(object_depth) {
    if (object_depth <= 0) {
      return
    }
    version_seen[object_depth] = 1
    version_line[object_depth] = NR
    version_text[object_depth] = $0
    maybe_report(object_depth)
  }
  function mark_command(object_depth) {
    if (object_depth <= 0) {
      return
    }
    command_seen[object_depth] = 1
    maybe_report(object_depth)
  }
  BEGIN {
    depth = 0
    in_string = 0
    escaped = 0
    closed_string = 0
    awaiting_version_depth = 0
    awaiting_command_depth = 0
  }
  {
    # JSON strings cannot contain a raw newline. Candidate files are prose, so
    # an unmatched quote on an earlier line must not poison every later JSON
    # example in the file. Preserve completed keys and pending values, which
    # may legally be separated from their colon/value by whitespace lines.
    if (in_string) {
      in_string = 0
      escaped = 0
      token = ""
    }
    for (i = 1; i <= length($0); i++) {
      ch = substr($0, i, 1)

      if (in_string) {
        if (escaped) {
          token = token ch
          escaped = 0
        } else if (ch == "\\") {
          token = token ch
          escaped = 1
        } else if (ch == "\"") {
          in_string = 0
          if (awaiting_command_depth > 0) {
            # Only the retired output envelope commands are legacy
            # candidates. Other independently versioned JSON contracts may
            # legitimately use schema_version 1.
            if (token == "measure" || token == "lint" || token == "diff") {
              mark_command(awaiting_command_depth)
            }
            awaiting_command_depth = 0
            closed_string = 0
          } else {
            closed_string = 1
          }
        } else {
          token = token ch
        }
        continue
      }

      if (closed_string) {
        if (ch ~ /[[:space:]]/) {
          continue
        }
        if (ch == ":") {
          if (token == "schema_version") {
            awaiting_version_depth = depth
          } else if (token == "command") {
            awaiting_command_depth = depth
          }
          closed_string = 0
          continue
        }
        closed_string = 0
      }

      if (awaiting_version_depth > 0) {
        if (ch ~ /[[:space:]]/) {
          continue
        }
        if (ch == "1" && substr($0, i + 1) ~ /^[[:space:]]*([,}]|$)/) {
          mark_version(awaiting_version_depth)
        }
        awaiting_version_depth = 0
      }

      if (ch == "\"") {
        in_string = 1
        escaped = 0
        token = ""
      } else if (ch == "{") {
        depth++
        clear_object(depth)
      } else if (ch == "}") {
        clear_object(depth)
        if (depth > 0) {
          depth--
        }
      }
    }
  }
'

# Candidate selection deliberately knows only the field name. The object-aware
# scanner above owns value, whitespace, field-order, and nesting semantics.
legacy_candidate_pattern='"schema_version"'

# Pin the scanner against a normal outer envelope whose schema/tool fields sit
# between its version and command. Also prove that nested measurements v2 in a
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

legacy_multiline_fixture=$(
  printf '%s\n' \
    '{' \
    '  "schema_version"' \
    '    :' \
    '    1,' \
    '  "command": "lint"' \
    '}'
)
legacy_multiline_regression=""
if printf '%s\n' "$legacy_multiline_fixture" | grep -Fq "$legacy_candidate_pattern"; then
  legacy_multiline_regression=$(
    printf '%s\n' "$legacy_multiline_fixture" | awk "$legacy_envelope_awk"
  )
fi
if [ -z "$legacy_multiline_regression" ]; then
  fail "legacy-envelope candidate/scanner pipeline missed its multiline value fixture"
fi

legacy_quote_poison_regression=$(
  printf '%s\n' \
    'prose with an unmatched "' \
    '{' \
    '  "schema_version": 1,' \
    '  "command": "lint"' \
    '}' \
    | awk "$legacy_envelope_awk"
)
if [ -z "$legacy_quote_poison_regression" ]; then
  fail "legacy-envelope scanner let an earlier unmatched quote poison later examples"
fi

legacy_minified_regression=$(
  printf '%s\n' \
    '{"files":[{"measurements":{"schema_version":1}}],"note":"} {","schema_version":1,"command":"lint"}' \
    | awk "$legacy_envelope_awk"
)
if [ -z "$legacy_minified_regression" ]; then
  fail "legacy-envelope scanner missed its minified nested-version regression fixture"
fi

sibling_scanner_regression=$(
  printf '%s\n' \
    '[{"schema_version":1},{"command":"lint"}]' \
    | awk "$legacy_envelope_awk"
)
if [ -n "$sibling_scanner_regression" ]; then
  fail "legacy-envelope scanner combined fields from sibling objects"
fi

modern_scanner_regression=$(
  printf '%s\n' \
    '{' \
    '  "schema_version": 2,' \
    '  "command": "measure",' \
    '  "files": [{ "measurements": {' \
    '    "schema_version": 2,' \
    '    "schema": "urn:animsmith:schema:measurements:2"' \
    '  }}]' \
    '}' \
    | awk "$legacy_envelope_awk"
)
if [ -n "$modern_scanner_regression" ]; then
  fail "legacy-envelope scanner misclassified nested measurements v2"
fi

legacy_envelope=$(
  while IFS= read -r file; do
    awk "$legacy_envelope_awk" "$file"
  done < <(git grep -lF \
    "$legacy_candidate_pattern" \
    -- ':!scripts/check-schema-id.sh' || true)
)
if [ -n "$legacy_envelope" ]; then
  fail "removed outer output-v1 envelope remains:\n$legacy_envelope"
fi

if [ "$failures" -ne 0 ]; then
  exit 1
fi
