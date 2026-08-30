#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"
work="$(mktemp -d "${TMPDIR:-/tmp}/animsmith-bevy-readback.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cargo build -p animsmith --bin animsmith >/dev/null
cargo +1.95.0 build --manifest-path tools/bevy-readback/Cargo.toml --features test-support >/dev/null
cli=target/debug/animsmith
probe=tools/bevy-readback/target/debug/animsmith-bevy-readback
config=examples/bevy-v3.animsmith.toml
predict() { "$cli" --config "$config" generate addressability --target-pointer-width 64 "$1" > "$2"; }
expect() { local want="$1" got; shift; set +e; "$@" > /dev/null; got=$?; set -e; test "$got" = "$want" || { echo "expected exit $want, got $got" >&2; exit 1; }; }
cp examples/assets/clip.glb "$work/fixture.glb"
predict "$work/fixture.glb" "$work/glb.json"
"$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json" > "$work/glb.readback.json"
jq -e '.conformance.state == "exact" and .observation.terminal.state == "loaded" and (.harness.updates >= 1 and .harness.updates <= 4096) and .observation.default_scene == 0 and (.observation.nodes | length) == 2 and (.observation.skins | length) == 0 and (.observation.inverse_bind_matrices | length) == 0 and (.observation.targets | length) == 1 and (.observation.warnings | length) >= 1 and (.observation.warnings_truncated == false)' "$work/glb.readback.json" >/dev/null
jq '.animations += [.animations[0], .animations[0]] | .animations[1] |= del(.name) | .animations[2].name = .animations[0].name | .scenes[0].name = "SceneName"' crates/animsmith-gltf/testdata/rig.gltf > "$work/fixture.gltf"
predict "$work/fixture.gltf" "$work/gltf.json"
"$probe" --asset-root "$work" --asset fixture.gltf --prediction "$work/gltf.json" > "$work/gltf.readback.json"
jq -e '.conformance.state == "exact" and .observation.default_scene == 0 and (.observation.nodes | length) == 3 and (.observation.skins | length) == 0 and (.observation.inverse_bind_matrices | length) == 0 and (.observation.animations | length) == 3 and .observation.named_animation_winners[0].index == 2 and .observation.named_scene_winners[0].index == 0' "$work/gltf.readback.json" >/dev/null
printf '%s' '{"asset":{"version":"2.0"},"nodes":[{"name":"joint","skin":0}],"skins":[{"name":"attached","joints":[0]}],"scenes":[{"nodes":[0]}],"scene":0}' > "$work/attached-skin.gltf"
predict "$work/attached-skin.gltf" "$work/attached-skin.json"
"$probe" --asset-root "$work" --asset attached-skin.gltf --prediction "$work/attached-skin.json" > "$work/attached-skin.readback.json"
jq -e '.conformance.state == "exact" and (.observation.skins | length) == 1 and .observation.skins[0].label == "Skin0" and (.observation.inverse_bind_matrices | length) == 1 and .observation.inverse_bind_matrices[0].label == "Skin0/InverseBindMatrices" and .observation.named_skin_winners[0].index == 0' "$work/attached-skin.readback.json" >/dev/null
printf '%s' '{"asset":{"version":"2.0"},"nodes":[{"name":"joint"}],"skins":[{"name":"unattached","joints":[0]}],"scenes":[{"nodes":[0]}],"scene":0}' > "$work/unattached-skin.gltf"
predict "$work/unattached-skin.gltf" "$work/unattached-skin.json"
set +e
"$probe" --asset-root "$work" --asset unattached-skin.gltf --prediction "$work/unattached-skin.json" > "$work/unattached-skin.readback.json"
unattached_skin_status=$?
set -e
test "$unattached_skin_status" = 1
jq -e '.conformance.state == "not_exact" and (.conformance.mismatch_codes | index("skin_mismatch")) and (.observation.skins | length) == 0 and (.observation.inverse_bind_matrices | length) == 0' "$work/unattached-skin.readback.json" >/dev/null
uri="$(jq -r '.buffers[0].uri' crates/animsmith-gltf/testdata/rig.gltf)"
printf '%s' "${uri#*,}" | base64 -d > "$work/buffer.bin"
jq '.buffers[0].uri = "buffer.bin"' crates/animsmith-gltf/testdata/rig.gltf > "$work/dependency.gltf"
predict "$work/dependency.gltf" "$work/dependency.json"
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
set +e
ANIMSMITH_BEVY_READBACK_TEST_MAX_UPDATES=0 "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json" > "$work/work-limit.readback.json"
work_limit_status=$?
set -e
test "$work_limit_status" = 1
jq -e '.conformance.state == "not_exact" and .observation.terminal.state == "work_limit" and .harness.updates == 0' "$work/work-limit.readback.json" >/dev/null
cp examples/bevy-v3.animsmith.toml "$work/unavailable.toml"
sed -i 's/bevy_animation_feature = true/bevy_animation_feature = false/' "$work/unavailable.toml"
"$cli" --config "$work/unavailable.toml" generate addressability --target-pointer-width 64 "$work/fixture.glb" > "$work/unavailable.json" || true
expect 1 "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/unavailable.json"
echo "bevy-readback integration matrix passed"
