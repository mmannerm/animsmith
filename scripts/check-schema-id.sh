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

check_schema docs/schemas/output-v2.schema.json urn:animsmith:schema:output:2
check_schema docs/schemas/output-v3.schema.json urn:animsmith:schema:output:3
check_schema docs/schemas/output-v4.schema.json urn:animsmith:schema:output:4
check_schema docs/schemas/output-v5.schema.json urn:animsmith:schema:output:5
check_schema docs/schemas/output-v6.schema.json urn:animsmith:schema:output:6
check_schema docs/schemas/output-v7.schema.json urn:animsmith:schema:output:7
check_schema docs/schemas/output-v8.schema.json urn:animsmith:schema:output:8
check_schema docs/schemas/output-v9.schema.json urn:animsmith:schema:output:9
check_schema docs/schemas/output-v10.schema.json urn:animsmith:schema:output:10 crates/animsmith-core/src/contract.rs docs/output.md
check_schema docs/schemas/output-v11.schema.json urn:animsmith:schema:output:11 crates/animsmith-core/src/contract.rs docs/output.md
check_schema docs/schemas/measurements-v8.schema.json urn:animsmith:schema:measurements:8
check_schema docs/schemas/measurements-v9.schema.json urn:animsmith:schema:measurements:9
check_schema docs/schemas/measurements-v10.schema.json urn:animsmith:schema:measurements:10
check_schema docs/schemas/measurements-v11.schema.json urn:animsmith:schema:measurements:11 docs/schemas/output-v4.schema.json docs/schemas/output-v5.schema.json
check_schema docs/schemas/measurements-v12.schema.json urn:animsmith:schema:measurements:12
check_schema docs/schemas/measurements-v13.schema.json urn:animsmith:schema:measurements:13 docs/schemas/output-v7.schema.json
check_schema docs/schemas/measurements-v14.schema.json urn:animsmith:schema:measurements:14 docs/schemas/output-v8.schema.json
check_schema docs/schemas/measurements-v15.schema.json urn:animsmith:schema:measurements:15 crates/animsmith-core/src/contract.rs docs/schemas/output-v9.schema.json docs/schemas/output-v10.schema.json docs/output.md
for historical_output in docs/schemas/output-v4.schema.json docs/schemas/output-v5.schema.json; do
  jq -e --arg expected 'urn:animsmith:schema:measurements:11' \
    '.["$defs"].file_report.properties.measurements["$ref"] == $expected' \
    "$historical_output" >/dev/null || fail "$historical_output must retain its measurements-v11 reference"
done
jq -e --arg expected 'urn:animsmith:schema:measurements:12' \
  '.["$defs"].file_report.properties.measurements["$ref"] == $expected' \
  docs/schemas/output-v6.schema.json >/dev/null || fail 'docs/schemas/output-v6.schema.json must retain measurements-v12'
jq -e --arg expected 'urn:animsmith:schema:measurements:13' \
  '.["$defs"].file_report.properties.measurements["$ref"] == $expected' \
  docs/schemas/output-v7.schema.json >/dev/null || fail 'docs/schemas/output-v7.schema.json must reference measurements-v13'
jq -e --arg expected 'urn:animsmith:schema:measurements:14' \
  '.["$defs"].file_report.properties.measurements["$ref"] == $expected' \
  docs/schemas/output-v8.schema.json >/dev/null || fail 'docs/schemas/output-v8.schema.json must reference measurements-v14'
jq -e --arg expected 'urn:animsmith:schema:measurements:15' \
  '.["$defs"].file_report.properties.measurements["$ref"] == $expected' \
  docs/schemas/output-v9.schema.json >/dev/null || fail 'docs/schemas/output-v9.schema.json must reference measurements-v15'
jq -e --arg expected 'urn:animsmith:schema:measurements:15' '
  .["$defs"].measure_file_report.properties.measurements["$ref"] == $expected
  and .["$defs"].lint_file_report.properties.measurements["$ref"] == $expected
' docs/schemas/output-v10.schema.json >/dev/null \
  || fail 'docs/schemas/output-v10.schema.json measure and lint files must reference measurements-v15'
