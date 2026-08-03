//! Source material-resource records are loader facts: source indices and
//! optional names survive even when no mesh uses them, and image-inspection
//! failures stay explicit without changing writer-facing texture bytes.

use animsmith_core::model::{
    DecodedImageColorType, ImageContainerFormat, ImageSourceKind, ImageUnavailableReason,
    MaterialTextureSlot, SourceImageInspection,
};
use base64::Engine as _;
use image::{ExtendedColorType, ImageEncoder, codecs::jpeg::JpegEncoder, codecs::png::PngEncoder};

fn png(color_type: ExtendedColorType, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(pixels, 1, 1, color_type)
        .expect("encodes PNG");
    bytes
}

fn rgba_png() -> Vec<u8> {
    png(ExtendedColorType::Rgba8, &[10, 20, 30, 40])
}

fn grayscale_png() -> Vec<u8> {
    png(ExtendedColorType::L8, &[127])
}

fn jpeg() -> Vec<u8> {
    let mut bytes = Vec::new();
    JpegEncoder::new(&mut bytes)
        .encode(&[10, 20, 30], 1, 1, ExtendedColorType::Rgb8)
        .expect("encodes JPEG");
    bytes
}

fn png_with_dimensions(mut bytes: Vec<u8>, width: u32, height: u32) -> Vec<u8> {
    // Rewrite the IHDR dimensions and checksum while retaining a tiny encoded
    // payload. The decoder must reject the declared decoded allocation before
    // it tries to consume that payload.
    bytes[16..20].copy_from_slice(&width.to_be_bytes());
    bytes[20..24].copy_from_slice(&height.to_be_bytes());
    let crc = crc32(&bytes[12..29]);
    bytes[29..33].copy_from_slice(&crc.to_be_bytes());
    bytes
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}

