//! Frozen identity of the committed `tools/bevy-readback/Cargo.lock`.
//!
//! The probe hashes that lock with `include_bytes!`. This crate cannot: the
//! lock lives outside the published package. `bevy_readback_lock.txt` carries
//! the same identity as two lines, the byte count then the lowercase SHA-256.
//! `just bevy-readback-lock-refresh` writes those lines from the lock and
//! `just bevy-readback-lock` renders them again and fails on any difference,
//! so the lock stays the one repository-owned source of this identity.

const FROZEN: (u64, &str) = parse(include_str!("bevy_readback_lock.txt"));

/// Frozen byte count of the committed excluded-tool lock graph.
pub const BEVY_READBACK_V1_LOCK_BYTES: u64 = FROZEN.0;
/// Frozen SHA-256 of the committed excluded-tool lock graph.
pub const BEVY_READBACK_V1_LOCK_SHA256: &str = FROZEN.1;

/// Split `<bytes>\n<sha256>\n` into its two fields.
///
/// Every rejection here is a compile error on the committed file, so a
/// truncated, reordered, or uppercase identity cannot reach the strict reader
/// as a silently wrong constant.
const fn parse(raw: &str) -> (u64, &str) {
    let raw_bytes = raw.as_bytes();
    let mut index = 0;
    let mut count = 0;
    while index < raw_bytes.len() && raw_bytes[index] != b'\n' {
        let digit = raw_bytes[index];
        assert!(digit.is_ascii_digit(), "lock byte count must be decimal");
        count = count * 10 + (digit - b'0') as u64;
        index += 1;
    }
    assert!(index > 0, "lock identity must open with a byte count");
    assert!(
        raw_bytes.len() == index + 66,
        "lock identity must be a byte count and a 64-digit digest, each on its own line"
    );
    assert!(
        raw_bytes[raw_bytes.len() - 1] == b'\n',
        "lock identity must end with a newline"
    );
    let mut cursor = index + 1;
    while cursor < index + 65 {
        let digit = raw_bytes[cursor];
        assert!(
            digit.is_ascii_digit() || (digit >= b'a' && digit <= b'f'),
            "lock digest must be lowercase hexadecimal"
        );
        cursor += 1;
    }
    let (_, digest_and_newline) = raw.split_at(index + 1);
    let (digest, _) = digest_and_newline.split_at(64);
    (count, digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parser_splits_a_byte_count_from_the_digest_that_follows_it() {
        // 1024 differs from its own reversal, so a parser accumulating digits
        // in the wrong order cannot pass; the digest is likewise asymmetric.
        assert_eq!(
            parse("1024\n0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n"),
            (
                1024,
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            )
        );
    }

    #[test]
    #[should_panic(expected = "lock digest must be lowercase hexadecimal")]
    fn the_parser_refuses_an_uppercase_digest() {
        parse("1024\n0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef\n");
    }
}