if ! jq -e '
  .["$defs"].prediction_provenance.properties.schema.const
    == "urn:animsmith:prediction-provenance:1"
  and .["$defs"].engine_prediction.properties.schema.const
    == "urn:animsmith:engine-prediction:1"
  and .["$defs"].resolved_engine_settings.properties.schema.const
    == "urn:animsmith:resolved-engine-settings:1"
  and .["$defs"].resolved_engine_profile.properties.schema.const
    == "urn:animsmith:engine-profile-facts:1"
  and .["$defs"].consumed_contracts.prefixItems
    == [
      {"const":"urn:animsmith:schema:output:10"},
      {"const":"urn:animsmith:schema:measurements:15"},
      {"const":"urn:animsmith:raw-source-facts:1"},
      {"const":"urn:animsmith:dependency-closure:1"},
      {"const":"urn:animsmith:engine-profile-facts:1"}
    ]
' docs/schemas/output-v10.schema.json >/dev/null; then
  fail 'output-v10 must retain all four adjunct identities and the exact five consumed contracts'
fi
if ! cmp -s docs/schemas/measurements-v11.schema.json <(
  sed \
    -e 's/urn:animsmith:schema:measurements:12/urn:animsmith:schema:measurements:11/g' \
    -e 's/animsmith measurements v12/animsmith measurements v11/' \
    -e 's/"const": 12/"const": 11/' \
    docs/schemas/measurements-v12.schema.json
); then
  fail 'measurements-v12 must differ from immutable measurements-v11 only by identity'
fi
if ! cmp -s docs/schemas/output-v5.schema.json <(
  sed \
    -e 's/urn:animsmith:schema:output:6/urn:animsmith:schema:output:5/g' \
    -e 's/animsmith output v6/animsmith output v5/' \
    -e 's/"const": 6/"const": 5/' \
    -e 's/urn:animsmith:schema:measurements:12/urn:animsmith:schema:measurements:11/g' \
    docs/schemas/output-v6.schema.json
); then
  fail 'output-v6 must differ from immutable output-v5 only by identity and nested measurement identity'
