#!/usr/bin/env bash
# One repository-owned source of truth for the isolated exact-Bevy probe's
# resolved dependency graph: `tools/bevy-readback/Cargo.lock`.
#
# The probe itself hashes that lock with `include_bytes!`. The engine cannot,
# because the lock lives outside the published crate, so the engine carries a
# generated module holding the same identity. This script renders that module
# from the lock and either writes it (`--refresh`) or fails when the committed
# module disagrees (default; `just bevy-readback-lock` inside `just gates`).
#
# Usage:
#   check-bevy-readback-lock.sh            check the committed pair, then self-test
#   check-bevy-readback-lock.sh --refresh  rewrite the generated module
#   check-bevy-readback-lock.sh LOCK [MODULE]
#                                          check one explicit pair, no self-test
set -euo pipefail

cd "$(dirname "$0")/.."

refresh=0
if [ "${1:-}" = "--refresh" ]; then
    refresh=1
    shift
fi
# An explicit pair is used by the integration fixture in test-bevy-readback.sh
# to test rejection of an internal patch drift, and by this script's own
# self-test below. Passing one also suppresses the self-test, so the mutant
# runs do not recurse.
explicit_pair=$#
lock="${1:-tools/bevy-readback/Cargo.lock}"
generated="${2:-crates/animsmith-engine/src/bevy_readback_lock.rs}"
refresh_command="just bevy-readback-lock-refresh"
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
# Underscore-grouped digits, matching the hand-written bounds this generated
# module sits beside in the engine crate.
group_digits() {
    local digits="$1" grouped=""
    while [ "${#digits}" -gt 3 ]; do
        grouped="_${digits: -3}$grouped"
        digits="${digits:0:${#digits}-3}"
    done
    printf '%s%s' "$digits" "$grouped"
}
# The literals a module states, wherever they sit: the drift diagnosis and the
# self-test both read a module through these, so neither is tied to the
# generator's exact layout. A module stating a literal twice yields two lines
# and matches nothing.
declared_bytes_of() {
    sed -nE 's/.*BEVY_READBACK_V1_LOCK_BYTES: u64 = ([0-9_]+);.*/\1/p' "$1" | tr -d '_'
}
declared_sha_of() {
    sed -nE 's/.*"([0-9a-f]{64})".*/\1/p' "$1"
}
# The one renderer both modes use, so `--refresh` cannot write text the check
# would then reject.
render_module() {
    cat <<RUST
//! Frozen identity of the committed \`tools/bevy-readback/Cargo.lock\`.
//!
//! Generated from that lock; do not edit by hand. Regenerate it with
//! \`$refresh_command\`. \`just bevy-readback-lock\` renders the
//! same text and fails when this module and the lock disagree.

/// Frozen byte count of the committed excluded-tool lock graph.
pub const BEVY_READBACK_V1_LOCK_BYTES: u64 = $(group_digits "$bytes");
/// Frozen SHA-256 of the committed excluded-tool lock graph.
pub const BEVY_READBACK_V1_LOCK_SHA256: &str =
    "$sha";
RUST
}
if [ "$refresh" = 1 ]; then
    render_module > "$generated"
    echo "refreshed $generated from $lock"
    exit 0
fi
test -f "$generated" || {
    echo "bevy-readback lock module missing: $generated; run '$refresh_command'" >&2
    exit 1
}
expected="$(mktemp)"
trap 'rm -f "$expected"' EXIT
render_module > "$expected"
if ! cmp -s "$expected" "$generated"; then
    # Name the drift instead of printing a diff: these are distinct repository
    # states, and only the last one is a hand edit of a generated file.
    declared_bytes="$(declared_bytes_of "$generated")"
    declared_sha="$(declared_sha_of "$generated")"
    if [ "$declared_bytes" != "$bytes" ]; then
        echo "bevy-readback lock byte drift: $generated declares ${declared_bytes:-no byte count}, $lock is $bytes bytes; run '$refresh_command'" >&2
        exit 1
    fi
    if [ "$declared_sha" != "$sha" ]; then
        echo "bevy-readback lock hash drift: $generated declares ${declared_sha:-no digest}, $lock hashes to $sha; run '$refresh_command'" >&2
        exit 1
    fi
    echo "bevy-readback lock module drift: $generated carries the right identity but is not the text '$refresh_command' generates; run it" >&2
    exit 1
fi
test "$explicit_pair" -eq 0 || exit 0

# Self-test: prove the comparison above rejects each way the generated module
# and the lock can disagree. The control run proves the mutants are rejected
# for their mutation rather than because the harness itself is broken.
work="$(mktemp -d "${TMPDIR:-/tmp}/animsmith-bevy-readback-lock.XXXXXX")"
trap 'rm -f "$expected"; rm -rf "$work"' EXIT
fail() {
    echo "FAIL: $*" >&2
    exit 1
}
# Each case names its lock, its module, and the message the check must print.
expect_rejected() {
    local name="$1" case_lock="$2" case_module="$3" wanted="$4" output status
    set +e
    output="$(bash scripts/check-bevy-readback-lock.sh "$case_lock" "$case_module" 2>&1)"
    status=$?
    set -e
    test "$status" -ne 0 || fail "$name: the check accepted the mutation"
    grep -Fq "$wanted" <<< "$output" \
        || fail "$name: expected a message containing '$wanted', got: $output"
    grep -Fq "$refresh_command" <<< "$output" \
        || fail "$name: the message did not name the refresh command: $output"
    echo "ok: $name rejected"
}
zeros="$(printf '%064d' 0)"
cp "$lock" "$work/Cargo.lock"
cp "$generated" "$work/bevy_readback_lock.rs"
bash scripts/check-bevy-readback-lock.sh "$work/Cargo.lock" "$work/bevy_readback_lock.rs" \
    || fail "control: the check rejected an unmodified lock and module pair"
