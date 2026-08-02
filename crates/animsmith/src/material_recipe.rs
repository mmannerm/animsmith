//! Declarative material-texture recipes for `convert`.
//!
//! This module owns recipe parsing, exact material-name matching, and the
//! recipe-relative filesystem boundary.  It deliberately prepares every
//! texture before cloning and updating the document, so a failed recipe never
//! returns a partially updated document.

use crate::texture_processing::{
    BASE_COLOR_ALGORITHM, IMAGE_CODEC_VERSION, JPEG_CODEC_VERSION, MAX_INPUT_BYTES,
    MAX_MAX_DIMENSION, MIN_MAX_DIMENSION, NORMAL_ALGORITHM, OUTPUT_ENCODING, PNG_CODEC_VERSION,
    ProcessedImage, TextureRole, process_image,
};
use animsmith_core::Document;
use animsmith_core::model::{NormalTextureAsset, TextureAsset};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Component, Path, PathBuf};

const RECIPE_SCHEMA_VERSION: u32 = 1;
const RECIPE_SCHEMA_ID: &str = "urn:animsmith:schema:material-texture-recipe:1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialTextureRecipe {
    schema_version: u32,
    schema: String,
    #[serde(default)]
    texture_root: Option<PathBuf>,
    max_dimension: u32,
    materials: Vec<MaterialTextureRecipeMaterial>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialTextureRecipeMaterial {
    name: String,
    base_color: PathBuf,
    normal: PathBuf,
}

/// A fully prepared recipe application and stable provenance evidence.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct MaterialTextureRecipeApplication {
    /// A clone of the input document with every recipe texture embedded.
    #[serde(skip)]
    pub(crate) document: Document,
    /// Recipe and texture-consumption provenance.
    pub(crate) evidence: MaterialTextureRecipeEvidence,
}

/// Machine-readable evidence emitted for a successful recipe application.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct MaterialTextureRecipeEvidence {
    /// Recipe schema identity.
    pub(crate) schema: &'static str,
    /// Recipe schema version.
    pub(crate) schema_version: u32,
    /// The operator-declared recipe path, never a canonical host path.
    pub(crate) path: String,
    /// The operator-declared root, when one constrains texture paths.
    pub(crate) texture_root: Option<String>,
    /// Largest allowed output dimension.
    pub(crate) max_dimension: u32,
    /// Pinned texture processor implementation details.
    pub(crate) processor: MaterialTextureProcessorEvidence,
    /// Inputs consumed in source-material, then slot order.
    pub(crate) consumed_inputs: Vec<MaterialTextureConsumedInput>,
    /// Embedded textures emitted in source-material, then slot order.
    pub(crate) emitted_textures: Vec<MaterialTextureEmittedTexture>,
}

/// The pinned image-decoding, resize, and encoding implementation.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct MaterialTextureProcessorEvidence {
    /// Version of the image crate in the producer dependency closure.
    pub(crate) image_crate: &'static str,
    /// Version of the PNG codec crate in the producer dependency closure.
    pub(crate) png_crate: &'static str,
    /// Version of the JPEG codec crate in the producer dependency closure.
    pub(crate) jpeg_crate: &'static str,
    /// Base-color resize behavior.
    pub(crate) base_color_algorithm: &'static str,
    /// Normal-map resize behavior.
    pub(crate) normal_algorithm: &'static str,
    /// Deterministic output encoding behavior.
    pub(crate) output_encoding: &'static str,
}

/// One recipe texture that was read and decoded.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct MaterialTextureConsumedInput {
    /// Stable source-material array index.
    pub(crate) material_index: usize,
    /// Source material name.
    pub(crate) material_name: String,
    /// Texture role on the material.
    pub(crate) slot: TextureRole,
    /// Path exactly as declared in the recipe, never canonicalized.
    pub(crate) declared_path: String,
    /// Detected source image MIME type.
    pub(crate) mime: String,
    /// Decoded source width and height.
    pub(crate) dimensions: [u32; 2],
}