fi
check_schema docs/schemas/conversion-evidence-v1.schema.json urn:animsmith:schema:conversion-evidence:1 docs/output.md
check_schema docs/schemas/conversion-evidence-v2.schema.json urn:animsmith:schema:conversion-evidence:2 docs/output.md docs/cli.md
check_schema docs/schemas/producer-refusal-v1.schema.json urn:animsmith:schema:producer-refusal:1 crates/animsmith/src/producer.rs docs/output.md docs/cli.md
check_schema docs/schemas/scale-evidence-v1.schema.json urn:animsmith:schema:scale-evidence:1
check_schema docs/schemas/scale-evidence-v2.schema.json urn:animsmith:schema:scale-evidence:2
check_schema docs/schemas/scale-evidence-v3.schema.json urn:animsmith:schema:scale-evidence:3
check_schema docs/schemas/scale-evidence-v4.schema.json urn:animsmith:schema:scale-evidence:4 crates/animsmith/src/scale.rs docs/output.md docs/cli.md
check_schema docs/schemas/scale-evidence-v5.schema.json urn:animsmith:schema:scale-evidence:5 crates/animsmith/src/scale.rs docs/output.md docs/cli.md
check_schema docs/schemas/gltf-animation-addressability-v1.schema.json urn:animsmith:schema:gltf-animation-addressability:1 crates/animsmith-engine/src/addressability.rs docs/output.md docs/cli.md
check_schema docs/schemas/engine-import-advice-v1.schema.json urn:animsmith:schema:engine-import-advice:1 crates/animsmith-engine/src/import_advice.rs docs/output.md docs/cli.md
check_schema docs/schemas/collection-manifest-v1.schema.json urn:animsmith:schema:collection-manifest:1 crates/animsmith-core/src/collection.rs crates/animsmith/src/collection_manifest.rs DESIGN.md
check_schema docs/schemas/collection-output-v1.schema.json urn:animsmith:schema:collection-output:1 DESIGN.md
check_schema docs/schemas/collection-output-v2.schema.json urn:animsmith:schema:collection-output:2 crates/animsmith/src/collection_output.rs DESIGN.md docs/output.md docs/cli.md
check_schema docs/schemas/character-assembly-recipe-v2.schema.json urn:animsmith:schema:character-assembly-recipe:2
check_schema docs/schemas/character-assembly-recipe-v3.schema.json urn:animsmith:schema:character-assembly-recipe:3 crates/animsmith/src/assembly.rs
check_schema docs/schemas/character-assembly-recipe-v4.schema.json urn:animsmith:schema:character-assembly-recipe:4 crates/animsmith/src/assembly.rs
check_schema docs/schemas/character-assembly-recipe-v5.schema.json urn:animsmith:schema:character-assembly-recipe:5 crates/animsmith/src/assembly.rs
check_schema docs/schemas/character-assembly-recipe-v6.schema.json urn:animsmith:schema:character-assembly-recipe:6 crates/animsmith/src/assembly.rs
check_schema docs/schemas/character-assembly-recipe-v7.schema.json urn:animsmith:schema:character-assembly-recipe:7 crates/animsmith/src/assembly.rs docs/character-assembly.md docs/cli.md docs/output.md
check_schema docs/schemas/character-assembly-evidence-v2.schema.json urn:animsmith:schema:character-assembly-evidence:2
check_schema docs/schemas/character-assembly-evidence-v3.schema.json urn:animsmith:schema:character-assembly-evidence:3 crates/animsmith/src/assembly.rs
check_schema docs/schemas/character-assembly-evidence-v4.schema.json urn:animsmith:schema:character-assembly-evidence:4 crates/animsmith/src/assembly.rs
check_schema docs/schemas/character-assembly-evidence-v5.schema.json urn:animsmith:schema:character-assembly-evidence:5 crates/animsmith/src/assembly.rs
check_schema docs/schemas/character-assembly-evidence-v6.schema.json urn:animsmith:schema:character-assembly-evidence:6 crates/animsmith/src/assembly.rs
check_schema docs/schemas/character-assembly-evidence-v7.schema.json urn:animsmith:schema:character-assembly-evidence:7 crates/animsmith/src/assembly.rs docs/character-assembly.md docs/cli.md docs/output.md
if ! cmp -s docs/schemas/scale-evidence-v2.schema.json <(
  sed \
    -e 's/urn:animsmith:schema:scale-evidence:3/urn:animsmith:schema:scale-evidence:2/g' \
    -e 's/animsmith scale evidence v3/animsmith scale evidence v2/' \
    -e 's/"const": 3/"const": 2/' \
    -e 's/"rest_hierarchy", "translation_animation", "scale_animation", "inverse_binds", "base_mesh_positions"/"rest_hierarchy", "translation_animation", "inverse_binds", "base_mesh_positions"/' \
    -e '/"scale_animation": { "type": "boolean" },/d' \
    -e 's/, "animated_matrix_node"//' \
    docs/schemas/scale-evidence-v3.schema.json
); then
  fail 'scale-evidence-v3 must differ from immutable scale-evidence-v2 only by identity, scale-animation rewrite evidence, and animated-matrix-node'
fi
if ! cmp -s docs/schemas/scale-evidence-v3.schema.json <(
  sed \
    -e 's/urn:animsmith:schema:scale-evidence:4/urn:animsmith:schema:scale-evidence:3/g' \
    -e 's/animsmith scale evidence v4/animsmith scale evidence v3/' \
    -e 's/"const": 4/"const": 3/' \
    -e 's/, "morph_weight_locations"//' \
    -e '/"morph_weight_locations": { "\$ref": "#\/\$defs\/string_array" },/d' \
    docs/schemas/scale-evidence-v4.schema.json
); then
  fail 'scale-evidence-v4 must differ from immutable scale-evidence-v3 only by identity and the complete morph-weight capability inventory'
fi
if ! cmp -s docs/schemas/character-assembly-recipe-v1.schema.json <(
  sed \
    -e 's/urn:animsmith:schema:character-assembly-recipe:2/urn:animsmith:schema:character-assembly-recipe:1/g' \
    -e 's/animsmith character assembly recipe v2/animsmith character assembly recipe v1/' \
    -e 's/"const": 2/"const": 1/' \
    -e '/^    "prune_constant_tracks": {$/,/^    },$/d' \
    docs/schemas/character-assembly-recipe-v2.schema.json
); then
  fail 'character-assembly-recipe-v2 must differ from immutable character-assembly-recipe-v1 only by identity and prune_constant_tracks'
