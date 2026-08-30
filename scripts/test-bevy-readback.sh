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
jq -e '.conformance.state == "exact" and .observation.terminal.state == "loaded" and .observation.default_scene == 0 and (.observation.nodes | length) == 2 and (.observation.skins | length) == 0 and (.observation.inverse_bind_matrices | length) == 0 and (.observation.targets | length) == 1 and (.observation.warnings | length) >= 1 and (.observation.warnings_truncated == false)' "$work/glb.readback.json" >/dev/null
jq '.animations += [.animations[0], .animations[0]] | .animations[1] |= del(.name) | .animations[2].name = .animations[0].name | .scenes[0].name = "SceneName"' crates/animsmith-gltf/testdata/rig.gltf > "$work/fixture.gltf"
predict "$work/fixture.gltf" "$work/gltf.json"
"$probe" --asset-root "$work" --asset fixture.gltf --prediction "$work/gltf.json" > "$work/gltf.readback.json"
jq -e '.conformance.state == "exact" and .observation.default_scene == 0 and (.observation.nodes | length) == 3 and (.observation.skins | length) == 0 and (.observation.inverse_bind_matrices | length) == 0 and (.observation.animations | length) == 3 and .observation.named_animation_winners[0].index == 2 and .observation.named_scene_winners[0].index == 0' "$work/gltf.readback.json" >/dev/null
uri="$(jq -r '.buffers[0].uri' crates/animsmith-gltf/testdata/rig.gltf)"
printf '%s' "${uri#*,}" | base64 -d > "$work/buffer.bin"
jq '.buffers[0].uri = "buffer.bin"' crates/animsmith-gltf/testdata/rig.gltf > "$work/dependency.gltf"
predict "$work/dependency.gltf" "$work/dependency.json"
rm "$work/buffer.bin"
expect 1 "$probe" --asset-root "$work" --asset dependency.gltf --prediction "$work/dependency.json"
printf x >> "$work/fixture.glb"
expect 1 "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json"
cp examples/assets/clip.glb "$work/fixture.glb"
ANIMSMITH_BEVY_READBACK_TEST_MAX_UPDATES=0 expect 1 "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/glb.json"
cp examples/bevy-v3.animsmith.toml "$work/unavailable.toml"
sed -i 's/bevy_animation_feature = true/bevy_animation_feature = false/' "$work/unavailable.toml"
"$cli" --config "$work/unavailable.toml" generate addressability --target-pointer-width 64 "$work/fixture.glb" > "$work/unavailable.json" || true
expect 1 "$probe" --asset-root "$work" --asset fixture.glb --prediction "$work/unavailable.json"
echo "bevy-readback integration matrix passed"
