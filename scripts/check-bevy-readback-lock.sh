#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
# An explicit lock path is used by the integration fixture to test rejection
# of an internal patch drift. Normal callers use the committed probe lock.
lock="${1:-tools/bevy-readback/Cargo.lock}"
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
# Cargo permits the `bevy = 0.19.0` facade to select newer internal crates.
# The probe's observation is only evidence for one exact graph, so reject any
# Bevy release-crate patch drift (bevy_mikktspace is an independently versioned
# helper and is intentionally excluded).
if ! awk '
    function check_package() {
        if (name ~ /^bevy($|_)/ && name != "bevy_mikktspace" && version != "0.19.0") {
            printf "bevy-readback lock %s must be 0.19.0 (got %s)\n", name, version > "/dev/stderr"
            bad = 1
        }
    }
    /^\[\[package\]\]$/ { check_package(); name = ""; version = ""; next }
    /^name = / { name = $0; sub(/^name = "/, "", name); sub(/"$/, "", name); next }
    /^version = / { version = $0; sub(/^version = "/, "", version); sub(/"$/, "", version); next }
    END { check_package(); exit bad }
' "$lock"; then
    exit 1
fi
bytes="$(wc -c < "$lock" | tr -d ' ')"
sha="$(sha256sum "$lock" | awk '{print $1}')"
declared_bytes="$(sed -nE 's/.*BEVY_READBACK_V1_LOCK_BYTES: u64 = ([0-9_]+);/\1/p' crates/animsmith-engine/src/bevy_readback.rs | tr -d '_')"
test "$declared_bytes" = "$bytes" || { echo "bevy-readback lock byte drift" >&2; exit 1; }
grep -A1 -F "BEVY_READBACK_V1_LOCK_SHA256" crates/animsmith-engine/src/bevy_readback.rs | grep -Fq "\"$sha\"" || { echo "bevy-readback lock hash drift" >&2; exit 1; }
