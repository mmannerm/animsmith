//! Real files exercise the library's complete repair write set. All fixture
//! JSON and motion below are self-authored; expected bytes do not use a writer.

use animsmith_gltf::fix::{FixSession, Repair};
use animsmith_gltf::{FixError, WriteError};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

fn bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn payload() -> Vec<u8> {
    bytes(&[
        0., 1., 0., 1., 0., 0., 0.6, -0.8, 0., 0., 0., 1., 0., 0., 1.,
    ])
}

fn repaired_payload() -> Vec<u8> {
    bytes(&[
        0., 1., 0., 1., 0., 0., -0.6, 0.8, -0., -0., 0., 1., 0., 0., 1.,
    ])
}

fn source(dir: &Path, two_buffers: bool, rotation_buffer: usize) -> PathBuf {
    let buffers = if two_buffers {
        json!([{"uri":"first.bin", "byteLength":60}, {"uri":"second.bin", "byteLength":60}])
    } else {
        // Keep a percent-encoded spelling to prove publication doesn't rewrite URIs.
        json!([{"uri":"first%2Ebin", "byteLength":60}])
    };
    let mut json = json!({
        "asset":{"version":"2.0"}, "nodes":[{"name":"joint"}],
        "buffers":buffers,
        "bufferViews":[
            {"buffer":0,"byteOffset":0,"byteLength":8},
            {"buffer":rotation_buffer,"byteOffset":8,"byteLength":32}
        ],
        "accessors":[
            {"bufferView":0,"componentType":5126,"count":2,"type":"SCALAR","min":[0],"max":[1]},
            {"bufferView":1,"componentType":5126,"count":2,"type":"VEC4"}
        ],
        "animations":[{"samplers":[{"input":0,"output":1,"interpolation":"LINEAR"}],
            "channels":[{"sampler":0,"target":{"node":0,"path":"rotation"}}]}]
    });
    if two_buffers {
        json["bufferViews"]
            .as_array_mut()
            .unwrap()
            .push(json!({"buffer":1-rotation_buffer,"byteOffset":24,"byteLength":36}));
        json["accessors"].as_array_mut().unwrap().push(json!({"bufferView":2,"componentType":5126,"count":3,"type":"VEC3","min":[-0.8,0,0],"max":[0.6,1,1]}));
        json["meshes"] = json!([{"primitives":[{"attributes":{"POSITION":2}}]}]);
    }
    let path = dir.join("scene.gltf");
    fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    fs::write(dir.join("first.bin"), payload()).unwrap();
    if two_buffers {
        fs::write(dir.join("second.bin"), payload()).unwrap();
    }
    path
}

fn session(input: &Path) -> FixSession {
    let mut session = FixSession::read(input).unwrap();
    assert_eq!(session.apply(Repair::QuatFlip).total_fixed(), 1);
    session
}

fn refused(result: Result<(), FixError>) {
    assert!(
        matches!(
            result,
            Err(FixError::Write(WriteError::RepairPublication(_)))
        ),
        "{result:?}"
    );
}

#[test]
fn distinct_primary_cannot_repair_the_source_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    let input = source(dir.path(), false, 0);
    let original = fs::read(&input).unwrap();
    let output = dir.path().join("fixed.gltf");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let session = session(&input);
    refused(session.write(&input, &output));
    assert!(!output.exists());
    fs::write(&output, b"previous output").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&output, fs::Permissions::from_mode(0o640)).unwrap();
    }
    // The session's read location, not this legacy argument, owns protection.
    refused(session.write(&dir.path().join("unrelated.gltf"), &output));
    assert_eq!(fs::read(&output).unwrap(), b"previous output");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(dir.path().join("first.bin")).unwrap(), payload());
}

#[test]
fn separate_directory_copy_preserves_json_uris_and_source_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let input = source(dir.path(), false, 0);
    let original = fs::read(&input).unwrap();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let output = out.join("fixed.gltf");
    let session = session(&input);
    session
        .write(&dir.path().join("unused-legacy-argument.gltf"), &output)
        .unwrap();
    assert_eq!(fs::read(&output).unwrap(), original);
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(dir.path().join("first.bin")).unwrap(), payload());
    assert_eq!(fs::read(out.join("first.bin")).unwrap(), repaired_payload());
    // A live session still owns the captured JSON after the source name moves.
    let renamed = dir.path().join("renamed-source.gltf");
    fs::rename(&input, &renamed).unwrap();
    session.write(&input, &output).unwrap();
    assert!(!input.exists());
    assert_eq!(fs::read(&renamed).unwrap(), original);
    assert_eq!(fs::read(&output).unwrap(), original);
    assert_eq!(fs::read(dir.path().join("first.bin")).unwrap(), payload());
}

