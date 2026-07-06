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

retired_fbx_gate='ANIMSMITH_''MESH_FBX'
if git grep -n "$retired_fbx_gate" -- .; then
    printf 'retired env-gated FBX asset path is still referenced: %s\n' "$retired_fbx_gate" >&2
    exit 1
fi

allowed_env='^(ANIMSMITH_GOLDEN_GLB|ANIMSMITH_GOLDEN_SKIP|ANIMSMITH_VERSION)$'
unexpected_env="$(git grep -h -o -E 'ANIMSMITH_[A-Z0-9_]+' -- . | sort -u | grep -Ev "$allowed_env" || true)"
if [[ -n "$unexpected_env" ]]; then
    printf 'unexpected ANIMSMITH_* environment reference(s):\n%s\n' "$unexpected_env" >&2
    exit 1
fi
