#![no_main]
//! Fuzz `FixSession::apply_to_path(..., Repair::QuatFlip)`: the
//! byte-surgical quaternion repair path — parse, locate rotation
//! accessors, patch value bytes, and re-derive the GLB chunk bounds on
//! write-out. Exercises the read + patch + write pipeline end to end
//! against arbitrary containers.
//! No input may panic or OOM (invariant-1); the byte-surgery guarantees
//! themselves (invariant-2) are covered by unit tests, not here.

use animsmith_gltf::fix::{FixSession, Repair};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = animsmith_fuzz::scratch_file("fix_input.glb", data);
    let output = animsmith_fuzz::scratch_path("fix_output.glb");
    let _ = FixSession::apply_to_path(&input, &output, Repair::QuatFlip);
});
