//! Deterministic, bounded texture decoding and resizing for conversion recipes.

use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::imageops::{self, FilterType};
use image::{
    ExtendedColorType, GenericImageView, ImageEncoder, ImageFormat, ImageReader, Limits, Rgba,
    Rgba32FImage, RgbaImage,
};
use serde::Serialize;
use std::fmt;
use std::io::Cursor;

/// Largest encoded recipe image accepted before decoding.
pub(crate) const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Largest accepted source width.
pub(crate) const MAX_SOURCE_WIDTH: u32 = 4096;
/// Largest accepted source height.
pub(crate) const MAX_SOURCE_HEIGHT: u32 = 4096;
/// Smallest accepted recipe cap.
pub(crate) const MIN_MAX_DIMENSION: u32 = 1;
/// Largest accepted recipe cap.
pub(crate) const MAX_MAX_DIMENSION: u32 = 4096;
/// Decoder allocation limit, excluding allocator overhead.
pub(crate) const MAX_DECODE_ALLOC_BYTES: u64 = 192 * 1024 * 1024;

/// Exact image crate version in the recipe-texture producer closure.
pub(crate) const IMAGE_CODEC_VERSION: &str = "image@0.25.10";
/// Exact PNG codec version in the recipe-texture producer closure.
pub(crate) const PNG_CODEC_VERSION: &str = "png@0.18.1";
/// Exact JPEG codec version in the recipe-texture producer closure.
pub(crate) const JPEG_CODEC_VERSION: &str = "zune-jpeg@0.5.15";
/// Base-color resize contract.
pub(crate) const BASE_COLOR_ALGORITHM: &str = "sRGB-to-linear premultiplied-alpha Lanczos3";
/// Normal-map resize contract.
pub(crate) const NORMAL_ALGORITHM: &str = "tangent-vector Triangle renormalize +Z fallback";
/// Metallic-roughness and occlusion resize contract.
pub(crate) const DATA_ALGORITHM: &str = "linear-channel Triangle";
/// Canonical output encoding contract for resized textures.
pub(crate) const OUTPUT_ENCODING: &str = "PNG RGBA8 compression=Best filter=NoFilter";

/// Material role and semantic interpretation of a texture's RGB components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TextureRole {
    /// An sRGB base-color texture.
    BaseColor,
    /// A tangent-space normal map.
    Normal,
    /// A linear metallic-roughness map (roughness in G, metallic in B).
    MetallicRoughness,
    /// A linear occlusion map (occlusion in R).
    Occlusion,
}

/// Texture bytes and dimensions produced from one recipe image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessedImage {
    /// Original encoded bytes when no resize was necessary, otherwise canonical PNG bytes.
    pub(crate) bytes: Vec<u8>,
    /// MIME type inferred from the input magic.
    pub(crate) source_mime: &'static str,
    /// MIME type carried by `bytes`.
    pub(crate) output_mime: &'static str,
    /// Decoded source width and height.
    pub(crate) source_dimensions: [u32; 2],
    /// Emitted width and height.
    pub(crate) output_dimensions: [u32; 2],
    /// Whether pixels were resampled and bytes re-encoded.
    pub(crate) resized: bool,
}

/// A bounded image-processing failure suitable for a conversion operator error.
#[derive(Debug)]
pub(crate) enum TextureProcessError {
    /// The encoded input exceeds the recipe boundary.
    InputTooLarge { bytes: usize },
    /// The recipe's dimension cap is outside its supported range.
    InvalidMaxDimension { max_dimension: u32 },
    /// Bytes are not an accepted PNG or JPEG stream.
    UnsupportedFormat,
    /// The image decoder rejected malformed pixels or resource limits.
    Decode(image::ImageError),
    /// The decoded dimensions exceed the source boundary.
    SourceDimensionsTooLarge { width: u32, height: u32 },
    /// Checked image allocation or sizing arithmetic overflowed.
    SizeOverflow,
    /// Canonical PNG emission failed.
    Encode(image::ImageError),
}