#[cfg(unix)]
#[test]
fn json_copies_use_captured_permissions_while_in_place_keeps_its_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let input = source(dir.path(), false, 0);
    let original = fs::read(&input).unwrap();
    fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
    let session = session(&input);
    // Both JSON and permissions belong to the captured session.
    fs::set_permissions(&input, fs::Permissions::from_mode(0o644)).unwrap();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let output = out.join("fixed.gltf");
    session.write(&input, &output).unwrap();
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(fs::read(&output).unwrap(), original);
    assert_eq!(fs::read(out.join("first.bin")).unwrap(), repaired_payload());

    fs::set_permissions(&output, fs::Permissions::from_mode(0o640)).unwrap();
    // A prior longer destination must end at the captured snapshot's exact length.
    fs::write(&output, vec![b'x'; original.len() + 100]).unwrap();
    session.write(&input, &output).unwrap();
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(fs::read(&output).unwrap(), original);
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(dir.path().join("first.bin")).unwrap(), payload());

    fs::set_permissions(&input, fs::Permissions::from_mode(0o640)).unwrap();
    session.write(&input, &input).unwrap();
    assert_eq!(
        fs::metadata(&input).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(
        fs::read(dir.path().join("first.bin")).unwrap(),
        repaired_payload()
    );

    // The other direction may widen permissions only after replacing all old
    // bytes. A successful public copy restores its source mode.
    fs::set_permissions(&input, fs::Permissions::from_mode(0o644)).unwrap();
    let public_session = FixSession::read(&input).unwrap();
    public_session.write(&input, &output).unwrap();
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o644
    );
    assert_eq!(fs::read(&output).unwrap(), original);
}

#[test]
fn hardlinked_buffers_refuse_both_write_orders_but_distinct_files_work() {
    for rotation_buffer in [0, 1] {
        let dir = tempfile::tempdir().unwrap();
        let input = source(dir.path(), true, rotation_buffer);
        let original = fs::read(&input).unwrap();
        let second = dir.path().join("second.bin");
        fs::remove_file(&second).unwrap();
        fs::hard_link(dir.path().join("first.bin"), &second).unwrap();
        refused(session(&input).write(&input, &input));
        assert_eq!(fs::read(&input).unwrap(), original);
        for name in ["first.bin", "second.bin"] {
            let actual = fs::read(dir.path().join(name)).unwrap();
            assert_eq!(actual, payload());
            // Independent POSITION Xs at offsets 24, 36, 48, including the
            // value corrupted when the repaired buffer was written last.
            let x: Vec<f32> = [24, 36, 48]
                .into_iter()
                .map(|at| f32::from_le_bytes(actual[at..at + 4].try_into().unwrap()))
                .collect();
            assert_eq!(x, [0.6, 0., 0.]);
        }
        // The source aliases are not a reason to ban a genuinely independent
        // copy: its two new sidecars can carry their respective buffer bytes.
        let out = dir.path().join("out");
        fs::create_dir(&out).unwrap();
        session(&input)
            .write(&input, &out.join("fixed.gltf"))
            .unwrap();
        assert_eq!(fs::read(dir.path().join("first.bin")).unwrap(), payload());
        for (index, name) in ["first.bin", "second.bin"].into_iter().enumerate() {
            let expected = if index == rotation_buffer {
                repaired_payload()
            } else {
                payload()
            };
            assert_eq!(fs::read(out.join(name)).unwrap(), expected);
        }
        fs::remove_file(&second).unwrap();
        fs::write(&second, payload()).unwrap();
        session(&input).write(&input, &input).unwrap();
        assert_eq!(fs::read(&input).unwrap(), original);
        for (index, name) in ["first.bin", "second.bin"].into_iter().enumerate() {
            let expected = if index == rotation_buffer {
                repaired_payload()
            } else {
                payload()
            };
            assert_eq!(fs::read(dir.path().join(name)).unwrap(), expected);
        }
    }
}