/// One embedded texture produced from a recipe texture.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct MaterialTextureEmittedTexture {
    /// Stable source-material array index.
    pub(crate) material_index: usize,
    /// Source material name.
    pub(crate) material_name: String,
    /// Texture role on the material.
    pub(crate) slot: TextureRole,
    /// Path exactly as declared in the recipe, never canonicalized.
    pub(crate) declared_path: String,
    /// MIME type of the embedded output.
    pub(crate) mime: String,
    /// Output width and height.
    pub(crate) dimensions: [u32; 2],
    /// Whether processing reduced either dimension.
    pub(crate) resized: bool,
    /// Number of embedded encoded bytes.
    pub(crate) emitted_bytes: usize,
}

/// A recipe, mapping, path, or image-processing failure.
#[derive(Debug)]
pub(crate) enum MaterialTextureRecipeError {
    /// The recipe could not be read or parsed.
    Recipe(String),
    /// A declared path crosses the recipe's configured filesystem boundary.
    UnsafePath { path: String, reason: &'static str },
    /// A declared root is not an existing directory.
    InvalidRoot { path: String },
    /// A texture is missing, not a regular file, or cannot be read.
    TextureFile { path: String, reason: String },
    /// A texture exceeds the per-input limit.
    TextureTooLarge { path: String, bytes: u64 },
    /// The recipe repeats a material name.
    DuplicateRecipeMaterial { name: String },
    /// The input has multiple material records with the same name.
    AmbiguousSourceMaterial { name: String },
    /// A recipe entry names no source material.
    UnusedRecipeMaterial { name: String },
    /// A source material has no recipe entry.
    MissingMaterialMapping { name: String },
    /// Image processing rejected a supported-path input.
    Image { path: String, reason: String },
}

impl fmt::Display for MaterialTextureRecipeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recipe(reason) => write!(f, "invalid material texture recipe: {reason}"),
            Self::UnsafePath { path, reason } => {
                write!(f, "unsafe texture path {path:?}: {reason}")
            }
            Self::InvalidRoot { path } => {
                write!(f, "texture root {path:?} is not an existing directory")
            }
            Self::TextureFile { path, reason } => {
                write!(f, "cannot use texture file {path:?}: {reason}")
            }
            Self::TextureTooLarge { path, bytes } => write!(
                f,
                "texture file {path:?} is {bytes} bytes, exceeding the 64 MiB limit"
            ),
            Self::DuplicateRecipeMaterial { name } => {
                write!(f, "recipe contains duplicate material entry {name:?}")
            }
            Self::AmbiguousSourceMaterial { name } => {
                write!(f, "source contains multiple materials named {name:?}")
            }
            Self::UnusedRecipeMaterial { name } => write!(
                f,
                "recipe material entry {name:?} matches no source material"
            ),
            Self::MissingMaterialMapping { name } => {
                write!(f, "source material {name:?} has no recipe entry")
            }
            Self::Image { path, reason } => write!(f, "cannot process texture {path:?}: {reason}"),
        }
    }
}

impl std::error::Error for MaterialTextureRecipeError {}