impl fmt::Display for TextureProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge { bytes } => write!(
                f,
                "image is {bytes} bytes, exceeding the {} byte recipe limit",
                MAX_INPUT_BYTES
            ),
            Self::InvalidMaxDimension { max_dimension } => write!(
                f,
                "recipe max_dimension {max_dimension} must be in {MIN_MAX_DIMENSION}..={MAX_MAX_DIMENSION}"
            ),
            Self::UnsupportedFormat => {
                write!(f, "unsupported image format (expected PNG or JPEG magic)")
            }
            Self::Decode(error) => write!(f, "failed to decode PNG/JPEG image: {error}"),
            Self::SourceDimensionsTooLarge { width, height } => write!(
                f,
                "image dimensions {width}x{height} exceed the {MAX_SOURCE_WIDTH}x{MAX_SOURCE_HEIGHT} recipe limit"
            ),
            Self::SizeOverflow => write!(f, "image dimensions overflow recipe processing limits"),
            Self::Encode(error) => write!(f, "failed to encode resized PNG image: {error}"),
        }
    }
}

impl std::error::Error for TextureProcessError {}

/// Decode and, when necessary, deterministically resize one recipe image.
pub(crate) fn process_image(
    bytes: &[u8],
    max_dimension: u32,
    role: TextureRole,
) -> Result<ProcessedImage, TextureProcessError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(TextureProcessError::InputTooLarge { bytes: bytes.len() });
    }
    if !(MIN_MAX_DIMENSION..=MAX_MAX_DIMENSION).contains(&max_dimension) {
        return Err(TextureProcessError::InvalidMaxDimension { max_dimension });
    }

    let (format, source_mime) =
        input_format(bytes).ok_or(TextureProcessError::UnsupportedFormat)?;
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_WIDTH);
    limits.max_image_height = Some(MAX_SOURCE_HEIGHT);
    limits.max_alloc = Some(MAX_DECODE_ALLOC_BYTES);
    reader.limits(limits);
    // Always decode, including no-ops: a valid header with corrupt pixels is not accepted.
    let decoded = reader.decode().map_err(TextureProcessError::Decode)?;
    let (width, height) = decoded.dimensions();
    if width > MAX_SOURCE_WIDTH || height > MAX_SOURCE_HEIGHT {
        return Err(TextureProcessError::SourceDimensionsTooLarge { width, height });
    }
    let source_dimensions = [width, height];
    let (out_width, out_height) = target_dimensions(width, height, max_dimension)?;
    if [out_width, out_height] == source_dimensions {
        return Ok(ProcessedImage {
            bytes: bytes.to_vec(),
            source_mime,
            output_mime: source_mime,
            source_dimensions,
            output_dimensions: source_dimensions,
            resized: false,
        });
    }

    let rgba = decoded.into_rgba8();
    let resized = match role {
        TextureRole::BaseColor => resize_base_color(&rgba, out_width, out_height),
        TextureRole::Normal => resize_normal(&rgba, out_width, out_height),
        TextureRole::MetallicRoughness | TextureRole::Occlusion => {
            imageops::resize(&rgba, out_width, out_height, FilterType::Triangle)
        }
    };
    let output = encode_png(&resized)?;
    Ok(ProcessedImage {
        bytes: output,
        source_mime,
        output_mime: "image/png",
        source_dimensions,
        output_dimensions: [out_width, out_height],
        resized: true,
    })
}

fn input_format(bytes: &[u8]) -> Option<(ImageFormat, &'static str)> {
    const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.starts_with(PNG_MAGIC) {
        Some((ImageFormat::Png, "image/png"))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some((ImageFormat::Jpeg, "image/jpeg"))
    } else {
        None
    }
}

fn target_dimensions(
    width: u32,
    height: u32,
    max_dimension: u32,
) -> Result<(u32, u32), TextureProcessError> {
    let longest = width.max(height);
    if longest <= max_dimension {
        return Ok((width, height));
    }
    let scale = u64::from(max_dimension);
    let longest = u64::from(longest);
    let scaled = |dimension: u32| -> Result<u32, TextureProcessError> {
        let result = u64::from(dimension)
            .checked_mul(scale)
            .ok_or(TextureProcessError::SizeOverflow)?
            / longest;
        u32::try_from(result.max(1)).map_err(|_| TextureProcessError::SizeOverflow)
    };
    Ok((scaled(width)?, scaled(height)?))
}

