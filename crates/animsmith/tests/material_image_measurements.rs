//! Black-box CLI coverage for the source material/image measurement inventory.

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

const MEASUREMENTS_SCHEMA: &str = include_str!("../../../docs/schemas/measurements-v4.schema.json");

fn assert_valid_measurements(value: &Value) {
    let schema = serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid v4 schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("measurement schema compiles");
    let errors = validator.iter_errors(value).collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "v4 measurement schema errors: {errors:#?}"
    );
}

fn png(color_type: ExtendedColorType, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, CompressionType::Best, FilterType::NoFilter)
        .write_image(pixels, 1, 1, color_type)
        .expect("encodes PNG fixture");
    bytes
}

fn jpeg() -> Vec<u8> {
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 100)
        .write_image(&[1, 2, 3], 1, 1, ExtendedColorType::Rgb8)
        .expect("encodes JPEG fixture");
    bytes
}

fn write_image(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).expect("writes image fixture");
}

fn write_fixture(path: &Path) {
    write_image(
        &path.with_file_name("l.png"),
        &png(ExtendedColorType::L8, &[7]),
    );
    write_image(
        &path.with_file_name("la.png"),
        &png(ExtendedColorType::La8, &[7, 255]),
    );
    write_image(
        &path.with_file_name("rgb.png"),
        &png(ExtendedColorType::Rgb8, &[1, 2, 3]),
    );
    write_image(
        &path.with_file_name("rgba.png"),
        &png(ExtendedColorType::Rgba8, &[1, 2, 3, 255]),
    );
    write_image(&path.with_file_name("photo.jpg"), &jpeg());
    // The PNG signature makes container detection observable while the absent
    // chunks force decode failure rather than a successful image inspection.
    write_image(
        &path.with_file_name("broken.png"),
        b"\x89PNG\r\n\x1a\ntruncated",
    );
    write_image(
        &path.with_file_name("unsupported.bmp"),
        b"BMnot-a-real-bitmap",
    );

    // The data URI is a known valid one-pixel RGBA PNG. Its URI is intentionally
    // short and relative-free so output must never reveal a host path.
    let document = json!({
        "asset": { "version": "2.0" },
        "images": [
            { "name": "luminance", "uri": "l.png" },
            { "name": "luminance-alpha", "uri": "la.png" },
            { "name": "rgb", "uri": "rgb.png" },
            { "name": "rgba", "uri": "rgba.png" },
            { "name": "photo", "uri": "photo.jpg" },
            { "name": "broken", "uri": "broken.png" },
            { "name": "unsupported", "uri": "unsupported.bmp" },
            { "name": "inline", "uri": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL9aQAAAABJRU5ErkJggg==" }
        ],
        "textures": [
            { "name": "shared-base", "source": 3 },
            { "name": "normal", "source": 2 },
            { "name": "photo", "source": 4 },
            { "name": "inline", "source": 7 }
        ],
        "materials": [
            { "name": "untextured" },
            { "name": "painted", "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } } },
            { "name": "detailed", "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } }, "normalTexture": { "index": 1 } },
            { "name": "photo-material", "pbrMetallicRoughness": { "baseColorTexture": { "index": 2 }, "metallicRoughnessTexture": { "index": 3 } }, "occlusionTexture": { "index": 3 } }
        ]
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&document).expect("serializes glTF"),
    )
    .expect("writes glTF fixture");
}

fn measure(input: &Path, command: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            command,
            input.to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(
        output.status.success(),
        "{command} stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON")
}

#[test]
fn cli_measure_reports_deterministic_material_image_inventory_without_paths() {
    let directory = tempfile::tempdir().expect("temporary fixture directory");
    let input = directory.path().join("materials.gltf");
    write_fixture(&input);

    let source = gltf::Gltf::open(&input).expect("fixture parses as glTF");
    assert_eq!(
        source.materials().count(),
        4,
        "untextured and textured materials"
    );
    assert_eq!(
        source.textures().count(),
        4,
        "shared and independent textures"
    );
    assert_eq!(
        source.images().count(),
        8,
        "all image forms remain declared"
    );
    assert!(
        source
            .materials()
            .nth(2)
            .expect("detailed material")
            .normal_texture()
            .is_some()
    );

    let first = measure(&input, "measure");
    let second = measure(&input, "measure");
    assert_eq!(first, second, "repeated measurement is deterministic");
    let measurements = &first["files"][0]["measurements"];
    assert_valid_measurements(measurements);
    assert_eq!(measurements["material_resource_coverage"], "complete");
    assert_eq!(
        measurements["material_definitions"],
        json!([
            { "material_index": 0, "name": "untextured", "texture_bindings": [] },
            { "material_index": 1, "name": "painted", "texture_bindings": [{ "slot": "base_color", "texture_index": 0 }] },
            { "material_index": 2, "name": "detailed", "texture_bindings": [{ "slot": "base_color", "texture_index": 0 }, { "slot": "normal", "texture_index": 1 }] },
            { "material_index": 3, "name": "photo-material", "texture_bindings": [{ "slot": "base_color", "texture_index": 2 }, { "slot": "metallic_roughness", "texture_index": 3 }, { "slot": "occlusion", "texture_index": 3 }] }
        ])
    );
    assert_eq!(
        measurements["textures"],
        json!([
            { "texture_index": 0, "name": "shared-base", "image_index": 3 },
            { "texture_index": 1, "name": "normal", "image_index": 2 },
            { "texture_index": 2, "name": "photo", "image_index": 4 },
            { "texture_index": 3, "name": "inline", "image_index": 7 }
        ])
    );
    let images = measurements["images"]
        .as_array()
        .expect("image measurements");
    assert_eq!(images.len(), 8);
    assert_eq!(
        images
            .iter()
            .map(|image| image["name"].as_str())
            .collect::<Vec<_>>(),
        vec![
            Some("luminance"),
            Some("luminance-alpha"),
            Some("rgb"),
            Some("rgba"),
            Some("photo"),
            Some("broken"),
            Some("unsupported"),
            Some("inline"),
        ],
        "authored image names remain source-ordered identity evidence"
    );
    for (index, color_type, channels) in
        [(0, "l8", 1), (1, "la8", 2), (2, "rgb8", 3), (3, "rgba8", 4)]
    {
        assert_eq!(images[index]["image_index"], index);
        assert_eq!(images[index]["source_kind"], "external");
        assert_eq!(images[index]["detected_container"], "png");
        assert_eq!(images[index]["width"], 1);
        assert_eq!(images[index]["height"], 1);
        assert_eq!(images[index]["decoded_color_type"], color_type);
        assert_eq!(images[index]["channel_count"], channels);
        assert!(images[index].get("unavailable_reason").is_none());
    }
    assert_eq!(images[4]["source_kind"], "external");
    assert_eq!(images[4]["detected_container"], "jpeg");
    assert_eq!(images[4]["width"], 1);
    assert_eq!(images[4]["height"], 1);
    assert_eq!(images[4]["channel_count"], 3);
    assert_eq!(images[4]["decoded_color_type"], "rgb8");
    assert_eq!(images[5]["source_kind"], "external");
    assert_eq!(images[5]["detected_container"], "png");
    assert_eq!(images[5]["unavailable_reason"], "decode_failed");
    assert!(images[5].get("width").is_none());
    assert_eq!(images[6]["source_kind"], "external");
    assert_eq!(images[6]["unavailable_reason"], "unsupported_container");
    assert_eq!(images[7]["source_kind"], "data_uri");
    assert_eq!(images[7]["declared_mime_type"], "image/png");
    assert_eq!(images[7]["detected_container"], "png");
    assert!(
        !serde_json::to_string(images)
            .expect("serializes image records")
            .contains(&directory.path().display().to_string()),
        "resource records must not leak host paths; the outer input path is intentional CLI evidence"
    );

    let lint = measure(&input, "lint");
    assert_valid_measurements(&lint["files"][0]["measurements"]);
    assert_eq!(lint["files"][0]["measurements"], *measurements);
}

#[test]
fn cli_measure_reports_embedded_image_metadata() {
    let directory = tempfile::tempdir().expect("temporary fixture directory");
    let input = directory.path().join("embedded.glb");
    let mut json = serde_json::to_vec(&json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "byteLength": 70 }],
        "bufferViews": [{ "buffer": 0, "byteLength": 70 }],
        "images": [{ "name": "packed", "bufferView": 0, "mimeType": "image/png" }]
    }))
    .expect("serializes GLB JSON");
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut image = png(ExtendedColorType::Rgba8, &[1, 2, 3, 255]);
    let image_len = image.len();
    // Keep the declared buffer/view lengths exact while retaining legal GLB padding.
    json = serde_json::to_vec(&json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "byteLength": image_len }],
        "bufferViews": [{ "buffer": 0, "byteLength": image_len }],
        "images": [{ "name": "packed", "bufferView": 0, "mimeType": "image/png" }]
    }))
    .expect("serializes GLB JSON");
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    while !image.len().is_multiple_of(4) {
        image.push(0);
    }
    let total = 12 + 8 + json.len() + 8 + image.len();
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(image.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942_u32.to_le_bytes());
    glb.extend_from_slice(&image);
    std::fs::write(&input, glb).expect("writes GLB fixture");
    let image = &measure(&input, "measure")["files"][0]["measurements"]["images"][0];
    assert_eq!(
        image,
        &json!({
            "image_index": 0,
            "name": "packed",
            "source_kind": "embedded",
            "declared_mime_type": "image/png",
            "detected_container": "png",
            "width": 1,
            "height": 1,
            "channel_count": 4,
            "decoded_color_type": "rgba8",
        })
    );
}