/// Parse, validate, and atomically apply a material-texture recipe.
///
/// All recipe entries are matched case-sensitively and must cover the loaded
/// document's material records exactly once. Image reads and processing finish
/// before this function clones the document, so an error cannot expose a
/// partial material update.
pub(crate) fn apply_material_texture_recipe(
    recipe_path: &Path,
    doc: &Document,
) -> Result<MaterialTextureRecipeApplication, MaterialTextureRecipeError> {
    let recipe_text = std::fs::read_to_string(recipe_path).map_err(|error| {
        MaterialTextureRecipeError::Recipe(format!(
            "cannot read {}: {error}",
            recipe_path.display()
        ))
    })?;
    let recipe: MaterialTextureRecipe = toml::from_str(&recipe_text)
        .map_err(|error| MaterialTextureRecipeError::Recipe(error.to_string()))?;
    validate_recipe_header(&recipe)?;
    validate_recipe_names(&recipe)?;

    let source_indices = source_material_indices(doc)?;
    for entry in &recipe.materials {
        if !source_indices.contains_key(entry.name.as_str()) {
            return Err(MaterialTextureRecipeError::UnusedRecipeMaterial {
                name: entry.name.clone(),
            });
        }
    }
    let entries: BTreeMap<&str, &MaterialTextureRecipeMaterial> = recipe
        .materials
        .iter()
        .map(|entry| (entry.name.as_str(), entry))
        .collect();
    for material in &doc.assets.materials {
        if !entries.contains_key(material.name.as_str()) {
            return Err(MaterialTextureRecipeError::MissingMaterialMapping {
                name: material.name.clone(),
            });
        }
    }

    let recipe_base = recipe_path.parent().unwrap_or_else(|| Path::new("."));
    let root = resolve_root(recipe_base, recipe.texture_root.as_deref())?;
    let mut prepared = Vec::with_capacity(doc.assets.materials.len() * 2);
    for (material_index, material) in doc.assets.materials.iter().enumerate() {
        let entry = entries[material.name.as_str()];
        prepared.push(prepare_texture(
            recipe_base,
            root.as_deref(),
            &entry.base_color,
            material_index,
            &material.name,
            TextureRole::BaseColor,
            recipe.max_dimension,
        )?);
        prepared.push(prepare_texture(
            recipe_base,
            root.as_deref(),
            &entry.normal,
            material_index,
            &material.name,
            TextureRole::Normal,
            recipe.max_dimension,
        )?);
    }

    let mut updated = doc.clone();
    let mut consumed_inputs = Vec::with_capacity(prepared.len());
    let mut emitted_textures = Vec::with_capacity(prepared.len());
    for texture in prepared {
        let asset = TextureAsset {
            bytes: texture.image.bytes.clone(),
            mime: texture.image.output_mime.to_owned(),
        };
        let material = &mut updated.assets.materials[texture.material_index];
        match texture.slot {
            TextureRole::BaseColor => {
                material.base_color = [1.0, 1.0, 1.0, 1.0];
                material.base_color_texture = Some(asset);
            }
            TextureRole::Normal => {
                material.normal_texture = Some(NormalTextureAsset {
                    texture: asset,
                    scale: 1.0,
                });
            }
        }
        consumed_inputs.push(MaterialTextureConsumedInput {
            material_index: texture.material_index,
            material_name: texture.material_name.clone(),
            slot: texture.slot,
            declared_path: texture.declared_path.clone(),
            mime: texture.image.source_mime.to_owned(),
            dimensions: texture.image.source_dimensions,
        });
        emitted_textures.push(MaterialTextureEmittedTexture {
            material_index: texture.material_index,
            material_name: texture.material_name,
            slot: texture.slot,
            declared_path: texture.declared_path,
            mime: texture.image.output_mime.to_owned(),
            dimensions: texture.image.output_dimensions,
            resized: texture.image.resized,
            emitted_bytes: texture.image.bytes.len(),
        });
    }

    Ok(MaterialTextureRecipeApplication {
        document: updated,
        evidence: MaterialTextureRecipeEvidence {
            schema: RECIPE_SCHEMA_ID,
            schema_version: RECIPE_SCHEMA_VERSION,
            path: recipe_path.display().to_string(),
            texture_root: recipe.texture_root.map(|path| path.display().to_string()),
            max_dimension: recipe.max_dimension,
            processor: MaterialTextureProcessorEvidence {
                image_crate: IMAGE_CODEC_VERSION,
                png_crate: PNG_CODEC_VERSION,
                jpeg_crate: JPEG_CODEC_VERSION,
                base_color_algorithm: BASE_COLOR_ALGORITHM,
                normal_algorithm: NORMAL_ALGORITHM,
                output_encoding: OUTPUT_ENCODING,
            },
            consumed_inputs,
            emitted_textures,
        },
    })
}