fn resize_base_color(source: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    // `imageops::resize` explicitly assumes scene-linear input and premultiplied alpha.
    let linear = Rgba32FImage::from_fn(source.width(), source.height(), |x, y| {
        let [r, g, b, a] = source.get_pixel(x, y).0;
        let alpha = f32::from(a) / 255.0;
        Rgba([
            srgb_to_linear(r) * alpha,
            srgb_to_linear(g) * alpha,
            srgb_to_linear(b) * alpha,
            alpha,
        ])
    });
    let filtered = imageops::resize(&linear, width, height, FilterType::Lanczos3);
    RgbaImage::from_fn(width, height, |x, y| {
        let [r, g, b, alpha] = filtered.get_pixel(x, y).0;
        let alpha = alpha.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return Rgba([0, 0, 0, 0]);
        }
        Rgba([
            linear_to_srgb(r / alpha),
            linear_to_srgb(g / alpha),
            linear_to_srgb(b / alpha),
            quantize(alpha),
        ])
    })
}

fn resize_normal(source: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let vectors = Rgba32FImage::from_fn(source.width(), source.height(), |x, y| {
        let [r, g, b, a] = source.get_pixel(x, y).0;
        Rgba([
            unorm_to_normal(r),
            unorm_to_normal(g),
            unorm_to_normal(b),
            f32::from(a) / 255.0,
        ])
    });
    // RGBA32F applies Triangle independently to each component: RGB are vector
    // components and alpha is independently filtered scalar coverage.
    let filtered = imageops::resize(&vectors, width, height, FilterType::Triangle);
    RgbaImage::from_fn(width, height, |x, y| {
        let [x, y, z, alpha] = filtered.get_pixel(x, y).0;
        let length_squared = x * x + y * y + z * z;
        let [x, y, z] = if length_squared.is_finite() && length_squared > 1.0e-12 {
            let inverse_length = length_squared.sqrt().recip();
            [x * inverse_length, y * inverse_length, z * inverse_length]
        } else {
            [0.0, 0.0, 1.0]
        };
        Rgba([
            normal_to_unorm(x),
            normal_to_unorm(y),
            normal_to_unorm(z),
            quantize(alpha.clamp(0.0, 1.0)),
        ])
    })
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, TextureProcessError> {
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, CompressionType::Best, PngFilterType::NoFilter)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(TextureProcessError::Encode)?;
    Ok(bytes)
}

fn srgb_to_linear(component: u8) -> f32 {
    let value = f32::from(component) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let srgb = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    quantize(srgb)
}

fn unorm_to_normal(component: u8) -> f32 {
    f32::from(component) / 255.0 * 2.0 - 1.0
}

fn normal_to_unorm(component: f32) -> u8 {
    quantize((component * 0.5 + 0.5).clamp(0.0, 1.0))
}