#[test]
fn late_sidecar_alias_preserves_all_existing_output_files() {
    let dir = tempfile::tempdir().unwrap();
    let input = source(dir.path(), true, 0);
    let original = fs::read(&input).unwrap();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let output = out.join("fixed.gltf");
    fs::write(&output, b"previous primary").unwrap();
    fs::write(out.join("first.bin"), b"previous first sidecar").unwrap();
    fs::hard_link(dir.path().join("second.bin"), out.join("second.bin")).unwrap();
    refused(session(&input).write(&input, &output));
    assert_eq!(fs::read(&output).unwrap(), b"previous primary");
    assert_eq!(
        fs::read(out.join("first.bin")).unwrap(),
        b"previous first sidecar"
    );
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(dir.path().join("first.bin")).unwrap(), payload());
    assert_eq!(fs::read(out.join("second.bin")).unwrap(), payload());
}

#[test]
fn missing_later_directory_removes_new_reservations() {
    let dir = tempfile::tempdir().unwrap();
    let input = source(dir.path(), true, 0);
    fs::create_dir(dir.path().join("nested")).unwrap();
    fs::rename(
        dir.path().join("second.bin"),
        dir.path().join("nested/second.bin"),
    )
    .unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    json["buffers"][1]["uri"] = json!("nested/second.bin");
    let original = serde_json::to_vec(&json).unwrap();
    fs::write(&input, &original).unwrap();
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    refused(session(&input).write(&input, &out.join("fixed.gltf")));
    assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(dir.path().join("first.bin")).unwrap(), payload());
    assert_eq!(
        fs::read(dir.path().join("nested/second.bin")).unwrap(),
        payload()
    );
}

#[cfg(windows)]
fn prospective_short_alias(dir: &Path, name: &str) -> Option<PathBuf> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    // The occupied sibling changes the generated alias from the one a private
    // child directory would assign. Observe the actual filesystem independently.
    fs::write(
        dir.join("Long-animation-existing.gltf"),
        b"existing sibling",
    )
    .unwrap();
    let path = dir.join(name);
    fs::write(&path, b"alias control").unwrap();
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: the input is NUL-terminated; first query has no output buffer.
    let length = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    assert_ne!(length, 0, "{}", std::io::Error::last_os_error());
    let mut short = vec![0u16; length as usize];
    // SAFETY: the output buffer has the exact capacity returned by Windows.
    let written = unsafe { GetShortPathNameW(wide.as_ptr(), short.as_mut_ptr(), length) };
    assert!(written > 0 && written < length);
    let short = PathBuf::from(std::ffi::OsString::from_wide(&short[..written as usize]));
    assert_eq!(fs::read(&short).unwrap(), b"alias control");
    let alias = short.file_name().unwrap().to_owned();
    fs::remove_file(path).unwrap();
    if alias == name {
        eprintln!("8.3 aliases disabled on test volume; no prospective alias exists");
        None
    } else {
        eprintln!("actual sibling-dependent 8.3 alias exercised: {alias:?}");
        Some(PathBuf::from(alias))
    }
}

#[cfg(windows)]
#[test]
fn windows_prospective_short_alias_cannot_occupy_a_missing_image() {
    let dir = tempfile::tempdir().unwrap();
    let output_name = "Long-animation-output.gltf";
    let Some(alias) = prospective_short_alias(dir.path(), output_name) else {
        return;
    };
    let input = dir.path().join("scene.gltf");
    let original = serde_json::to_vec(&json!({
        "asset":{"version":"2.0"}, "images":[{"uri":alias.to_str().unwrap()}]
    }))
    .unwrap();
    fs::write(&input, &original).unwrap();
    assert!(!dir.path().join(&alias).exists());
    refused(
        FixSession::read(&input)
            .unwrap()
            .write(&input, &dir.path().join(output_name)),
    );
    assert!(!dir.path().join(output_name).exists());
    assert!(!dir.path().join(&alias).exists());
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(
        fs::read(dir.path().join("Long-animation-existing.gltf")).unwrap(),
        b"existing sibling"
    );
    // A different real name is still publishable beside the same missing image.
    let control = dir.path().join("independent.gltf");
    FixSession::read(&input)
        .unwrap()
        .write(&input, &control)
        .unwrap();
    assert_eq!(fs::read(control).unwrap(), original);
    assert!(!dir.path().join(alias).exists());
}

