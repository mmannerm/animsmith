#![no_main]
//! Fuzz `animsmith_gltf::load`: GLB container framing, embedded JSON, and
//! accessor offset arithmetic against arbitrary bytes. Any input must
//! surface a `LoadError` (or a `Document`) — never a panic or OOM
//! (invariant-1).

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let path = animsmith_fuzz::scratch_file("input.glb", data);
    let _ = animsmith_gltf::load(&path);
});
