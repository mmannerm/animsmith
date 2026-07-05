#![no_main]
//! Fuzz `animsmith_fbx::load`: the ufbx C-library boundary and the
//! animation bake that follows it. animsmith's own code must turn any ufbx
//! failure into a `LoadError` rather than panicking (invariant-1). ufbx is
//! C, so a sanitizer finding inside it is an upstream bug to report against
//! the ufbx project, not necessarily an animsmith defect.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let path = animsmith_fuzz::scratch_file("input.fbx", data);
    let _ = animsmith_fbx::load(&path);
});
