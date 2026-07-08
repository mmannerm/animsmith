//! Regenerates the committed example assets under `examples/assets/`:
//! the mechanical-check clips (`clip.glb` clean, `clip-dirty.glb` with
//! repairable `quat-norm`/`quat-flip` defects) and the semantic-check
//! walk rigs (`walk.glb` a clean loop, `walk-dirty.glb` a popped seam).
//!
//! The filename↔document wiring lives in `animsmith-testkit`'s
//! `write_example_assets`, which both this example and the guard test
//! (`example_assets_match_generator_output`) drive, so the test builds
//! the identical bytes and exercises this wiring. The dirty copy is
//! `.glb` on purpose: `fix` is byte-surgical over a GLB binary chunk and
//! skips the data-URI buffers a `.gltf` would embed.
//!
//! Run (writes to the repo's `examples/assets/`):
//!   cargo run -p animsmith --example gen_example_assets
//! Write elsewhere:
//!   cargo run -p animsmith --example gen_example_assets -- /some/dir

use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/assets"));
    std::fs::create_dir_all(&out_dir)?;

    animsmith_testkit::write_example_assets(&out_dir, |doc, path| {
        animsmith_gltf::write::write(doc, path).inspect(|()| println!("wrote {}", path.display()))
    })?;

    Ok(())
}