#[test]
fn cli_measure_reports_16_bit_png_color_types_and_channel_counts() {
    let directory = tempfile::tempdir().expect("temporary fixture directory");
    let input = directory.path().join("sixteen-bit.gltf");
    let fixtures = [
        ("l16", ExtendedColorType::L16, vec![0, 1], "l16", 1),
        ("la16", ExtendedColorType::La16, vec![0, 1, 0, 2], "la16", 2),
        (
            "rgb16",
            ExtendedColorType::Rgb16,
            vec![0, 1, 0, 2, 0, 3],
            "rgb16",
            3,
        ),
        (
            "rgba16",
            ExtendedColorType::Rgba16,
            vec![0, 1, 0, 2, 0, 3, 0, 4],
            "rgba16",
            4,
        ),
    ];
    for (name, color_type, pixels, _, _) in &fixtures {
        write_image(
            &input.with_file_name(format!("{name}.png")),
            &png(*color_type, pixels),
        );
    }
    let document = json!({
        "asset": { "version": "2.0" },
        "images": fixtures.iter().map(|(name, _, _, _, _)| {
            json!({ "name": name, "uri": format!("{name}.png") })
        }).collect::<Vec<_>>(),
    });
    std::fs::write(
        &input,
        serde_json::to_vec_pretty(&document).expect("serializes glTF"),
    )
    .expect("writes glTF fixture");

    let measurements = &measure(&input, "measure")["files"][0]["measurements"];
    assert_valid_measurements(measurements);
    let images = measurements["images"]
        .as_array()
        .expect("image measurements");
    for (index, (name, _, _, color_type, channel_count)) in fixtures.iter().enumerate() {
        assert_eq!(
            images[index],
            json!({
                "image_index": index,
                "name": name,
                "source_kind": "external",
                "detected_container": "png",
                "width": 1,
                "height": 1,
                "channel_count": channel_count,
                "decoded_color_type": color_type,
            })
        );
    }
}

/// Non-glTF loaders retain their explicit unavailable boundary instead of
/// presenting an empty material table as complete source evidence.
#[cfg(feature = "fbx")]
#[test]
fn cli_measure_marks_other_loader_material_resources_unavailable() {
    let input =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../animsmith-fbx/testdata/rigged_triangle.fbx");
    let report = measure(&input, "measure");
    let measurements = &report["files"][0]["measurements"];
    assert_eq!(measurements["material_resource_coverage"], "unavailable");
    assert_eq!(measurements["material_definitions"], json!([]));
    assert_eq!(measurements["textures"], json!([]));
    assert_eq!(measurements["images"], json!([]));
}