fn validate_recipe_header(
    recipe: &MaterialTextureRecipe,
) -> Result<(), MaterialTextureRecipeError> {
    if recipe.schema_version != RECIPE_SCHEMA_VERSION {
        return Err(MaterialTextureRecipeError::Recipe(format!(
            "unsupported schema_version {}; expected {RECIPE_SCHEMA_VERSION}",
            recipe.schema_version
        )));
    }
    if recipe.schema != RECIPE_SCHEMA_ID {
        return Err(MaterialTextureRecipeError::Recipe(format!(
            "unsupported schema {:?}; expected {RECIPE_SCHEMA_ID:?}",
            recipe.schema
        )));
    }
    if !(MIN_MAX_DIMENSION..=MAX_MAX_DIMENSION).contains(&recipe.max_dimension) {
        return Err(MaterialTextureRecipeError::Recipe(format!(
            "max_dimension must be in {MIN_MAX_DIMENSION}..={MAX_MAX_DIMENSION}"
        )));
    }
    if recipe.materials.is_empty() {
        return Err(MaterialTextureRecipeError::Recipe(
            "materials must contain at least one entry".into(),
        ));
    }
    Ok(())
}

fn validate_recipe_names(recipe: &MaterialTextureRecipe) -> Result<(), MaterialTextureRecipeError> {
    let mut names = BTreeSet::new();
    for entry in &recipe.materials {
        if entry.name.is_empty() {
            return Err(MaterialTextureRecipeError::Recipe(
                "material names must not be empty".into(),
            ));
        }
        if !names.insert(&entry.name) {
            return Err(MaterialTextureRecipeError::DuplicateRecipeMaterial {
                name: entry.name.clone(),
            });
        }
    }
    Ok(())
}

fn source_material_indices(
    doc: &Document,
) -> Result<BTreeMap<&str, usize>, MaterialTextureRecipeError> {
    let mut indices = BTreeMap::new();
    for (index, material) in doc.assets.materials.iter().enumerate() {
        if indices.insert(material.name.as_str(), index).is_some() {
            return Err(MaterialTextureRecipeError::AmbiguousSourceMaterial {
                name: material.name.clone(),
            });
        }
    }
    Ok(indices)
}

fn resolve_root(
    base: &Path,
    declared_root: Option<&Path>,
) -> Result<Option<PathBuf>, MaterialTextureRecipeError> {
    let Some(root) = declared_root else {
        return Ok(None);
    };
    validate_declared_path(root, true)?;
    let joined = base.join(root);
    let canonical =
        std::fs::canonicalize(&joined).map_err(|_| MaterialTextureRecipeError::InvalidRoot {
            path: root.display().to_string(),
        })?;
    if !std::fs::metadata(&canonical)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
    {
        return Err(MaterialTextureRecipeError::InvalidRoot {
            path: root.display().to_string(),
        });
    }
    Ok(Some(canonical))
}