fi
if ! cmp -s docs/schemas/character-assembly-evidence-v1.schema.json <(
  sed \
    -e 's/urn:animsmith:schema:character-assembly-evidence:2/urn:animsmith:schema:character-assembly-evidence:1/g' \
    -e 's/animsmith character assembly evidence v2/animsmith character assembly evidence v1/' \
    -e 's/"const": 2/"const": 1/' \
    -e 's/, "pruned_constant_tracks"//' \
    -e '/"pruned_constant_tracks":/d' \
    -e '/^    "pruned_constant_track": {/,/^    },$/d' \
    docs/schemas/character-assembly-evidence-v2.schema.json
); then
  fail 'character-assembly-evidence-v2 must differ from immutable character-assembly-evidence-v1 only by identity and pruned_constant_tracks'
fi
if ! cmp -s docs/schemas/character-assembly-recipe-v2.schema.json <(
  sed \
    -e 's/urn:animsmith:schema:character-assembly-recipe:3/urn:animsmith:schema:character-assembly-recipe:2/g' \
    -e 's/animsmith character assembly recipe v3/animsmith character assembly recipe v2/' \
    -e 's/"const": 3/"const": 2/' \
    -e '/^    "remove_nodes": {$/,/^    },$/d' \
    docs/schemas/character-assembly-recipe-v3.schema.json
); then
  fail 'character-assembly-recipe-v3 must differ from immutable character-assembly-recipe-v2 only by identity and remove_nodes'
fi
if ! cmp -s docs/schemas/character-assembly-evidence-v2.schema.json <(
  sed \
    -e 's/urn:animsmith:schema:character-assembly-evidence:3/urn:animsmith:schema:character-assembly-evidence:2/g' \
    -e 's/animsmith character assembly evidence v3/animsmith character assembly evidence v2/' \
    -e 's/"const": 3/"const": 2/' \
    -e 's/, "removed_nodes"//' \
    -e '/"removed_nodes":/d' \
    -e '/^    "removed_node": {/,/^    },$/d' \
    -e '/        "base_index": {$/,/^        }$/c\        "base_index": { "type": "integer", "minimum": 0 }' \
    -e '/        "bone_index": {$/,/^        },$/c\        "bone_index": { "type": "integer", "minimum": 0 },' \
    docs/schemas/character-assembly-evidence-v3.schema.json
); then
  fail 'character-assembly-evidence-v3 must differ from immutable character-assembly-evidence-v2 only by identity, removed_nodes, and pre-removal index descriptions'
fi
if ! cmp -s <(jq -S . docs/schemas/character-assembly-recipe-v3.schema.json) <(
  jq -S '
    .["$id"] = "urn:animsmith:schema:character-assembly-recipe:3"
    | .title = "animsmith character assembly recipe v3"
    | .properties.schema_version.const = 3
    | .properties.schema.const = "urn:animsmith:schema:character-assembly-recipe:3"
    | del(.properties.rest_bind_scale, .["$defs"].rest_bind_scale)
  ' docs/schemas/character-assembly-recipe-v4.schema.json
); then
  fail 'character-assembly-recipe-v4 must differ from immutable character-assembly-recipe-v3 only by identity and rest_bind_scale'
fi
if ! cmp -s <(jq -S . docs/schemas/character-assembly-evidence-v3.schema.json) <(
  jq -S '
    .["$id"] = "urn:animsmith:schema:character-assembly-evidence:3"
    | .title = "animsmith character assembly evidence v3"
    | .properties.schema_version.const = 3
    | .properties.schema.const = "urn:animsmith:schema:character-assembly-evidence:3"
    | del(
        .properties.rest_bind_scale,
        .["$defs"].rest_bind_scale,
        .["$defs"].rest_bind_scale_input,
        .["$defs"].residual_comparison_counts,
        .["$defs"].shared_scale_evidence,
        .["$defs"].scale_tolerance,
        .["$defs"].scale_factors,
        .["$defs"].scale_affected,
        .["$defs"].scale_domain_rewrites,
        .["$defs"].scale_proof,
        .["$defs"].scale_artifact,
        .["$defs"].scale_artifact_proof,
        .["$defs"].scale_residuals,
        .["$defs"].scale_residual,
        .["$defs"].index,
        .["$defs"].index_array,
        .["$defs"].string_array
      )
  ' docs/schemas/character-assembly-evidence-v4.schema.json
); then
  fail 'character-assembly-evidence-v4 must differ from immutable character-assembly-evidence-v3 only by identity and rest_bind_scale evidence'