#[test]
fn load_reports_source_order_bindings_and_bounded_image_inspection() {
    let dir = tempfile::tempdir().unwrap();
    let external = grayscale_png();
    std::fs::write(dir.path().join("external-gray.png"), &external).unwrap();
    let rgba = rgba_png();
    let jpeg = jpeg();
    let rgba_b64 = base64::engine::general_purpose::STANDARD.encode(&rgba);
    let jpeg_b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg);
    // Valid container magic followed by truncated content exercises
    // DecodeFailed instead of collapsing the row as absent.
    let corrupt_b64 = base64::engine::general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1a\ncorrupt");
    let unsupported_b64 = base64::engine::general_purpose::STANDARD.encode(b"GIF89a");
    let path = dir.path().join("materials.gltf");
    std::fs::write(
        &path,
        format!(
            r#"{{
                "asset": {{ "version": "2.0" }},
                "images": [
                    {{ "name": "rgba-data", "uri": "data:image/png;base64,{rgba_b64}" }},
                    {{ "name": "external-gray", "uri": "external-gray.png", "mimeType": "image/png" }},
                    {{ "name": "jpeg-data", "uri": "data:image/jpeg;base64,{jpeg_b64}" }},
                    {{ "name": "corrupt", "uri": "data:image/png;base64,{corrupt_b64}" }},
                    {{ "name": "invalid", "uri": "data:image/png;base64,%%%" }},
                    {{ "name": "missing", "uri": "missing.png", "mimeType": "image/png" }},
                    {{ "name": "unsupported", "uri": "data:application/octet-stream;base64,{unsupported_b64}" }}
                ],
                "textures": [
                    {{ "name": "shared-rgba", "source": 0 }},
                    {{ "name": "normal-gray", "source": 1 }},
                    {{ "name": "metal-jpeg", "source": 2 }},
                    {{ "name": "occlusion-corrupt", "source": 3 }},
                    {{ "name": "invalid-uri", "source": 4 }},
                    {{ "name": "missing-external", "source": 5 }},
                    {{ "name": "unsupported-image", "source": 6 }}
                ],
                "materials": [
                    {{}},
                    {{
                        "name": "all-slots",
                        "pbrMetallicRoughness": {{
                            "baseColorTexture": {{ "index": 0 }},
                            "metallicRoughnessTexture": {{ "index": 2 }}
                        }},
                        "normalTexture": {{ "index": 1, "scale": 0.5 }},
                        "occlusionTexture": {{ "index": 3, "strength": 0.75 }}
                    }},
                    {{ "name": "shares-rgba", "pbrMetallicRoughness": {{ "baseColorTexture": {{ "index": 0 }} }} }}
                ]
            }}"#
        ),
    )
    .unwrap();

    let document = animsmith_gltf::load(&path).expect("source rows load");
    let resources = &document.assets.material_resources;
    assert_eq!(
        resources.materials.len(),
        3,
        "untextured rows remain visible"
    );
    assert_eq!(
        resources.textures.len(),
        7,
        "unreferenced texture rows remain visible"
    );
    assert_eq!(
        resources.images.len(),
        7,
        "unreferenced image rows remain visible"
    );
    assert_eq!(
        resources
            .materials
            .iter()
            .map(|material| (material.material_index, material.name.as_deref()))
            .collect::<Vec<_>>(),
        vec![(0, None), (1, Some("all-slots")), (2, Some("shares-rgba"))]
    );
    assert_eq!(
        resources.materials[1]
            .texture_bindings
            .iter()
            .map(|binding| (binding.slot, binding.texture_index))
            .collect::<Vec<_>>(),
        vec![
            (MaterialTextureSlot::BaseColor, 0),
            (MaterialTextureSlot::Normal, 1),
            (MaterialTextureSlot::MetallicRoughness, 2),
            (MaterialTextureSlot::Occlusion, 3),
        ],
        "bindings use semantic slot order, not incidental JSON order"
    );
    assert_eq!(resources.materials[2].texture_bindings[0].texture_index, 0);
    assert_eq!(
        resources
            .textures
            .iter()
            .map(|texture| (
                texture.texture_index,
                texture.image_index,
                texture.name.as_deref()
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, 0, Some("shared-rgba")),
            (1, 1, Some("normal-gray")),
            (2, 2, Some("metal-jpeg")),
            (3, 3, Some("occlusion-corrupt")),
            (4, 4, Some("invalid-uri")),
            (5, 5, Some("missing-external")),
            (6, 6, Some("unsupported-image")),
        ]
    );

    assert_eq!(resources.images[0].source_kind, ImageSourceKind::DataUri);
    assert_eq!(
        resources.images[0].declared_mime_type.as_deref(),
        Some("image/png")
    );
    assert_eq!(
        resources.images[0].detected_container,
        Some(ImageContainerFormat::Png)
    );
    assert_eq!(
        resources.images[0].inspection,
        SourceImageInspection::Available {
            width: 1,
            height: 1,
            channel_count: 4,
            color_type: DecodedImageColorType::Rgba8,
        }
    );
    assert_eq!(resources.images[1].source_kind, ImageSourceKind::External);
    assert_eq!(
        resources.images[1].inspection,
        SourceImageInspection::Available {
            width: 1,
            height: 1,
            channel_count: 1,
            color_type: DecodedImageColorType::L8,
        }
    );
    assert_eq!(
        resources.images[2].detected_container,
        Some(ImageContainerFormat::Jpeg)
    );
    assert!(matches!(
        resources.images[2].inspection,
        SourceImageInspection::Available { .. }
    ));
    for (image, reason) in [
        (3, ImageUnavailableReason::DecodeFailed),
        (4, ImageUnavailableReason::InvalidDataUri),
        (5, ImageUnavailableReason::SourceUnavailable),
        (6, ImageUnavailableReason::UnsupportedContainer),
    ] {
        assert_eq!(
            resources.images[image].inspection,
            SourceImageInspection::Unavailable { reason },
            "image {image} preserves its explicit reason"
        );
    }

    // Writer-facing slots still retain resolvable raw source bytes even when
    // source-image inspection rejected their corrupt payload.
    assert_eq!(
        document.assets.materials[1]
            .base_color_texture
            .as_ref()
            .expect("resolved data URI remains writable")
            .bytes,
        rgba
    );
    assert!(
        document.assets.materials[1]
            .occlusion_texture
            .as_ref()
            .is_some_and(|texture| !texture.texture.bytes.is_empty())
    );

    // Resource records never retain external paths; a host-specific temp
    // directory must not become observable through their Debug representation.
    assert!(!format!("{resources:#?}").contains(&dir.path().display().to_string()));
}

