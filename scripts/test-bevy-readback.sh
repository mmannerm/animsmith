#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"
work="$(mktemp -d "${TMPDIR:-/tmp}/animsmith-bevy-readback.XXXXXX")"
trap 'rm -rf "$work"' EXIT
snapshot_tmp="$work/snapshots"
mkdir "$snapshot_tmp"
export TMPDIR="$snapshot_tmp"
cargo build -p animsmith --bin animsmith >/dev/null
cargo +1.95.0 build --manifest-path tools/bevy-readback/Cargo.toml --features test-support >/dev/null
cli=target/debug/animsmith
probe=tools/bevy-readback/target/debug/animsmith-bevy-readback
config=examples/bevy-v3.animsmith.toml
predict() { "$cli" --config "$config" generate addressability --target-pointer-width 64 "$1" > "$2"; }
expect() { local want="$1" got; shift; set +e; "$@" > /dev/null; got=$?; set -e; test "$got" = "$want" || { echo "expected exit $want, got $got" >&2; exit 1; }; }
readback_status() { local want="$1" output="$2" got; shift 2; set +e; "$@" > "$output"; got=$?; set -e; test "$got" = "$want" || { echo "expected exit $want, got $got" >&2; exit 1; }; }
cp examples/assets/clip.glb "$work/fixture.glb"
predict "$work/fixture.glb" "$work/glb.json"
mkfifo "$work/prediction.fifo"
readback_status 2 "$work/fifo.stdout" timeout 2 "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/prediction.fifo"
test ! -s "$work/fifo.stdout"
"$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json" > "$work/glb.readback.json"
jq -e '.conformance.state == "exact" and .harness.rust_toolchain == "rustc 1.95.0 (59807616e 2026-04-14)" and .harness.bevy_animation_feature == true and .harness.load_animations == true and .observation.terminal.state == "loaded" and (.harness.updates >= 1 and .harness.updates <= 4096) and .observation.primary_verified == true and .observation.dependencies_verified == true and .observation.default_scene == 0 and (.observation.nodes | length) == 2 and (.observation.skins | length) == 0 and (.observation.inverse_bind_matrices | length) == 0 and (.observation.targets | length) == 1 and (.observation.warnings | length) >= 1 and (.observation.warnings_truncated == false)' "$work/glb.readback.json" >/dev/null
cp examples/assets/clip.glb "$work/race.glb"
predict "$work/race.glb" "$work/race.json"
ANIMSMITH_BEVY_READBACK_TEST_MUTATE_ORIGINAL_AFTER_SNAPSHOT=1 "$probe" --asset-root "$work" --asset race.glb --prediction "$work/race.json" > "$work/race.readback.json"
jq -e '.conformance.state == "exact" and .observation.primary_verified == true and .observation.dependencies_verified == true' "$work/race.readback.json" >/dev/null
test "$(wc -c < "$work/race.glb")" -gt "$(jq -r '.input.bytes' "$work/race.readback.json")"
readback_status 1 "$work/post-observe-mutation.stdout" env ANIMSMITH_BEVY_READBACK_TEST_MUTATE_SNAPSHOT_AFTER_OBSERVE=1 "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json"
test ! -s "$work/post-observe-mutation.stdout"
test -z "$(find "$snapshot_tmp" -maxdepth 1 -type d -name '.animsmith-bevy-readback-*' -print -quit)"
jq '.animations += [.animations[0], .animations[0]] | .animations[1] |= del(.name) | .animations[2].name = .animations[0].name | .scenes[0].name = "SceneName"' crates/animsmith-gltf/testdata/rig.gltf > "$work/fixture.gltf"
predict "$work/fixture.gltf" "$work/gltf.json"
"$probe" --asset-root "$work" --asset fixture.gltf --prediction "$work/gltf.json" > "$work/gltf.readback.json"
jq -e '.conformance.state == "exact" and .observation.default_scene == 0 and (.observation.nodes | length) == 3 and (.observation.skins | length) == 0 and (.observation.inverse_bind_matrices | length) == 0 and (.observation.animations | length) == 3 and .observation.named_animation_winners[0].index == 2 and .observation.named_scene_winners[0].index == 0' "$work/gltf.readback.json" >/dev/null
printf '%s' '{"asset":{"version":"2.0"},"nodes":[{"name":"joint"},{"name":"first_attachment","skin":2},{"name":"second_attachment","skin":1}],"skins":[{"name":"unattached","joints":[0]},{"name":"duplicate","joints":[0]},{"name":"duplicate","joints":[0]}],"scenes":[{"nodes":[0,1,2]}],"scene":0}' > "$work/attached-skins.gltf"
predict "$work/attached-skins.gltf" "$work/attached-skins.json"
"$probe" --asset-root "$work" --asset attached-skins.gltf --prediction "$work/attached-skins.json" > "$work/attached-skins.readback.json"
jq -e '.conformance.state == "exact" and [.observation.skins[].index] == [1,2] and [.observation.skins[].label] == ["Skin1","Skin2"] and [.observation.inverse_bind_matrices[].index] == [0,1,2] and [.observation.inverse_bind_matrices[].label] == ["Skin0/InverseBindMatrices","Skin1/InverseBindMatrices","Skin2/InverseBindMatrices"] and .observation.named_skin_winners == [{"name":"duplicate","index":1}]' "$work/attached-skins.readback.json" >/dev/null
printf '%s' '{"asset":{"version":"2.0"},"nodes":[{"name":"joint"}],"skins":[{"name":"unattached","joints":[0]}],"scenes":[{"nodes":[0]}],"scene":0}' > "$work/unattached-skin.gltf"
predict "$work/unattached-skin.gltf" "$work/unattached-skin.json"
"$probe" --asset-root "$work" --asset unattached-skin.gltf --prediction "$work/unattached-skin.json" > "$work/unattached-skin.readback.json"
jq -e '.conformance.state == "exact" and (.observation.skins | length) == 0 and .observation.inverse_bind_matrices == [{"index":0,"label":"Skin0/InverseBindMatrices"}] and (.observation.named_skin_winners | length) == 0' "$work/unattached-skin.readback.json" >/dev/null
uri="$(jq -r '.buffers[0].uri' crates/animsmith-gltf/testdata/rig.gltf)"
printf '%s' "${uri#*,}" | base64 -d > "$work/buffer.bin"
jq '.buffers[0].uri = "buffer.bin"' crates/animsmith-gltf/testdata/rig.gltf > "$work/dependency.gltf"
predict "$work/dependency.gltf" "$work/dependency.json"
ANIMSMITH_BEVY_READBACK_TEST_MUTATE_ORIGINAL_DEPENDENCY_AFTER_SNAPSHOT=buffer.bin "$probe" --asset-root "$work" --asset dependency.gltf --prediction "$work/dependency.json" > "$work/dependency-race.readback.json"
jq -e '.conformance.state == "exact" and .observation.primary_verified == true and .observation.dependencies_verified == true' "$work/dependency-race.readback.json" >/dev/null
printf '%s' "${uri#*,}" | base64 -d > "$work/buffer.bin"
printf 'self-authored secondary resource' > "$work/image.bin"
jq '.buffers[0].uri = "buffer.bin" | .images = [{"uri":"image.bin"}]' crates/animsmith-gltf/testdata/rig.gltf > "$work/multi-dependency.gltf"
predict "$work/multi-dependency.gltf" "$work/multi-dependency.json"
printf x >> "$work/image.bin"
expect 1 "$probe" --asset-root "$work" --asset multi-dependency.gltf --prediction "$work/multi-dependency.json"
rm "$work/buffer.bin"
expect 1 "$probe" --asset-root "$work" --asset dependency.gltf --prediction "$work/dependency.json"
printf x >> "$work/fixture.glb"
expect 1 "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json"
cp examples/assets/clip.glb "$work/fixture.glb"
readback_status 1 "$work/root-failure.readback.json" env ANIMSMITH_BEVY_READBACK_TEST_MISSING_ROOT_LABEL=1 "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json"
jq -e '.conformance.state == "not_exact" and (.conformance.mismatch_codes | index("load_did_not_succeed")) and .observation.terminal.state == "root_failure" and .observation.terminal.error == "missing_label" and (.harness.updates >= 1 and .harness.updates <= 4096) and .observation.primary_verified == true and .observation.dependencies_verified == true' "$work/root-failure.readback.json" >/dev/null
printf '%s' '{"asset":{"version":"2.0"},"images":[{"uri":"unsupported.png"}],"textures":[{"source":0}]}' > "$work/dependency-failure.gltf"
printf 'not an image, but present and identity-verified' > "$work/unsupported.png"
predict "$work/dependency-failure.gltf" "$work/dependency-failure.json"
readback_status 1 "$work/dependency-failure.readback.json" "$probe" --asset-root "$work" --asset dependency-failure.gltf --prediction "$work/dependency-failure.json"
jq -e '.conformance.state == "not_exact" and (.conformance.mismatch_codes | index("load_did_not_succeed")) and .observation.terminal.state == "dependency_failure" and .observation.terminal.error == "missing_asset_loader" and (.harness.updates >= 1 and .harness.updates <= 4096) and .observation.primary_verified == true and .observation.dependencies_verified == true' "$work/dependency-failure.readback.json" >/dev/null
readback_status 1 "$work/work-limit.readback.json" env ANIMSMITH_BEVY_READBACK_TEST_MAX_UPDATES=0 "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json"
jq -e '.conformance.state == "not_exact" and (.conformance.mismatch_codes | index("load_did_not_succeed")) and .observation.terminal.state == "work_limit" and .harness.updates == 0 and .observation.primary_verified == true and .observation.dependencies_verified == true' "$work/work-limit.readback.json" >/dev/null

