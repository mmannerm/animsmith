#!/usr/bin/env bash
# One repository-owned source of truth for the isolated exact-Bevy probe's
# resolved dependency graph: `tools/bevy-readback/Cargo.lock`.
#
# The probe hashes that lock with `include_bytes!`. The engine cannot, because
# the lock lives outside the published crate, so the engine carries the same
# identity as two lines in `bevy_readback_lock.txt`. This script renders those
# lines from the lock and either writes them (`--refresh`) or fails when the
# committed file differs (default; `just bevy-readback-lock` inside `just gates`).
#
# Usage:
#   check-bevy-readback-lock.sh                     check the committed pair, then self-test
#   check-bevy-readback-lock.sh --refresh           rewrite the committed identity file
#   check-bevy-readback-lock.sh --refresh LOCK OUT  write LOCK's identity to OUT
#   check-bevy-readback-lock.sh LOCK [IDENTITY]     check one explicit pair, no self-test
#
# Refreshing from a lock other than the committed one requires an explicit
# destination, so a probe of a foreign lock cannot overwrite the tracked file.
set -euo pipefail

script="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)/$(basename "${BASH_SOURCE[0]}")"
cd "$(dirname "$script")/.."

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
identity="${2:-crates/animsmith-engine/src/bevy_readback_lock.txt}"
refresh_command="just bevy-readback-lock-refresh"
if [ "$refresh" = 1 ] && [ "$explicit_pair" -eq 1 ]; then
    echo "refreshing from $lock needs an explicit destination: --refresh LOCK OUT" >&2
    exit 1
fi
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
# GNU coreutils first, then the BSD tool macOS ships. Falling through on a
# failing tool rather than only on a missing one keeps the second path
# reachable, so the self-test below can prove it agrees with the first.
sha256_of() {
    local reported
    if reported="$(sha256sum "$1" 2>/dev/null)" || reported="$(shasum -a 256 "$1" 2>/dev/null)"; then
        printf '%s\n' "${reported%% *}"
        return 0
    fi
    echo "no working sha256sum or shasum for $1" >&2
    exit 1
}
bytes="$(wc -c < "$lock" | tr -d ' ')"
sha="$(sha256_of "$lock")"
if [ "$refresh" = 1 ]; then
    # Write beside the destination and rename, so an interrupted refresh cannot
    # leave a tracked file truncated.
    staged="$identity.refresh.$$"
    printf '%s\n%s\n' "$bytes" "$sha" > "$staged"
    mv "$staged" "$identity"
    echo "refreshed $identity from $lock"
    exit 0
fi
expected="$(mktemp)"
trap 'rm -f "$expected"' EXIT
printf '%s\n%s\n' "$bytes" "$sha" > "$expected"
if ! cmp -s "$expected" "$identity" 2>/dev/null; then
    echo "$identity is not what $lock renders: $bytes bytes, $sha; run '$refresh_command'" >&2
    diff -u "$identity" "$expected" >&2 || true
    exit 1
fi
test "$explicit_pair" -eq 0 || exit 0

# Self-test: prove the comparison above rejects a lock the identity file does
# not describe, and that `--refresh` writes exactly what it accepts. The
# control run proves a rejection below is the mutation and not a broken
# harness. Version and Bevy-patch drift are exercised by the opt-in matrix in
# test-bevy-readback.sh, which has a lock fixture for them.
work="$(mktemp -d "${TMPDIR:-/tmp}/animsmith-bevy-readback-lock.XXXXXX")"
trap 'rm -f "$expected"; rm -rf "$work"' EXIT
fail() {
    echo "FAIL: $*" >&2
    exit 1
}
cp "$lock" "$work/Cargo.lock"
cp "$identity" "$work/identity.txt"
bash "$script" "$work/Cargo.lock" "$work/identity.txt" \
    || fail "control: the check rejected an unmodified lock and identity pair"
echo "ok: control accepted"

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
set +e
rejection="$(bash "$script" "$work/one-byte-Cargo.lock" "$work/identity.txt" 2>&1)"
rejection_status=$?
set -e
test "$rejection_status" -ne 0 || fail "the check accepted a lock the identity file does not describe"
grep -Fq "$work/identity.txt is not what $work/one-byte-Cargo.lock renders: $bytes bytes, $(sha256_of "$work/one-byte-Cargo.lock")" <<< "$rejection" \
    || fail "the rejection did not name the file, the lock and the identity it renders: $rejection"