#[test]
fn load_reports_buffer_view_image_as_embedded() {
    let dir = tempfile::tempdir().unwrap();
    let png = rgba_png();
    let png_b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    let path = dir.path().join("embedded-image.gltf");
    std::fs::write(
        &path,
        format!(
            r#"{{
                "asset": {{ "version": "2.0" }},
                "buffers": [{{ "byteLength": {}, "uri": "data:application/octet-stream;base64,{png_b64}" }}],
                "bufferViews": [{{ "buffer": 0, "byteOffset": 0, "byteLength": {} }}],
                "images": [{{ "name": "embedded-rgba", "bufferView": 0, "mimeType": "image/png" }}],
                "textures": [{{ "source": 0 }}],
                "materials": [{{ "pbrMetallicRoughness": {{ "baseColorTexture": {{ "index": 0 }} }} }}]
            }}"#,
            png.len(),
            png.len(),
        ),
    )
    .unwrap();

    let document = animsmith_gltf::load(&path).expect("embedded image loads");
    let image = &document.assets.material_resources.images[0];
    assert_eq!(image.source_kind, ImageSourceKind::Embedded);
    assert_eq!(image.name.as_deref(), Some("embedded-rgba"));
    assert_eq!(image.declared_mime_type.as_deref(), Some("image/png"));
    assert!(matches!(
        image.inspection,
        SourceImageInspection::Available {
            color_type: DecodedImageColorType::Rgba8,
            ..
        }
    ));
    assert_eq!(
        document.assets.materials[0]
            .base_color_texture
            .as_ref()
            .expect("embedded writer texture")
            .bytes,
        png
    );
}

#[test]
fn load_reports_embedded_png_channel_variants() {
    let encoded_images = [
        png(ExtendedColorType::L8, &[1]),
        png(ExtendedColorType::La8, &[1, 2]),
        png(ExtendedColorType::Rgb8, &[1, 2, 3]),
        png(ExtendedColorType::Rgba8, &[1, 2, 3, 4]),
    ];
    let mut bytes = Vec::new();
    let mut views = Vec::new();
    for image in &encoded_images {
        let offset = bytes.len();
        bytes.extend_from_slice(image);
        views.push(serde_json::json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": image.len(),
        }));
    }
    let images = views
        .iter()
        .enumerate()
        .map(|(index, _)| {
            serde_json::json!({
                "name": format!("embedded-{index}"),
                "bufferView": index,
                "mimeType": "image/png",
            })
        })
        .collect::<Vec<_>>();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let fixture = serde_json::json!({
        "asset": { "version": "2.0" },
        "buffers": [{
            "byteLength": bytes.len(),
            "uri": format!("data:application/octet-stream;base64,{encoded}"),
        }],
        "bufferViews": views,
        "images": images,
    });
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("embedded-channel-variants.gltf");
    std::fs::write(&path, serde_json::to_vec_pretty(&fixture).unwrap()).unwrap();

    let document = animsmith_gltf::load(&path).expect("embedded variants load");
    let actual = document
        .assets
        .material_resources
        .images
        .iter()
        .map(|image| {
            assert_eq!(image.source_kind, ImageSourceKind::Embedded);
            image.inspection
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            SourceImageInspection::Available {
                width: 1,
                height: 1,
                channel_count: 1,
                color_type: DecodedImageColorType::L8,
            },
            SourceImageInspection::Available {
                width: 1,
                height: 1,
                channel_count: 2,
                color_type: DecodedImageColorType::La8,
            },
            SourceImageInspection::Available {
                width: 1,
                height: 1,
                channel_count: 3,
                color_type: DecodedImageColorType::Rgb8,
            },
            SourceImageInspection::Available {
                width: 1,
                height: 1,
                channel_count: 4,
                color_type: DecodedImageColorType::Rgba8,
            },
        ]
    );
}

