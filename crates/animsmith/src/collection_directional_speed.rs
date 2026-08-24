//! Directional-speed evaluator command and bounded raw-input capture.
//!
//! The policy reader, collection-output reader, typed adapter, and evaluator
//! own their respective contracts. This module only captures the two raw
//! bounded inputs once, binds their exact byte identities, and applies the
//! command's stdout and exit-code contract.

use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use std::process::ExitCode;

use animsmith_core::{
    COLLECTION_DIRECTIONAL_SPEED_EVIDENCE_V1_MAX_BYTES,
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES, InputIdentity,
    evaluate_collection_directional_speed_v1,
};

use super::{EXIT_FINDINGS, collection_directional_speed_policy, collection_output, render};

/// Run the JSON-only directional-speed policy evaluator.
pub(crate) fn run(policy_path: &Path, evidence_path: &Path) -> Result<ExitCode, String> {
    // The raw source bytes are identities, not a serialization cache. Read
    // each only once under its independent cap before any parsed value or
    // retained buffer can escape this command boundary.
    let policy_bytes = read_bounded(
        policy_path,
        COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES,
        "directional-speed policy",
    )?;
    let policy =
        collection_directional_speed_policy::parse_collection_directional_speed_policy_bytes(
            &policy_bytes,
        )
        .map_err(|error| error.to_string())?;

    let evidence_bytes = read_bounded(
        evidence_path,
        COLLECTION_DIRECTIONAL_SPEED_EVIDENCE_V1_MAX_BYTES,
        "collection-output evidence",
    )?;
    let output = collection_output::read_collection_output(Cursor::new(&evidence_bytes))
        .map_err(|error| error.to_string())?;
    let evidence = output
        .directional_speed_evidence(policy.runtime_set_id())
        .map_err(|error| error.to_string())?;
    let result = evaluate_collection_directional_speed_v1(
        &policy,
        InputIdentity::from_bytes(&policy_bytes),
        InputIdentity::from_bytes(&evidence_bytes),
        &evidence,
    )
    .map_err(|error| error.to_string())?;

    let requires_failure = result.not_evaluated_reason().is_some() || !result.findings().is_empty();
    // This is the one canonical serializer for the immutable result; no
    // output-file or alternate presentation path exists for this command.
    render::print_json(&result)?;
    Ok(if requires_failure {
        ExitCode::from(EXIT_FINDINGS)
    } else {
        ExitCode::SUCCESS
    })
}

fn read_bounded(path: &Path, limit: u64, label: &str) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|_| format!("cannot read {label}"))?;
    let mut bytes = Vec::new();
    let mut reader = file.take(limit + 1);
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| format!("cannot read {label}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{label} exceeds its bounded reader limit"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::read_bounded;

    #[test]
    fn bounded_reader_refuses_n_plus_one_without_unbounded_reading() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), [0_u8; 5]).unwrap();
        assert!(read_bounded(temp.path(), 4, "test input").is_err());
    }
}