grep -Fq "$refresh_command" <<< "$rejection" \
    || fail "the rejection did not name the refresh command: $rejection"
echo "ok: one modified lock byte rejected"

# The identity file's count line alone: a comparison that read only the digest
# would accept this. The digest half is covered by the lock mutation above.
{ echo "$((bytes + 1))"; tail -n 1 "$identity"; } > "$work/wrong-count.txt"
cmp -s "$identity" "$work/wrong-count.txt" && fail "the byte-count mutation changed nothing"
set +e
counted="$(bash "$script" "$work/Cargo.lock" "$work/wrong-count.txt" 2>&1)"
counted_status=$?
set -e
test "$counted_status" -ne 0 || fail "the check accepted an identity file with the wrong byte count"
grep -Fq "$work/wrong-count.txt is not what $work/Cargo.lock renders: $bytes bytes, $sha" <<< "$counted" \
    || fail "the byte-count rejection did not name the identity the lock renders: $counted"
echo "ok: wrong byte count rejected"

# The BSD digest tool the fallback reaches for on macOS, standing in for a host
# where `sha256sum` is missing or broken. It hashes independently, so accepting
# the committed pair through it is two implementations agreeing on this lock.
mkdir "$work/bin"
cat > "$work/bin/sha256sum" <<'ABSENT'
#!/usr/bin/env bash
exit 127
ABSENT
cat > "$work/bin/shasum" <<'BSD'
#!/usr/bin/env bash
set -euo pipefail
test "$1" = "-a" && test "$2" = "256" || { echo "unexpected shasum arguments: $*" >&2; exit 1; }
printf '%s  %s
' "$(python3 -c 'import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$3")" "$3"
BSD
chmod +x "$work/bin/sha256sum" "$work/bin/shasum"
PATH="$work/bin:$PATH" bash "$script" "$work/Cargo.lock" "$work/identity.txt" \
    || fail "the check rejected the committed pair when it fell back to shasum"
set +e
PATH="$work/bin:$PATH" bash "$script" "$work/one-byte-Cargo.lock" "$work/identity.txt" >/dev/null 2>&1
fallback_status=$?
set -e
test "$fallback_status" -ne 0 || fail "the shasum fallback accepted a lock the identity file does not describe"
echo "ok: the shasum fallback agrees with sha256sum"

# `--refresh` must write, from the lock it is handed, exactly the two lines the
# lock renders, and the check must then accept that pair. The lock here differs
# from the committed one in both length and digest (a trailing comment is valid
# TOML and moves neither a package version nor a Bevy patch).
{ cat "$lock"; echo "# a longer lock, for the refresh case below"; } > "$work/longer-Cargo.lock"
longer_bytes="$(wc -c < "$work/longer-Cargo.lock" | tr -d ' ')"
test "$longer_bytes" -ne "$bytes" || fail "the longer lock kept the committed length"
printf '%s\n%s\n' "$longer_bytes" "$(sha256_of "$work/longer-Cargo.lock")" > "$work/wanted.txt"
bash "$script" --refresh "$work/longer-Cargo.lock" "$work/refreshed.txt" > /dev/null \
    || fail "--refresh failed on a valid lock"
cmp -s "$work/wanted.txt" "$work/refreshed.txt" \
    || fail "--refresh did not write the identity of the lock it was handed"
bash "$script" "$work/longer-Cargo.lock" "$work/refreshed.txt" \
    || fail "the check rejected the file --refresh had just written"
echo "ok: --refresh writes the identity the check accepts"

# A refresh from a foreign lock must name its destination rather than silently
# rewriting the tracked file.
set +e
foreign="$(bash "$script" --refresh "$work/longer-Cargo.lock" 2>&1)"
foreign_status=$?
set -e
# Repair before judging, so a regression in that guard cannot leave the tracked
# file rewritten behind this run.
if ! cmp -s "$expected" "$identity"; then
    cp "$expected" "$identity"
    fail "--refresh from a foreign lock rewrote $identity"
fi
test "$foreign_status" -ne 0 || fail "--refresh from a foreign lock did not refuse"
grep -Fq "needs an explicit destination" <<< "$foreign" \
    || fail "the refusal did not name the missing destination: $foreign"
echo "ok: --refresh from a foreign lock refuses without a destination"

echo "bevy-readback lock identity agrees with $lock"