#[test]
fn load_reports_wide_image_metadata_without_dimension_policy() {
    // A 4097×1 grayscale PNG is cheap to decode and must remain evidence;
    // consumers, not animsmith, choose acceptable dimensions.
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(&vec![0; 4097], 4097, 1, ExtendedColorType::L8)
        .expect("encodes wide PNG");
    let encoded = base64::engine::general_purpose::STANDARD.encode(png);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wide.gltf");
    std::fs::write(
        &path,
        format!(
            r#"{{
                "asset": {{ "version": "2.0" }},
                "images": [{{ "uri": "data:image/png;base64,{encoded}" }}]
            }}"#
        ),
    )
    .unwrap();

    let document = animsmith_gltf::load(&path).expect("wide source loads");
    assert_eq!(document.assets.material_resources.images.len(), 1);
    assert_eq!(
        document.assets.material_resources.images[0].inspection,
        SourceImageInspection::Available {
            width: 4097,
            height: 1,
            channel_count: 1,
            color_type: DecodedImageColorType::L8,
        }
    );
    assert_eq!(
        document.assets.material_resources.images[0].detected_container,
        Some(ImageContainerFormat::Png)
    );
}

#[test]
fn load_reports_decoder_allocation_limit_without_large_fixture() {
    // The encoded PNG is a one-pixel RGBA file, but its valid IHDR advertises a
    // decoded RGBA allocation far above the 192 MiB decoder budget.
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_with_dimensions(
        png(ExtendedColorType::Rgba8, &[0, 0, 0, 255]),
        16_384,
        16_384,
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("allocation-limit.gltf");
    std::fs::write(
        &path,
        format!(
            r#"{{
                "asset": {{ "version": "2.0" }},
                "images": [{{ "uri": "data:image/png;base64,{encoded}" }}]
            }}"#
        ),
    )
    .unwrap();

    let document = animsmith_gltf::load(&path).expect("oversized source still loads");
    let image = &document.assets.material_resources.images[0];
    assert_eq!(image.detected_container, Some(ImageContainerFormat::Png));
    assert_eq!(
        image.inspection,
        SourceImageInspection::Unavailable {
            reason: ImageUnavailableReason::ResourceLimit,
        }
    );
}

#[test]
fn load_marks_percent_escaped_unsafe_external_image_uris_unavailable() {
    let root = tempfile::tempdir().unwrap();
    let input_dir = root.path().join("input");
    std::fs::create_dir(&input_dir).unwrap();
    std::fs::write(root.path().join("escape.png"), rgba_png()).unwrap();
    std::fs::create_dir(input_dir.join("nested")).unwrap();
    std::fs::write(input_dir.join("nested/albedo.png"), rgba_png()).unwrap();
    let path = input_dir.join("unsafe-image-uri.gltf");
    std::fs::write(
        &path,
        r#"{
            "asset": { "version": "2.0" },
            "images": [
                { "uri": "%2e%2e/escape.png" },
                { "uri": "%2Fetc%2Fpasswd" },
                { "uri": "nested%2Falbedo.png" },
                { "uri": "nested%5Calbedo.png" },
                { "uri": "nested%00albedo.png" },
                { "uri": "nested%2albedo.png" },
                { "uri": "nested%GGalbedo.png" }
            ]
        }"#,
    )
    .unwrap();

    let document = animsmith_gltf::load(&path).expect("unsafe image URIs stay measurable");
    for image in &document.assets.material_resources.images {
        assert_eq!(image.source_kind, ImageSourceKind::External);
        assert_eq!(
            image.inspection,
            SourceImageInspection::Unavailable {
                reason: ImageUnavailableReason::SourceUnavailable,
            },
            "unsafe external URI must not read a resource"
        );
    }
}