echo "ok: control accepted"

expect_rejected "missing generated module" \
    "$work/Cargo.lock" "$work/absent.rs" \
    "module missing: $work/absent.rs"

# One flipped hex digit inside the first checksum: the lock keeps its length
# and its package versions, so only the digest can catch it.
awk '
    !flipped && /^checksum = "/ {
        last = substr($0, length($0) - 1, 1)
        $0 = substr($0, 1, length($0) - 2) (last == "0" ? "1" : "0") "\""
        flipped = 1
    }
    { print }
' "$lock" > "$work/one-byte-Cargo.lock"
test "$(wc -c < "$work/one-byte-Cargo.lock" | tr -d ' ')" -eq "$bytes" \
    || fail "the one-byte lock mutation changed the lock's length"
cmp -s "$lock" "$work/one-byte-Cargo.lock" && fail "the one-byte lock mutation changed nothing"
expect_rejected "one modified lock byte" \
    "$work/one-byte-Cargo.lock" "$work/bevy_readback_lock.rs" \
    "hash drift: $work/bevy_readback_lock.rs declares $sha, $work/one-byte-Cargo.lock hashes to"

# The right digest beside a wrong byte count.
sed -E "s/^(pub const BEVY_READBACK_V1_LOCK_BYTES: u64 = )[0-9_]+;$/\1$(group_digits "$((bytes + 1))");/" \
    "$generated" > "$work/wrong-count.rs"
cmp -s "$generated" "$work/wrong-count.rs" && fail "the byte-count mutation changed nothing"
expect_rejected "wrong declared byte count" \
    "$work/Cargo.lock" "$work/wrong-count.rs" \
    "byte drift: $work/wrong-count.rs declares $((bytes + 1)), $work/Cargo.lock is $bytes bytes"

# The right byte count beside a wrong digest.
sed -E "s/^    \"[0-9a-f]{64}\";$/    \"$zeros\";/" "$generated" > "$work/wrong-digest.rs"
cmp -s "$generated" "$work/wrong-digest.rs" && fail "the digest mutation changed nothing"
expect_rejected "wrong declared digest" \
    "$work/Cargo.lock" "$work/wrong-digest.rs" \
    "hash drift: $work/wrong-digest.rs declares $zeros, $work/Cargo.lock hashes to $sha"

# Both literals right, but the module is no longer the text the generator
# writes: the "do not edit by hand" marker has been edited away.
grep -v 'do not edit by hand' "$generated" > "$work/hand-edited.rs"
cmp -s "$generated" "$work/hand-edited.rs" && fail "the hand-edit mutation changed nothing"
expect_rejected "hand-edited generated module" \
    "$work/Cargo.lock" "$work/hand-edited.rs" \
    "module drift: $work/hand-edited.rs carries the right identity but is not the text"

# The identity intact but reflowed onto one line, as a future rustfmt could
# leave it. The diagnosis must still be module drift, so the diagnostic has to
# read both literals wherever they sit rather than only where the generator
# puts them.
awk '
    /BEVY_READBACK_V1_LOCK_SHA256: &str =$/ { printf "%s ", $0; next }
    /^    "/ { sub(/^ +/, ""); print; next }
    { print }
' "$generated" > "$work/reflowed.rs"
cmp -s "$generated" "$work/reflowed.rs" && fail "the reflow mutation changed nothing"
expect_rejected "reflowed generated module" \
    "$work/Cargo.lock" "$work/reflowed.rs" \
    "module drift: $work/reflowed.rs carries the right identity but is not the text"

# `--refresh` must state, as the module's own constants, the identity of the
# lock it is handed, and must write text the check then accepts. The lock used
# here differs from the committed one in both length and digest (a trailing
# comment is valid TOML and moves neither a package version nor a Bevy patch),
# so a refresh that copied the committed module, wrote nothing, mentioned the
# digest only in passing, or rendered differently from the check is caught.
{ cat "$lock"; echo "# a longer lock, for the refresh case below"; } > "$work/longer-Cargo.lock"
longer_bytes="$(wc -c < "$work/longer-Cargo.lock" | tr -d ' ')"
longer_sha="$(sha256sum "$work/longer-Cargo.lock" | awk '{print $1}')"
test "$longer_bytes" -ne "$bytes" || fail "the longer lock kept the committed length"
bash scripts/check-bevy-readback-lock.sh --refresh \
    "$work/longer-Cargo.lock" "$work/refreshed.rs" > /dev/null \
    || fail "--refresh failed on a valid lock"
test "$(declared_bytes_of "$work/refreshed.rs")" = "$longer_bytes" \
    || fail "--refresh did not state the byte count of the lock it was handed"
test "$(declared_sha_of "$work/refreshed.rs")" = "$longer_sha" \
    || fail "--refresh did not state the digest of the lock it was handed"
bash scripts/check-bevy-readback-lock.sh "$work/longer-Cargo.lock" "$work/refreshed.rs" \
    || fail "the check rejected the module --refresh had just written"
echo "ok: --refresh writes the module the check accepts"

echo "bevy-readback lock identity agrees with $lock"