fi
if ! cmp -s <(jq -S . docs/schemas/character-assembly-recipe-v4.schema.json) <(
  jq -S '
    .["$id"] = "urn:animsmith:schema:character-assembly-recipe:4"
    | .title = "animsmith character assembly recipe v4"
    | .properties.schema_version.const = 4
    | .properties.schema.const = "urn:animsmith:schema:character-assembly-recipe:4"
  ' docs/schemas/character-assembly-recipe-v5.schema.json
); then
  fail 'character-assembly-recipe-v5 must differ from immutable recipe-v4 only by identity'
fi
if ! cmp -s <(jq -S . docs/schemas/character-assembly-evidence-v4.schema.json) <(
  jq -S '
    .["$id"] = "urn:animsmith:schema:character-assembly-evidence:4"
    | .title = "animsmith character assembly evidence v4"
    | .properties.schema_version.const = 4
    | .properties.schema.const = "urn:animsmith:schema:character-assembly-evidence:4"
    | del(
        .["$defs"].rest_bind_scale.properties.effective_source_skin_index,
        .["$defs"].rest_bind_scale.properties.effective_source_root_node_index
      )
    | .["$defs"].rest_bind_scale.required -= [
        "effective_source_skin_index",
        "effective_source_root_node_index"
      ]
  ' docs/schemas/character-assembly-evidence-v5.schema.json
); then
  fail 'character-assembly-evidence-v5 must differ from immutable evidence-v4 only by identity and effective staged selectors'
fi
if ! cmp -s <(jq -S . docs/schemas/character-assembly-recipe-v5.schema.json) <(
  jq -S '
    .["$id"] = "urn:animsmith:schema:character-assembly-recipe:5"
    | .title = "animsmith character assembly recipe v5"
    | .properties.schema_version.const = 5
    | .properties.schema.const = "urn:animsmith:schema:character-assembly-recipe:5"
  ' docs/schemas/character-assembly-recipe-v6.schema.json
); then
  fail 'character-assembly-recipe-v6 must differ from immutable recipe-v5 only by identity'
fi
if ! cmp -s <(jq -S . docs/schemas/character-assembly-evidence-v5.schema.json) <(
  jq -S '
    .["$id"] = "urn:animsmith:schema:character-assembly-evidence:5"
    | .title = "animsmith character assembly evidence v5"
    | .properties.schema_version.const = 5
    | .properties.schema.const = "urn:animsmith:schema:character-assembly-evidence:5"
    | .["$defs"].rest_bind_scale_input.required -= ["input_format", "source_projection"]
    | del(
        .["$defs"].rest_bind_scale_input.allOf,
        .["$defs"].rest_bind_scale_input.properties.input_format,
        .["$defs"].rest_bind_scale_input.properties.source_projection,
        .["$defs"].raw_gltf_projection,
        .["$defs"].normalized_baked_fbx_projection,
        .["$defs"].input_identity,
        .["$defs"].fbx_capability,
        .["$defs"].source_identity,
        .["$defs"].status,
        .["$defs"].count
      )
  ' docs/schemas/character-assembly-evidence-v6.schema.json
); then
  fail 'character-assembly-evidence-v6 must differ from immutable evidence-v5 only by identity and explicit source-projection evidence'
fi
if ! cmp -s <(jq -S . docs/schemas/character-assembly-recipe-v6.schema.json) <(
  jq -S --slurpfile old docs/schemas/character-assembly-recipe-v6.schema.json '
    .["$id"] = "urn:animsmith:schema:character-assembly-recipe:6"
    | .title = "animsmith character assembly recipe v6"
    | .properties.schema_version.const = 6
    | .properties.schema.const = "urn:animsmith:schema:character-assembly-recipe:6"
    | .["$defs"].rest_bind_scale.required = $old[0]["$defs"].rest_bind_scale.required
    | del(.["$defs"].rest_bind_scale.properties.root_node_name)
    | .["$defs"].rest_bind_scale.properties.source_skin_index = $old[0]["$defs"].rest_bind_scale.properties.source_skin_index
    | .["$defs"].rest_bind_scale.properties.source_root_node_index = $old[0]["$defs"].rest_bind_scale.properties.source_root_node_index
  ' docs/schemas/character-assembly-recipe-v7.schema.json
); then
  fail 'character-assembly-recipe-v7 must differ from immutable recipe-v6 only by identity and name selector'
