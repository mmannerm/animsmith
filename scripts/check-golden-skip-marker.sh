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