#[cfg(windows)]
#[test]
fn windows_prospective_short_alias_cannot_merge_primary_and_buffer() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("source");
    let out = dir.path().join("out");
    fs::create_dir(&src).unwrap();
    fs::create_dir(&out).unwrap();
    let output_name = "Long-animation-output.gltf";
    let Some(alias) = prospective_short_alias(&out, output_name) else {
        return;
    };
    let input = source(&src, false, 0);
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    json["buffers"][0]["uri"] = json!(alias.to_str().unwrap());
    let original = serde_json::to_vec(&json).unwrap();
    fs::write(&input, &original).unwrap();
    fs::rename(src.join("first.bin"), src.join(&alias)).unwrap();
    refused(session(&input).write(&input, &out.join(output_name)));
    assert!(!out.join(output_name).exists());
    assert!(!out.join(&alias).exists());
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(src.join(&alias)).unwrap(), payload());
    assert_eq!(
        fs::read(out.join("Long-animation-existing.gltf")).unwrap(),
        b"existing sibling"
    );
}

#[test]
fn removing_the_source_name_does_not_lose_protection_for_its_remaining_hardlink() {
    let dir = tempfile::tempdir().unwrap();
    let input = source(dir.path(), false, 0);
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    fs::hard_link(dir.path().join("first.bin"), out.join("first.bin")).unwrap();
    let session = session(&input);
    fs::remove_file(dir.path().join("first.bin")).unwrap();
    refused(session.write(&input, &out.join("fixed.gltf")));
    assert!(!out.join("fixed.gltf").exists());
    assert!(!dir.path().join("first.bin").exists());
    assert_eq!(fs::read(out.join("first.bin")).unwrap(), payload());
}

#[test]
fn a_sidecar_cannot_alias_the_output_primary() {
    let dir = tempfile::tempdir().unwrap();
    let input = source(dir.path(), false, 0);
    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let output = out.join("first.bin");
    fs::write(&output, b"previous primary").unwrap();
    refused(session(&input).write(&input, &output));
    assert_eq!(fs::read(&output).unwrap(), b"previous primary");
    assert_eq!(fs::read(dir.path().join("first.bin")).unwrap(), payload());
}

#[test]
fn missing_images_are_protected_using_the_filesystems_name_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("scene.gltf");
    let json = br#"{ "asset":{"version":"2.0"}, "images":[{"uri":"Preview.Gltf"}] }"#;
    fs::write(&input, json).unwrap();
    let session = FixSession::read(&input).unwrap();
    refused(session.write(&input, &dir.path().join("Preview.Gltf")));
    assert!(!dir.path().join("Preview.Gltf").exists());
    // Independent real-filesystem control; do not assume an OS implies case.
    fs::write(dir.path().join("CaseControl"), b"control").unwrap();
    let insensitive = dir.path().join("casecontrol").exists();
    fs::remove_file(dir.path().join("CaseControl")).unwrap();
    let output = dir.path().join("preview.gltf");
    let result = session.write(&input, &output);
    if insensitive {
        refused(result);
        assert!(!output.exists());
    } else {
        result.unwrap();
        assert_eq!(fs::read(&output).unwrap(), json);
        assert!(!dir.path().join("Preview.Gltf").exists());
    }
    assert_eq!(fs::read(&input).unwrap(), json);
}

#[test]
fn in_place_does_not_authorize_overwriting_an_image_alias() {
    let dir = tempfile::tempdir().unwrap();
    let input = source(dir.path(), false, 0);
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    json["images"] = json!([{"uri":"image.png"}]);
    fs::write(&input, serde_json::to_vec(&json).unwrap()).unwrap();
    fs::hard_link(dir.path().join("first.bin"), dir.path().join("image.png")).unwrap();
    let original = fs::read(&input).unwrap();
    refused(session(&input).write(&input, &input));
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(dir.path().join("first.bin")).unwrap(), payload());
    assert_eq!(fs::read(dir.path().join("image.png")).unwrap(), payload());
}

#[test]
fn external_containers_preserve_bytes_and_allow_primary_hardlink_in_place() {
    for glb_container in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let json_path = source(dir.path(), false, 0);
        let mut json = fs::read(&json_path).unwrap();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        // A valid BIN-less GLB with an external buffer; raw container framing
        // makes the original header/JSON bytes an independent preservation oracle.
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&(20u32 + json.len() as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json);
        let (input, output, original) = if glb_container {
            (
                dir.path().join("scene.glb"),
                dir.path().join("fixed.glb"),
                glb,
            )
        } else {
            (
                json_path.clone(),
                dir.path().join("fixed.gltf"),
                fs::read(&json_path).unwrap(),
            )
        };
        fs::write(&input, &original).unwrap();
        fs::write(&output, b"prior GLB").unwrap();
        refused(session(&input).write(&input, &output));
        assert_eq!(fs::read(&output).unwrap(), b"prior GLB");
        assert_eq!(fs::read(&input).unwrap(), original);
        assert_eq!(fs::read(dir.path().join("first.bin")).unwrap(), payload());

        fs::remove_file(&output).unwrap();
        fs::hard_link(&input, &output).unwrap();
        session(&input).write(&input, &output).unwrap();
        assert_eq!(fs::read(&output).unwrap(), original);
        assert_eq!(fs::read(&input).unwrap(), original);
        assert_eq!(
            fs::read(dir.path().join("first.bin")).unwrap(),
            repaired_payload()
        );
    }
}