for facet_and_code in inventory:inventory_mismatch scene:scene_mismatch default_scene:default_scene_mismatch target:target_mismatch; do
    facet="${facet_and_code%%:*}"
    code="${facet_and_code#*:}"
    readback_status 1 "$work/$facet-mismatch.readback.json" env ANIMSMITH_BEVY_READBACK_TEST_MUTATE_OBSERVATION="$facet" "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json"
    jq -e --arg code "$code" '.conformance.state == "not_exact" and (.conformance.mismatch_codes | index($code))' "$work/$facet-mismatch.readback.json" >/dev/null
done
readback_status 1 "$work/named-mismatch.readback.json" env ANIMSMITH_BEVY_READBACK_TEST_MUTATE_OBSERVATION=named "$probe" --asset-root "$work" --asset fixture.gltf --prediction "$work/gltf.json"
jq -e '.conformance.state == "not_exact" and (.conformance.mismatch_codes | index("named_winner_mismatch"))' "$work/named-mismatch.readback.json" >/dev/null
readback_status 1 "$work/skin-mismatch.readback.json" env ANIMSMITH_BEVY_READBACK_TEST_MUTATE_OBSERVATION=skin "$probe" --asset-root "$work" --asset attached-skins.gltf --prediction "$work/attached-skins.json"
jq -e '.conformance.state == "not_exact" and (.conformance.mismatch_codes | index("skin_mismatch"))' "$work/skin-mismatch.readback.json" >/dev/null
for reference_and_code in prediction_document:prediction_document_mismatch provenance:provenance_mismatch; do
    reference="${reference_and_code%%:*}"
    code="${reference_and_code#*:}"
    readback_status 1 "$work/$reference-mismatch.readback.json" env ANIMSMITH_BEVY_READBACK_TEST_REFERENCE_MISMATCH="$reference" "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json"
    jq -e --arg code "$code" '.conformance.state == "not_exact" and (.conformance.mismatch_codes | index($code))' "$work/$reference-mismatch.readback.json" >/dev/null
done
cp examples/bevy-v3.animsmith.toml "$work/unavailable.toml"
sed -i 's/bevy_animation_feature = true/bevy_animation_feature = false/' "$work/unavailable.toml"
"$cli" --config "$work/unavailable.toml" generate addressability --target-pointer-width 64 "$work/fixture.glb" > "$work/unavailable.json" || true
readback_status 1 "$work/settings-mismatch.readback.json" "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/unavailable.json"
jq -e '.conformance.state == "not_exact" and (.conformance.mismatch_codes | index("settings_mismatch")) and (.conformance.unavailable_codes | index("required_prediction_unavailable"))' "$work/settings-mismatch.readback.json" >/dev/null
test -z "$(find "$snapshot_tmp" -maxdepth 1 -type d -name '.animsmith-bevy-readback-*' -print -quit)"
echo "bevy-readback integration matrix passed"