fn prepare_texture(
    recipe_base: &Path,
    root: Option<&Path>,
    declared_path: &Path,
    material_index: usize,
    material_name: &str,
    slot: TextureRole,
    max_dimension: u32,
) -> Result<PreparedTexture, MaterialTextureRecipeError> {
    validate_declared_path(declared_path, root.is_some())?;
    let joined = match root {
        Some(root) => root.join(declared_path),
        None => recipe_base.join(declared_path),
    };
    let canonical = std::fs::canonicalize(&joined).map_err(|error| {
        MaterialTextureRecipeError::TextureFile {
            path: declared_path.display().to_string(),
            reason: error.to_string(),
        }
    })?;
    if let Some(root) = root
        && !canonical.starts_with(root)
    {
        return Err(MaterialTextureRecipeError::UnsafePath {
            path: declared_path.display().to_string(),
            reason: "resolves outside declared texture_root",
        });
    }
    let metadata =
        std::fs::metadata(&canonical).map_err(|error| MaterialTextureRecipeError::TextureFile {
            path: declared_path.display().to_string(),
            reason: error.to_string(),
        })?;
    if !metadata.is_file() {
        return Err(MaterialTextureRecipeError::TextureFile {
            path: declared_path.display().to_string(),
            reason: "expected a regular file".into(),
        });
    }
    if metadata.len() > MAX_INPUT_BYTES as u64 {
        return Err(MaterialTextureRecipeError::TextureTooLarge {
            path: declared_path.display().to_string(),
            bytes: metadata.len(),
        });
    }
    let bytes =
        std::fs::read(&canonical).map_err(|error| MaterialTextureRecipeError::TextureFile {
            path: declared_path.display().to_string(),
            reason: error.to_string(),
        })?;
    let image = process_image(&bytes, max_dimension, slot).map_err(|error| {
        MaterialTextureRecipeError::Image {
            path: declared_path.display().to_string(),
            reason: error.to_string(),
        }
    })?;
    Ok(PreparedTexture {
        material_index,
        material_name: material_name.to_owned(),
        slot,
        declared_path: declared_path.display().to_string(),
        image,
    })
}

fn validate_declared_path(
    path: &Path,
    reject_parent: bool,
) -> Result<(), MaterialTextureRecipeError> {
    let rendered = path.display().to_string();
    if rendered.is_empty() {
        return Err(MaterialTextureRecipeError::UnsafePath {
            path: rendered,
            reason: "path is empty",
        });
    }
    if rendered.contains('\\') {
        return Err(MaterialTextureRecipeError::UnsafePath {
            path: rendered,
            reason: "backslashes are not supported",
        });
    }
    if is_drive_prefixed(&rendered) {
        return Err(MaterialTextureRecipeError::UnsafePath {
            path: rendered,
            reason: "drive-letter paths are not supported",
        });
    }
    if path.is_absolute() {
        return Err(MaterialTextureRecipeError::UnsafePath {
            path: rendered,
            reason: "absolute paths are not supported",
        });
    }
    if reject_parent
        && path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(MaterialTextureRecipeError::UnsafePath {
            path: rendered,
            reason: "parent traversal is not allowed with texture_root",
        });
    }
    Ok(())
}