fn quantize(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, pixels: Vec<u8>) -> Vec<u8> {
        let image = RgbaImage::from_raw(width, height, pixels).expect("valid RGBA fixture");
        encode_png(&image).expect("encodes PNG fixture")
    }

    fn jpeg(width: u32, height: u32, pixels: Vec<u8>) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95)
            .write_image(&pixels, width, height, ExtendedColorType::Rgb8)
            .expect("encodes JPEG fixture");
        bytes
    }

    fn decoded_png(bytes: &[u8]) -> RgbaImage {
        ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .expect("guesses PNG")
            .decode()
            .expect("decodes PNG")
            .into_rgba8()
    }

    #[test]
    fn rejects_limits_and_non_png_jpeg_magic() {
        assert!(matches!(
            process_image(b"not an image", 1, TextureRole::BaseColor),
            Err(TextureProcessError::UnsupportedFormat)
        ));
        assert!(matches!(
            process_image(&[], 0, TextureRole::BaseColor),
            Err(TextureProcessError::InvalidMaxDimension { .. })
        ));
        let too_large = vec![0; MAX_INPUT_BYTES + 1];
        assert!(matches!(
            process_image(&too_large, 1, TextureRole::BaseColor),
            Err(TextureProcessError::InputTooLarge { .. })
        ));

        // Complete 1x1 24-bit BMP: valid image bytes, deliberately outside
        // the recipe's PNG/JPEG allowlist.
        let bmp = [
            b'B', b'M', 58, 0, 0, 0, 0, 0, 0, 0, 54, 0, 0, 0, 40, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0,
            1, 0, 24, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 255, 0,
        ];
        assert!(matches!(
            process_image(&bmp, 1, TextureRole::BaseColor),
            Err(TextureProcessError::UnsupportedFormat)
        ));
    }

    #[test]
    fn no_op_preserves_exact_original_bytes_after_decode() {
        let source = png(1, 1, vec![7, 11, 13, 17]);
        let processed = process_image(&source, 1, TextureRole::BaseColor).expect("processes");
        assert_eq!(processed.bytes, source);
        assert_eq!(processed.source_mime, "image/png");
        assert_eq!(processed.output_mime, "image/png");
        assert_eq!(processed.source_dimensions, [1, 1]);
        assert_eq!(processed.output_dimensions, [1, 1]);
        assert!(!processed.resized);
    }

    #[test]
    fn jpeg_no_op_preserves_exact_original_bytes_and_mime() {
        let source = jpeg(1, 1, vec![7, 11, 13]);
        let processed = process_image(&source, 1, TextureRole::BaseColor).expect("processes");
        assert_eq!(processed.bytes, source);
        assert_eq!(processed.source_mime, "image/jpeg");
        assert_eq!(processed.output_mime, "image/jpeg");
        assert_eq!(processed.source_dimensions, [1, 1]);
        assert_eq!(processed.output_dimensions, [1, 1]);
        assert!(!processed.resized);
    }

    #[test]
    fn jpeg_resize_uses_the_declared_dimensions_and_png_output() {
        let source = jpeg(5, 3, [12, 34, 56].repeat(15));
        let processed = process_image(&source, 2, TextureRole::BaseColor).expect("resizes JPEG");
        assert_eq!(processed.source_mime, "image/jpeg");
        assert_eq!(processed.source_dimensions, [5, 3]);
        assert_eq!(processed.output_mime, "image/png");
        assert_eq!(processed.output_dimensions, [2, 1]);
        assert_eq!(decoded_png(&processed.bytes).dimensions(), (2, 1));
        assert!(processed.resized);
    }

    #[test]
    fn floor_aspect_ratio_dimensions_are_pinned() {
        let source = png(5, 3, [1, 2, 3, 255].repeat(15));
        let processed = process_image(&source, 2, TextureRole::BaseColor).expect("resizes");
        assert_eq!(processed.output_dimensions, [2, 1]);
        assert!(processed.resized);
    }

    #[test]
    fn base_color_filters_in_linear_light() {
        let source = png(2, 1, vec![0, 0, 0, 255, 255, 255, 255, 255]);
        let processed = process_image(&source, 1, TextureRole::BaseColor).expect("resizes");
        let pixel = decoded_png(&processed.bytes).get_pixel(0, 0).0;
        assert!(
            (187..=189).contains(&pixel[0]),
            "linear-light midpoint: {pixel:?}"
        );
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
    }

    #[test]
    fn base_color_filtering_uses_premultiplied_alpha() {
        let source = png(2, 1, vec![255, 255, 255, 255, 255, 0, 0, 0]);
        let processed = process_image(&source, 1, TextureRole::BaseColor).expect("resizes");
        let pixel = decoded_png(&processed.bytes).get_pixel(0, 0).0;
        assert_eq!(pixel, [255, 255, 255, 128]);
    }

    #[test]
    fn normal_resize_renormalizes_and_falls_back_to_positive_z() {
        let source = png(2, 1, vec![255, 128, 128, 255, 128, 255, 128, 255]);
        let processed = process_image(&source, 1, TextureRole::Normal).expect("resizes");
        let pixel = decoded_png(&processed.bytes).get_pixel(0, 0).0;
        assert!((216..=220).contains(&pixel[0]), "normalized X: {pixel:?}");
        assert!((216..=220).contains(&pixel[1]), "normalized Y: {pixel:?}");

        let cancelling = png(2, 1, vec![0, 128, 128, 255, 255, 127, 127, 255]);
        let processed = process_image(&cancelling, 1, TextureRole::Normal).expect("resizes");
        assert_eq!(
            decoded_png(&processed.bytes).get_pixel(0, 0).0,
            [128, 128, 255, 255]
        );
    }

    #[test]
    fn resized_png_bytes_are_deterministic() {
        let source = png(
            3,
            1,
            vec![12, 34, 56, 255, 78, 90, 123, 255, 210, 111, 45, 255],
        );
        let first = process_image(&source, 1, TextureRole::BaseColor).expect("first process");
        let second = process_image(&source, 1, TextureRole::BaseColor).expect("second process");
        assert_eq!(first.bytes, second.bytes);
    }
}
