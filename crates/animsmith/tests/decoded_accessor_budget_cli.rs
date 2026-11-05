//! The original tiny sparse allocation bomb is exercised only in a child
//! with address-space, core-dump and wall-time bounds. Portable loader tests
//! use tiny budgets and never request a hostile allocation in-process.
#![cfg(all(target_os = "linux", target_pointer_width = "64"))]

use serde_json::json;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn sparse_glb(count: u64) -> Vec<u8> {
    let mut json = serde_json::to_vec(&json!({
        "asset":{"version":"2.0"},"nodes":[{"name":"root","mesh":0}],
        "scenes":[{"nodes":[0]}],"scene":0,
        "buffers":[{"byteLength":16}],
        "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":4},{"buffer":0,"byteOffset":4,"byteLength":12}],
        "accessors":[{"componentType":5126,"count":count,"type":"VEC3","min":[0,0,0],"max":[0,0,0],
            "sparse":{"count":1,"indices":{"bufferView":0,"componentType":5125},"values":{"bufferView":1}}}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}]
    })).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut glb = b"glTF".to_vec();
    glb.extend(2_u32.to_le_bytes());
    glb.extend((28_u32 + json.len() as u32 + 16).to_le_bytes());
    glb.extend((json.len() as u32).to_le_bytes());
    glb.extend(b"JSON");
    glb.extend(json);
    glb.extend(16_u32.to_le_bytes());
    glb.extend(b"BIN\0");
    glb.extend([0_u8; 16]);
    glb
}

#[test]
fn tiny_sparse_bomb_is_a_located_load_error_and_small_zero_base_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    for (name, count, exit) in [("bomb", 1_u64 << 40, 2), ("small", 3, 0)] {
        let bytes = sparse_glb(count);
        if name == "bomb" {
            assert_eq!(bytes.len(), 528);
        }
        let input = dir.path().join(format!("{name}.glb"));
        let stderr = dir.path().join(format!("{name}.stderr"));
        std::fs::write(&input, bytes).unwrap();
        let mut child = Command::new("bash")
            .args([
                "-c",
                "set -e; ulimit -c 0; ulimit -v 524288; exec \"$1\" inspect \"$2\"",
                "bounded-inspect",
            ])
            .arg(env!("CARGO_BIN_EXE_animsmith"))
            .arg(&input)
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("bounded inspect exceeded 10 seconds");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let diagnostic = std::fs::read_to_string(stderr).unwrap();
        assert_eq!(status.code(), Some(exit), "{name}: {diagnostic}");
        if name == "bomb" {
            assert!(
                diagnostic.contains(
                    "mesh 0 primitive 0 POSITION: accessor 0 exceeds decoded accessor budget"
                ),
                "{diagnostic}"
            );
            assert!(
                diagnostic.contains("1099511627776 elements"),
                "{diagnostic}"
            );
        }
    }
}
