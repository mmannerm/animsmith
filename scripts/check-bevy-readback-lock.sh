#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
lock="tools/bevy-readback/Cargo.lock"
test -f "$lock" || { echo "bevy-readback lock missing" >&2; exit 1; }

workspace_version="$(awk '
    /^\[workspace\.package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version = / { gsub(/"/, "", $3); print $3; exit }
' Cargo.toml)"
test -n "$workspace_version" || { echo "workspace package version missing" >&2; exit 1; }
lock_package_version() {
    awk -v wanted="$1" '
        /^\[\[package\]\]$/ {
            if (matched && version != "") { print version }
            matched = 0
            version = ""
            next
        }
        /^name = / { matched = ($0 == "name = \"" wanted "\"") }
        matched && /^version = / { gsub(/"/, "", $3); version = $3 }
        END { if (matched && version != "") print version }
    ' "$lock"
}
for package in animsmith-core animsmith-engine; do
    locked_version="$(lock_package_version "$package")"
    test "$locked_version" = "$workspace_version" || {
        echo "bevy-readback lock $package version drift" >&2
        exit 1
    }
done
test "$(lock_package_version bevy)" = "0.19.0" || {
    echo "bevy-readback lock must retain exact bevy 0.19.0" >&2
    exit 1
}
bytes="$(wc -c < "$lock" | tr -d ' ')"
sha="$(sha256sum "$lock" | awk '{print $1}')"
declared_bytes="$(sed -nE 's/.*BEVY_READBACK_V1_LOCK_BYTES: u64 = ([0-9_]+);/\1/p' crates/animsmith-engine/src/bevy_readback.rs | tr -d '_')"
test "$declared_bytes" = "$bytes" || { echo "bevy-readback lock byte drift" >&2; exit 1; }
grep -A1 -F "BEVY_READBACK_V1_LOCK_SHA256" crates/animsmith-engine/src/bevy_readback.rs | grep -Fq "\"$sha\"" || { echo "bevy-readback lock hash drift" >&2; exit 1; }
