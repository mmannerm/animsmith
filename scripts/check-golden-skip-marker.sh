#!/usr/bin/env bash
set -euo pipefail

marker="ANIMSMITH_GOLDEN_SKIP: set ANIMSMITH_GOLDEN_GLB to run the golden test"
unset ANIMSMITH_GOLDEN_GLB

if ! output="$(cargo test -p animsmith-gltf --test golden -- --nocapture 2>&1)"; then
    printf '%s\n' "$output"
    exit 1
fi

printf '%s\n' "$output"
if ! grep -Fq "$marker" <<<"$output"; then
    printf 'missing golden skip marker: %s\n' "$marker" >&2
    exit 1
fi

retired_fbx_suffix="MESH_FBX"
retired_fbx_gate="ANIMSMITH_${retired_fbx_suffix}"
if git grep -n "$retired_fbx_gate" -- .; then
    printf 'retired env-gated FBX asset path is still referenced: %s\n' "$retired_fbx_gate" >&2
    exit 1
fi
if git grep -n "$retired_fbx_suffix" -- . ':!scripts/check-golden-skip-marker.sh'; then
    printf 'retired env-gated FBX asset fragment is still referenced: %s\n' "$retired_fbx_suffix" >&2
    exit 1
fi