fn is_drive_prefixed(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

struct PreparedTexture {
    material_index: usize,
    material_name: String,
    slot: TextureRole,
    declared_path: String,
    image: ProcessedImage,
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::model::MaterialAsset;

    fn document(names: &[&str]) -> Document {
        let mut document = Document::default();
        document.assets.materials = names
            .iter()
            .map(|name| MaterialAsset {
                name: (*name).into(),
                base_color: [0.25, 0.5, 0.75, 1.0],
                metallic: 0.0,
                roughness: 1.0,
                base_color_texture: None,
                normal_texture: None,
            })
            .collect();
        document
    }

    fn write_png(path: &Path) {
        image::RgbaImage::from_pixel(2, 2, image::Rgba([128, 128, 255, 255]))
            .save(path)
            .expect("writes PNG fixture");
    }

    fn recipe(materials: &str, root: Option<&str>) -> String {
        format!(
            "schema_version = 1\nschema = {RECIPE_SCHEMA_ID:?}\n{}max_dimension = 1\n{materials}",
            root.map_or(String::new(), |root| format!("texture_root = {root:?}\n"))
        )
    }

    #[test]
    fn exact_recipe_embeds_slots_in_source_material_order() {
        let directory = tempfile::tempdir().unwrap();
        write_png(&directory.path().join("a.png"));
        write_png(&directory.path().join("b.png"));
        let path = directory.path().join("recipe.toml");
        std::fs::write(
            &path,
            recipe(
                "[[materials]]\nname = \"second\"\nbase_color = \"b.png\"\nnormal = \"b.png\"\n\n[[materials]]\nname = \"first\"\nbase_color = \"a.png\"\nnormal = \"a.png\"\n",
                None,
            ),
        )
        .unwrap();

        let applied = apply_material_texture_recipe(&path, &document(&["first", "second"]))
            .expect("exact recipe applies");
        assert_eq!(
            applied
                .evidence
                .consumed_inputs
                .iter()
                .map(|input| (input.material_name.as_str(), input.slot))
                .collect::<Vec<_>>(),
            vec![
                ("first", TextureRole::BaseColor),
                ("first", TextureRole::Normal),
                ("second", TextureRole::BaseColor),
                ("second", TextureRole::Normal),
            ]
        );
        assert_eq!(applied.document.assets.materials[0].base_color, [1.0; 4]);
        assert_eq!(
            applied.document.assets.materials[0]
                .normal_texture
                .as_ref()
                .unwrap()
                .scale,
            1.0
        );
        assert_eq!(applied.evidence.emitted_textures[0].dimensions, [1, 1]);
    }

    #[test]
    fn mapping_failures_are_exact_and_do_not_mutate_the_input() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recipe.toml");
        let original = document(&["one", "two"]);
        for (label, materials, expected) in [
            (
                "duplicate",
                "[[materials]]\nname = \"one\"\nbase_color = \"x\"\nnormal = \"x\"\n[[materials]]\nname = \"one\"\nbase_color = \"x\"\nnormal = \"x\"\n",
                "duplicate material",
            ),
            (
                "unused",
                "[[materials]]\nname = \"one\"\nbase_color = \"x\"\nnormal = \"x\"\n[[materials]]\nname = \"other\"\nbase_color = \"x\"\nnormal = \"x\"\n",
                "matches no source",
            ),
            (
                "missing",
                "[[materials]]\nname = \"one\"\nbase_color = \"x\"\nnormal = \"x\"\n",
                "has no recipe",
            ),
        ] {
            std::fs::write(&path, recipe(materials, None)).unwrap();
            let error = apply_material_texture_recipe(&path, &original).unwrap_err();
            assert!(error.to_string().contains(expected), "{label}: {error}");
            assert_eq!(
                original.assets.materials[0].base_color,
                [0.25, 0.5, 0.75, 1.0]
            );
            assert!(original.assets.materials[0].base_color_texture.is_none());
        }
        let duplicate_source = document(&["one", "one"]);
        std::fs::write(
            &path,
            recipe(
                "[[materials]]\nname = \"one\"\nbase_color = \"x\"\nnormal = \"x\"\n",
                None,
            ),
        )
        .unwrap();
        assert!(matches!(
            apply_material_texture_recipe(&path, &duplicate_source),
            Err(MaterialTextureRecipeError::AmbiguousSourceMaterial { .. })
        ));
    }

    #[test]
    fn recipe_rejects_unknown_fields() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recipe.toml");
        std::fs::write(
            &path,
            concat!(
                "schema_version = 1\n",
                "schema = \"urn:animsmith:schema:material-texture-recipe:1\"\n",
                "max_dimension = 1\n",
                "unexpected = true\n",
                "[[materials]]\n",
                "name = \"one\"\n",
                "base_color = \"base.png\"\n",
                "normal = \"normal.png\"\n",
            ),
        )
        .unwrap();

        let error = apply_material_texture_recipe(&path, &document(&["one"])).unwrap_err();
        assert!(
            error.to_string().contains("unknown field `unexpected`"),
            "{error}"
        );
    }

    #[test]
    fn material_names_are_case_sensitive() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("recipe.toml");
        std::fs::write(
            &path,
            recipe(
                "[[materials]]\nname = \"Painted\"\nbase_color = \"base.png\"\nnormal = \"normal.png\"\n",
                None,
            ),
        )
        .unwrap();

        assert!(matches!(
            apply_material_texture_recipe(&path, &document(&["painted"])),
            Err(MaterialTextureRecipeError::UnusedRecipeMaterial { .. })
        ));
    }

    #[test]
    fn root_rejects_traversal_and_missing_or_unsupported_files() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("textures")).unwrap();
        let path = directory.path().join("recipe.toml");
        for (label, value, expected) in [
            ("traversal", "../outside.png", "parent traversal"),
            ("missing", "missing.png", "cannot use texture file"),
        ] {
            std::fs::write(
                &path,
                recipe(
                    &format!("[[materials]]\nname = \"one\"\nbase_color = {value:?}\nnormal = {value:?}\n"),
                    Some("textures"),
                ),
            )
            .unwrap();
            let error = apply_material_texture_recipe(&path, &document(&["one"])).unwrap_err();
            assert!(error.to_string().contains(expected), "{label}: {error}");
        }
        std::fs::write(directory.path().join("textures/bad.bmp"), b"not an image").unwrap();
        std::fs::write(
            &path,
            recipe(
                "[[materials]]\nname = \"one\"\nbase_color = \"bad.bmp\"\nnormal = \"bad.bmp\"\n",
                Some("textures"),
            ),
        )
        .unwrap();
        assert!(matches!(
            apply_material_texture_recipe(&path, &document(&["one"])),
            Err(MaterialTextureRecipeError::Image { .. })
        ));
        std::fs::write(
            directory.path().join("textures/malformed.png"),
            b"\x89PNG\r\n\x1a\ntruncated",
        )
        .unwrap();
        std::fs::write(
            &path,
            recipe(
                "[[materials]]\nname = \"one\"\nbase_color = \"malformed.png\"\nnormal = \"malformed.png\"\n",
                Some("textures"),
            ),
        )
        .unwrap();
        assert!(matches!(
            apply_material_texture_recipe(&path, &document(&["one"])),
            Err(MaterialTextureRecipeError::Image { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn root_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("textures");
        std::fs::create_dir(&root).unwrap();
        let outside = directory.path().join("outside.png");
        write_png(&outside);
        symlink(&outside, root.join("escape.png")).unwrap();
        let path = directory.path().join("recipe.toml");
        std::fs::write(
            &path,
            recipe(
                "[[materials]]\nname = \"one\"\nbase_color = \"escape.png\"\nnormal = \"escape.png\"\n",
                Some("textures"),
            ),
        )
        .unwrap();
        assert!(matches!(
            apply_material_texture_recipe(&path, &document(&["one"])),
            Err(MaterialTextureRecipeError::UnsafePath { .. })
        ));
    }

    #[test]
    fn path_policy_rejects_cross_platform_absolute_spelling() {
        assert!(matches!(
            validate_declared_path(Path::new("C:/assets/texture.png"), false),
            Err(MaterialTextureRecipeError::UnsafePath { .. })
        ));
        assert!(matches!(
            validate_declared_path(Path::new("C:assets/texture.png"), false),
            Err(MaterialTextureRecipeError::UnsafePath { .. })
        ));
        assert!(validate_declared_path(Path::new("../assets/texture.png"), false).is_ok());
    }

    #[test]
    fn unrooted_parent_paths_resolve_from_the_recipe_directory() {
        let directory = tempfile::tempdir().unwrap();
        let recipes = directory.path().join("recipes");
        std::fs::create_dir(&recipes).unwrap();
        write_png(&directory.path().join("base.png"));
        write_png(&directory.path().join("normal.png"));
        let path = recipes.join("recipe.toml");
        std::fs::write(
            &path,
            recipe(
                "[[materials]]\nname = \"one\"\nbase_color = \"../base.png\"\nnormal = \"../normal.png\"\n",
                None,
            ),
        )
        .unwrap();

        let applied = apply_material_texture_recipe(&path, &document(&["one"]))
            .expect("recipe-relative parent paths apply without a root");
        assert!(
            applied.document.assets.materials[0]
                .base_color_texture
                .is_some()
        );
        assert!(
            applied.document.assets.materials[0]
                .normal_texture
                .is_some()
        );
    }
}