#[cfg(unix)]
#[test]
fn readonly_source_with_missing_images_needs_no_source_directory_probe() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("source");
    let out = dir.path().join("out");
    fs::create_dir(&src).unwrap();
    fs::create_dir(&out).unwrap();
    let input = source(&src, false, 0);
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    json["images"] = json!([{"uri":"missing.png"}, {"uri":"missing-directory/image.png"}]);
    fs::write(&input, serde_json::to_vec(&json).unwrap()).unwrap();
    let original = fs::read(&input).unwrap();
    fs::set_permissions(&src, fs::Permissions::from_mode(0o555)).unwrap();
    let result = session(&input).write(&input, &out.join("fixed.gltf"));
    fs::set_permissions(&src, fs::Permissions::from_mode(0o755)).unwrap();
    result.unwrap();
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(src.join("first.bin")).unwrap(), payload());
    assert_eq!(fs::read(out.join("first.bin")).unwrap(), repaired_payload());
    assert_eq!(fs::read_dir(&src).unwrap().count(), 2);
}

#[cfg(unix)]
#[test]
fn existing_in_place_files_need_no_parent_creation_permission() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let input = source(dir.path(), false, 0);
    let original = fs::read(&input).unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
    let result = session(&input).write(&input, &input);
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    result.unwrap();
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(
        fs::read(dir.path().join("first.bin")).unwrap(),
        repaired_payload()
    );
}

#[cfg(unix)]
#[test]
fn symlinked_parents_and_followed_sidecars_do_not_bypass_source_protection() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("source");
    let out = dir.path().join("out");
    fs::create_dir(&src).unwrap();
    fs::create_dir(&out).unwrap();
    let input = source(&src, false, 0);
    let original = fs::read(&input).unwrap();
    symlink(&src, dir.path().join("alias")).unwrap();
    refused(session(&input).write(&input, &dir.path().join("alias/fixed.gltf")));
    symlink(src.join("first.bin"), out.join("first.bin")).unwrap();
    refused(session(&input).write(&input, &out.join("fixed.gltf")));
    assert!(!out.join("fixed.gltf").exists());
    assert!(!src.join("fixed.gltf").exists());
    assert_eq!(fs::read(&input).unwrap(), original);
    assert_eq!(fs::read(src.join("first.bin")).unwrap(), payload());
}

#[cfg(unix)]
#[test]
fn primary_symlink_keeps_its_declared_resource_root() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let declared = dir.path().join("declared");
    fs::create_dir(&declared).unwrap();
    let target = source(dir.path(), false, 0);
    fs::rename(dir.path().join("first.bin"), declared.join("first.bin")).unwrap();
    let input = declared.join("scene.gltf");
    symlink(&target, &input).unwrap();
    let original = fs::read(&target).unwrap();
    refused(session(&input).write(&input, &declared.join("fixed.gltf")));
    assert!(!declared.join("fixed.gltf").exists());
    assert_eq!(fs::read(&target).unwrap(), original);
    assert_eq!(fs::read(declared.join("first.bin")).unwrap(), payload());
}

#[cfg(unix)]
#[test]
fn dangling_final_symlink_resolves_to_the_protected_missing_image() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("scene.gltf");
    let json = br#"{"asset":{"version":"2.0"},"images":[{"uri":"missing.png"}]}"#;
    fs::write(&input, json).unwrap();
    symlink("missing.png", dir.path().join("fixed.gltf")).unwrap();
    refused(
        FixSession::read(&input)
            .unwrap()
            .write(&input, &dir.path().join("fixed.gltf")),
    );
    assert!(!dir.path().join("missing.png").exists());
    assert_eq!(
        fs::read_link(dir.path().join("fixed.gltf")).unwrap(),
        Path::new("missing.png")
    );
    assert_eq!(fs::read(&input).unwrap(), json);
}
