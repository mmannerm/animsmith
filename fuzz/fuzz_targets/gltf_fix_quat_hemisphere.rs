#![no_main]
//! Fuzz `animsmith_gltf::fix::fix_quat_hemisphere`: the byte-surgical
//! quaternion repair path — parse, locate rotation accessors, patch value
//! bytes, and re-derive the GLB chunk bounds on write-out. Exercises the
//! read + patch + write pipeline end to end against arbitrary containers.
//! No input may panic or OOM (invariant-1); the byte-surgery guarantees
//! themselves (invariant-2) are covered by unit tests, not here.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let input = animsmith_fuzz::scratch_file("fix_input.glb", data);
    let output = animsmith_fuzz::scratch_path("fix_output.glb");
    let _ = animsmith_gltf::fix::fix_quat_hemisphere(&input, &output);
});