fi
if ! cmp -s <(jq -S . docs/schemas/character-assembly-evidence-v6.schema.json) <(
  jq -S --slurpfile old docs/schemas/character-assembly-evidence-v6.schema.json '
    .["$id"] = "urn:animsmith:schema:character-assembly-evidence:6"
    | .title = "animsmith character assembly evidence v6"
    | .properties.schema_version.const = 6
    | .properties.schema.const = "urn:animsmith:schema:character-assembly-evidence:6"
    | .["$defs"].rest_bind_scale.required = $old[0]["$defs"].rest_bind_scale.required
    | del(.["$defs"].rest_bind_scale.properties.declared_root_node_name)
    | .["$defs"].rest_bind_scale.properties.source_skin_index = $old[0]["$defs"].rest_bind_scale.properties.source_skin_index
    | .["$defs"].rest_bind_scale.properties.source_root_node_index = $old[0]["$defs"].rest_bind_scale.properties.source_root_node_index
    | .["$defs"].rest_bind_scale_input.required = $old[0]["$defs"].rest_bind_scale_input.required
    | .["$defs"].rest_bind_scale_input.allOf = $old[0]["$defs"].rest_bind_scale_input.allOf
    | .["$defs"].rest_bind_scale_input.properties.basis_schema = $old[0]["$defs"].rest_bind_scale_input.properties.basis_schema
    | del(
        .["$defs"].rest_bind_scale_input.properties.application,
        .["$defs"].rest_bind_scale_input.properties.resolved_root_node_name,
        .["$defs"].rest_bind_scale_input.properties.resolved_source_skin_index,
        .["$defs"].rest_bind_scale_input.properties.resolved_source_root_node_index
      )
  ' docs/schemas/character-assembly-evidence-v7.schema.json
); then
  fail 'character-assembly-evidence-v7 must differ from immutable evidence-v6 only by identity, named selectors, and skinless clip track-rebase evidence'
fi
if ! jq -e '
  .["$defs"].residual_comparison_counts.required
    == .["$defs"].scale_residuals.required
  and (
    (.["$defs"].residual_comparison_counts.properties | keys)
      == (.["$defs"].scale_residuals.properties | keys)
  )
' docs/schemas/character-assembly-evidence-v4.schema.json >/dev/null; then
  fail 'character-assembly-evidence-v4 residual comparison counts must exactly pair every shared proof residual name'
fi
for basis_reference in \
  crates/animsmith/src/assembly.rs \
  docs/character-assembly.md \
  docs/output.md; do
  grep -Fq 'urn:animsmith:character-assembly-scale-basis:1' "$basis_reference" \
    || fail "$basis_reference does not reference the assembly scale basis fingerprint identity"
done

# Current-contract descriptions must not send readers back to the immutable
# output-v2 schema. Keep these exact statements aligned with the current outer
# contract when it advances.
grep -Fq 'Final output-v11 record for one catalog check.' crates/animsmith-core/src/evaluation.rs \
  || fail 'CheckEvaluation documentation does not identify output v11'
grep -Fq 'regenerate a current output-v11 report from the original' docs/output.md \
  || fail 'report migration documentation does not identify output v11'

for removed_schema in \
  docs/schemas/output-v1.schema.json \
  docs/schemas/output-v2-preview.schema.json; do
  if [ -e "$removed_schema" ]; then
    fail "$removed_schema is a removed alpha contract and must not be restored"
  fi
done

# Legacy API/name tombstone retained from the output-v2 cutover. Behavioural
# tests separately prove that non-current report inputs are rejected.
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
# between its version and command. Also prove that current nested measurements in a
# current output-v10 envelope are not mistaken for an outer legacy contract.
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
    '    "schema_version": 15,' \
    '    "schema": "urn:animsmith:schema:measurements:15"' \
    '  }}]' \
    '}' \
    | awk "$legacy_envelope_awk"
)
if [ -n "$modern_scanner_regression" ]; then
  fail "legacy-envelope scanner misclassified nested measurements v15"
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
