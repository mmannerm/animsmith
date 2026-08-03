# Material Texture Recipes

`animsmith convert --material-texture-recipe <PATH>` attaches explicit
BaseColor, normal, metallic-roughness, and occlusion images to named source materials. It is for assets whose
material links are incomplete or need a reproducible texture-size policy. The
recipe is declarative: it does not generate artistic content.

```console
animsmith convert input.fbx -o output.glb \
  --material-texture-recipe recipes/materials.toml
```

## Recipe format

Recipes are TOML documents with immutable identity
`urn:animsmith:schema:material-texture-recipe:1`. The retrievable JSON Schema
is [`material-texture-recipe-v1.schema.json`](schemas/material-texture-recipe-v1.schema.json).

```toml
schema_version = 1
schema = "urn:animsmith:schema:material-texture-recipe:1"
texture_root = "textures"
max_dimension = 1024

[[materials]]
name = "surface"
base_color = "surface-base.png"
normal = "surface-normal.jpg"
metallic_roughness = "surface-metallic-roughness.png"
occlusion = "surface-occlusion.png"
```

`materials` is a list. Each entry requires one exact source-material
`name`, one `base_color` path, and one `normal` path. `metallic_roughness` and
`occlusion` are optional paths for their corresponding glTF PBR slots. Names are matched exactly,
case-sensitively. A duplicate recipe name, a name that matches no source
material, or a name that matches multiple source materials is an operator error.
Every declared entry must be used; a recipe cannot partially apply and succeed.
Unknown recipe fields are rejected instead of ignored.

Run `animsmith inspect <input>` first and copy names from its top-level
`materials` section; its quoted names use TOML-compatible escapes and can be
copied directly into a recipe. The `mesh instances` section shows which material each
primitive references. Ambiguous source names are marked and must be corrected
in the authoring tool before a recipe can target them; `inspect` does not rename
or merge materials.

`max_dimension` is required and must be from 1 through 4096. It bounds both
the requested output size and accepted source dimensions. Source image files
are limited to 64 MiB. Only PNG and JPEG files are accepted; their type is
determined from file magic, not a filename extension.

## Paths and containment

All paths are interpreted relative to the recipe file. Empty, absolute,
drive-prefixed, and backslash spellings are rejected for consistent
cross-platform behavior.
`texture_root` is
optional. When it is set, it too is recipe-relative and is a containment root:
the converter canonicalizes the root and every image path and rejects traversal
or symlink resolution outside the root. Missing paths, non-regular files, and
escaped paths are operator errors. Without `texture_root`, ordinary
recipe-relative resolution applies, including explicit `..` components, and no
containment root is declared.

## Deterministic processing

The converter processes entries in source-material order and image slots in
BaseColor-then-normal-then-metallic-roughness-then-occlusion order. Recipe declaration order does not affect the
artifact or evidence. If an image already fits `max_dimension`, it is a no-op:
its original encoded bytes and MIME type are preserved. A resized image is
emitted as PNG RGBA8.

BaseColor resizing converts sRGB into linear light, premultiplies alpha, uses
Lanczos3 filtering, then converts back to sRGB. Normal resizing decodes
tangent-space vectors, uses Triangle filtering, and renormalizes vectors before
encoding. Metallic-roughness and occlusion maps use linear-channel Triangle
filtering: they are not treated as sRGB color or tangent-space vectors. For resized output, the pinned encoder uses `Best` compression and
`NoFilter`.
Producer evidence names the pinned primary packages `image 0.25.10`,
`png 0.18.1`, and `zune-jpeg 0.5.15`; `Cargo.lock` pins their full dependency
closure. Given the same supported-platform inputs, recipe, and tool version,
this policy produces deterministic output bytes.

## Evidence

With `--format json`, conversion evidence records the recipe path and root,
the declared dimension cap, the locked processor identity, every consumed image,
and every emitted texture. Records are ordered as described above. See
[machine-readable output](output.md#convert) for the versioned evidence
contract.
