//! Format-neutral source facts projected from one successfully parsed ufbx scene.

use animsmith_core::{
    InputIdentity, RAW_SOURCE_V1_MAX_TEXT_BYTES, RawSourceFactsBuilderV1, SourceAxisV1,
    SourceChannelFactV1, SourceChannelPropertyV1, SourceClipFactV1, SourceComponentMaskV1,
    SourceConstructFactV1, SourceConstructKindV1, SourceCoordinateBasisV1, SourceFactDomainV1,
    SourceFactSetV1, SourceFormatV1, SourceFramesPerSecondV1, SourceLinearUnitV1,
    SourceLoaderDispositionV1, SourceLogicalLocatorV1, SourceObservationV1, SourceProvenanceV1,
    SourceResourceKindV1, SourceResourceLocatorV1, SourceResourceReferenceV1, SourceTargetKindV1,
    SourceTargetV1, SourceTextV1, SourceTimeRangeV1, SourceUnavailableReasonV1,
};

/// Project raw FBX facts before the loader performs rooted dependency capture.
///
/// The caller retains this builder long enough to attach a closure constructed
/// from its exact resource row prefix.
pub(crate) fn project(
    scene: &ufbx::Scene,
    construct_counts: SourceConstructCounts,
    primary_bytes: &[u8],
) -> RawSourceFactsBuilderV1 {
    let mut builder = RawSourceFactsBuilderV1::new(
        SourceFormatV1::Fbx,
        InputIdentity::from_bytes(primary_bytes),
    );
    project_coordinate_facts(scene, &mut builder);
    project_clips(scene, &mut builder);
    project_constructs(construct_counts, &mut builder);
    project_resources(scene, &mut builder);
    builder
}

fn project_coordinate_facts(scene: &ufbx::Scene, builder: &mut RawSourceFactsBuilderV1) {
    let unit_provenance = parser_provenance("fbx:scene.settings.unit_meters");
    builder.set_linear_unit(match SourceLinearUnitV1::new(scene.settings.unit_meters) {
        Ok(unit) => SourceObservationV1::observed(
            unit,
            unit_provenance,
            SourceLoaderDispositionV1::Normalized,
        ),
        Err(_) => SourceObservationV1::unavailable(
            SourceUnavailableReasonV1::ParserUnavailable,
            Some(unit_provenance),
            SourceLoaderDispositionV1::Normalized,
        ),
    });

    let basis_provenance = parser_provenance("fbx:scene.settings.axes");
    let axes = scene.settings.axes;
    let basis = axis(axes.right)
        .zip(axis(axes.up))
        .zip(axis(axes.front))
        .and_then(|((right, up), forward)| SourceCoordinateBasisV1::new(right, up, forward).ok());
    builder.set_coordinate_basis(match basis {
        Some(basis) => SourceObservationV1::observed(
            basis,
            basis_provenance,
            SourceLoaderDispositionV1::Normalized,
        ),
        None => SourceObservationV1::unavailable(
            SourceUnavailableReasonV1::ParserUnavailable,
            Some(basis_provenance),
            SourceLoaderDispositionV1::Normalized,
        ),
    });

    let fps_provenance = parser_provenance("fbx:scene.settings.frames_per_second");
    builder.set_frames_per_second(
        match SourceFramesPerSecondV1::new(scene.settings.frames_per_second) {
            Ok(fps) => SourceObservationV1::observed(
                fps,
                fps_provenance,
                SourceLoaderDispositionV1::Discarded,
            ),
            Err(_) => SourceObservationV1::unavailable(
                SourceUnavailableReasonV1::ParserUnavailable,
                Some(fps_provenance),
                SourceLoaderDispositionV1::Discarded,
            ),
        },
    );
}

fn axis(axis: ufbx::CoordinateAxis) -> Option<SourceAxisV1> {
    match axis {
        ufbx::CoordinateAxis::PositiveX => Some(SourceAxisV1::PositiveX),
        ufbx::CoordinateAxis::NegativeX => Some(SourceAxisV1::NegativeX),
        ufbx::CoordinateAxis::PositiveY => Some(SourceAxisV1::PositiveY),
        ufbx::CoordinateAxis::NegativeY => Some(SourceAxisV1::NegativeY),
        ufbx::CoordinateAxis::PositiveZ => Some(SourceAxisV1::PositiveZ),
        ufbx::CoordinateAxis::NegativeZ => Some(SourceAxisV1::NegativeZ),
        ufbx::CoordinateAxis::Unknown => None,
    }
}

fn project_clips(scene: &ufbx::Scene, builder: &mut RawSourceFactsBuilderV1) {
    for (stack_index, stack) in scene.anim_stacks.iter().enumerate() {
        if builder.remaining_clip_rows() == 0 || builder.remaining_observation_rows() == 0 {
            builder.mark_budget_exceeded(SourceFactDomainV1::Clips);
            return;
        }

        let stack_locator_bytes = "fbx:anim_stacks/"
            .len()
            .saturating_add(decimal_len_usize(stack_index));
        let name_locator_bytes = stack_locator_bytes.saturating_add("/name".len());
        let range_locator_bytes = stack_locator_bytes.saturating_add("/time_range".len());
        let retained_name_bytes = if stack.element.name.is_empty()
            || stack.element.name.len() > RAW_SOURCE_V1_MAX_TEXT_BYTES
        {
            0
        } else {
            stack.element.name.len()
        };
        let fixed_text_bytes = name_locator_bytes
            .saturating_add(retained_name_bytes)
            .saturating_add(stack_locator_bytes)
            .saturating_add(range_locator_bytes);
        if fixed_text_bytes > builder.remaining_text_bytes() {
            builder.mark_budget_exceeded(SourceFactDomainV1::Clips);
            return;
        }
        let stack_locator = format!("fbx:anim_stacks/{stack_index}");
        let name_locator = format!("{stack_locator}/name");
        let range_locator = format!("{stack_locator}/time_range");
        let channel_budget = builder.remaining_observation_rows().saturating_sub(1);
        let channel_text_budget = builder
            .remaining_text_bytes()
            .saturating_sub(fixed_text_bytes);
        let (channels, channels_truncated) =
            project_channels(stack, channel_budget, channel_text_budget);
        let channels = if channels_truncated {
            SourceFactSetV1::partial(
                channels,
                SourceUnavailableReasonV1::ProjectionBudgetExceeded,
            )
        } else {
            SourceFactSetV1::complete(channels)
        };

        let source_name = if stack.element.name.is_empty() {
            SourceObservationV1::unavailable(
                SourceUnavailableReasonV1::ParserUnavailable,
                Some(parser_provenance(&name_locator)),
                SourceLoaderDispositionV1::Preserved,
            )
        } else if stack.element.name.len() > RAW_SOURCE_V1_MAX_TEXT_BYTES {
            SourceObservationV1::unavailable(
                SourceUnavailableReasonV1::ProjectionBudgetExceeded,
                Some(parser_provenance(&name_locator)),
                SourceLoaderDispositionV1::Preserved,
            )
        } else {
            SourceObservationV1::observed(
                SourceTextV1::new(stack.element.name.as_ref())
                    .expect("source name length checked before cloning"),
                parser_provenance(&name_locator),
                SourceLoaderDispositionV1::Preserved,
            )
        };

        let normalized_clip_index = SourceObservationV1::observed(
            stack_index,
            derived_provenance(&stack_locator),
            SourceLoaderDispositionV1::Baked,
        );
        let source_range = match SourceTimeRangeV1::new(stack.time_begin, stack.time_end) {
            Ok(range) => SourceObservationV1::observed(
                range,
                parser_provenance(&range_locator),
                SourceLoaderDispositionV1::Baked,
            ),
            Err(_) => SourceObservationV1::unavailable(
                SourceUnavailableReasonV1::Malformed,
                Some(parser_provenance(&range_locator)),
                SourceLoaderDispositionV1::Baked,
            ),
        };
        let sampler_range = SourceObservationV1::unavailable(
            SourceUnavailableReasonV1::ParserUnavailable,
            None,
            SourceLoaderDispositionV1::NotApplicable,
        );

        if !builder.push_clip(SourceClipFactV1::new(
            stack_index,
            source_name,
            normalized_clip_index,
            source_range,
            sampler_range,
            channels,
        )) {
            return;
        }
        if channels_truncated {
            return;
        }
    }
    builder.mark_complete(SourceFactDomainV1::Clips);
}

fn project_channels(
    stack: &ufbx::AnimStack,
    channel_budget: usize,
    total_text_budget: usize,
) -> (Vec<SourceChannelFactV1>, bool) {
    let mut channels = Vec::new();
    let mut retained_text_bytes = 0usize;

    for layer in &stack.layers {
        let layer_index = layer.element.typed_id as usize;
        for (property_index, property) in layer.anim_props.as_ref().iter().enumerate() {
            if channels.len() >= channel_budget {
                return (channels, true);
            }
            let channel_index = channels.len();
            let (kind, disposition) = property_kind(property.prop_name.as_ref());
            let property_name_bytes = if kind == SourceChannelPropertyV1::Other {
                property.prop_name.len()
            } else {
                0
            };
            if property_name_bytes > RAW_SOURCE_V1_MAX_TEXT_BYTES {
                return (channels, true);
            }
            // The row and unavailable-interpolation observations each retain
            // their parser locator. Standard TRS properties need no duplicate
            // source spelling because the enum is the complete identity.
            let locator_bytes = "fbx:anim_layers/"
                .len()
                .saturating_add(decimal_len_usize(layer_index))
                .saturating_add("/anim_props/".len())
                .saturating_add(decimal_len_usize(property_index));
            let row_text_bytes = locator_bytes
                .saturating_mul(2)
                .saturating_add(property_name_bytes);
            if retained_text_bytes
                .checked_add(row_text_bytes)
                .is_none_or(|bytes| bytes > total_text_budget)
            {
                return (channels, true);
            }

            let locator = format!("fbx:anim_layers/{layer_index}/anim_props/{property_index}");
            let provenance = parser_provenance(&locator);
            let interpolation = SourceObservationV1::unavailable(
                SourceUnavailableReasonV1::BakedAway,
                Some(provenance.clone()),
                disposition,
            );
            let target = if property.element.type_ == ufbx::ElementType::Node {
                SourceTargetV1::new(
                    SourceTargetKindV1::Node,
                    u64::from(property.element.typed_id),
                )
            } else {
                SourceTargetV1::new(
                    SourceTargetKindV1::Element,
                    u64::from(property.element.element_id),
                )
            };
            let curves = &property.anim_value.curves;
            let mut row = SourceChannelFactV1::new(
                channel_index,
                target,
                kind,
                SourceComponentMaskV1::new(
                    curves[0].is_some(),
                    curves[1].is_some(),
                    curves[2].is_some(),
                ),
                interpolation,
                disposition,
                provenance,
            )
            .with_source_layer_index(layer_index);
            if kind == SourceChannelPropertyV1::Other && !property.prop_name.is_empty() {
                row = row.with_property_name(
                    SourceTextV1::new(property.prop_name.as_ref())
                        .expect("source property length checked before cloning"),
                );
            }
            channels.push(row);
            retained_text_bytes = retained_text_bytes.saturating_add(row_text_bytes);
        }
    }
    (channels, false)
}

fn property_kind(name: &str) -> (SourceChannelPropertyV1, SourceLoaderDispositionV1) {
    match name {
        "Lcl Translation" => (
            SourceChannelPropertyV1::Translation,
            SourceLoaderDispositionV1::Baked,
        ),
        "Lcl Rotation" => (
            SourceChannelPropertyV1::Rotation,
            SourceLoaderDispositionV1::Baked,
        ),
        "Lcl Scaling" => (
            SourceChannelPropertyV1::Scale,
            SourceLoaderDispositionV1::Baked,
        ),
        _ => (
            SourceChannelPropertyV1::Other,
            SourceLoaderDispositionV1::Unsupported,
        ),
    }
}

fn project_constructs(counts: SourceConstructCounts, builder: &mut RawSourceFactsBuilderV1) {
    let mut source_order_index = 0usize;
    for (name, kind, count, disposition, locator) in [
        (
            "fbx:user-defined-properties",
            SourceConstructKindV1::CustomProperty,
            counts.rest_bind.user_defined_property_count,
            SourceLoaderDispositionV1::Unsupported,
            "fbx:scene.elements.props",
        ),
        (
            "fbx:unmodeled-elements",
            SourceConstructKindV1::UnknownElement,
            counts.rest_bind.total_unmodeled_element_count(),
            SourceLoaderDispositionV1::Unsupported,
            "fbx:scene.elements",
        ),
        (
            "fbx:stackless-animation",
            SourceConstructKindV1::UnknownElement,
            counts.stackless_animation_count,
            SourceLoaderDispositionV1::Unsupported,
            "fbx:scene.animation",
        ),
    ] {
        if count == 0 {
            continue;
        }
        let required_text = name.len().saturating_add(locator.len());
        if builder.remaining_observation_rows() == 0
            || required_text > builder.remaining_text_bytes()
        {
            builder.mark_budget_exceeded(SourceFactDomainV1::Constructs);
            return;
        }
        let row = SourceConstructFactV1::new(
            source_order_index,
            kind,
            SourceTextV1::new(name).expect("static construct name is bounded"),
            false,
            u64::try_from(count).unwrap_or(u64::MAX),
            disposition,
            parser_provenance(locator),
        )
        .expect("zero aggregate construct counts are skipped");
        if !builder.push_construct(row) {
            return;
        }
        source_order_index = source_order_index.saturating_add(1);
    }
    builder.mark_complete(SourceFactDomainV1::Constructs);
}

/// Raw parser counts shared by the source-facts projection and derived scale
/// inventory, computed exactly once per load.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceConstructCounts {
    pub(crate) rest_bind: RestBindSourceConstructCounts,
    stackless_animation_count: usize,
}

/// Same-parse breakdown of the aggregate unmodeled-construct row used by the
/// narrow rest/bind admission policy.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RestBindSourceConstructCounts {
    pub(crate) user_defined_property_count: usize,
    pub(crate) safe_texture_file_link_count: usize,
    pub(crate) unsupported_unmodeled_element_count: usize,
}

impl RestBindSourceConstructCounts {
    pub(crate) fn total_unmodeled_element_count(self) -> usize {
        self.safe_texture_file_link_count
            .saturating_add(self.unsupported_unmodeled_element_count)
    }
}

pub(crate) fn construct_counts(scene: &ufbx::Scene) -> SourceConstructCounts {
    let user_defined_property_count = scene
        .elements
        .iter()
        .flat_map(|element| element.props.props.iter())
        .filter(|prop| prop.flags.has_any(ufbx::PropFlags::USER_DEFINED))
        .count();
    let (unsupported_unmodeled_element_count, safe_texture_file_link_count) =
        rest_bind_unmodeled_element_counts(scene);
    let stackless_animation_count = if scene.anim_stacks.is_empty() {
        scene
            .anim_layers
            .len()
            .saturating_add(scene.anim_values.len())
            .saturating_add(scene.anim_curves.len())
    } else {
        0
    };
    SourceConstructCounts {
        rest_bind: RestBindSourceConstructCounts {
            user_defined_property_count,
            safe_texture_file_link_count,
            unsupported_unmodeled_element_count,
        },
        stackless_animation_count,
    }
}

/// Classify every field in `ufbx::Scene` at one exhaustive structural
/// boundary. Omitting `..` is deliberate: a ufbx upgrade that adds a typed
/// list must fail to compile until that list receives a classification.
fn rest_bind_unmodeled_element_counts(scene: &ufbx::Scene) -> (usize, usize) {
    let ufbx::Scene {
        metadata: _,
        settings: _,
        root_node: _,
        anim: _,
        // Unknown and source kinds without a normalized core representation.
        unknowns,
        nodes: _,
        meshes: _,
        // Cameras and lights have dedicated counts/core facts.
        lights: _,
        cameras: _,
        // Bone/empty attributes normalize into the complete node projection.
        bones: _,
        empties: _,
        line_curves,
        nurbs_curves,
        nurbs_surfaces,
        nurbs_trim_surfaces,
        nurbs_trim_boundaries,
        procedural_geometries,
        stereo_cameras,
        camera_switchers,
        markers,
        lod_groups,
        // Skin rows have dedicated bind/influence counts and sidecars.
        skin_deformers: _,
        skin_clusters: _,
        // Deformers are counted separately; their subordinate payload rows
        // are still unmodeled source elements.
        blend_deformers: _,
        blend_channels,
        blend_shapes,
        cache_deformers: _,
        cache_files,
        // The loader rebuilds its documented material/texture subset;
        // external payload absence is counted separately.
        materials: _,
        textures: _,
        videos: _,
        shaders,
        shader_bindings,
        // Animation lists are evaluated through bake_anim.
        anim_stacks: _,
        anim_layers: _,
        anim_values: _,
        anim_curves: _,
        display_layers,
        selection_sets,
        selection_nodes,
        characters,
        constraints,
        audio_layers,
        audio_clips,
        poses,
        metadata_objects,
        // Texture-file records carry source linkage beyond the rebuilt
        // texture subset, so they are conservatively unmodeled.
        texture_files,
        // Scene-wide structural indexes are parser-derived views, not source
        // element domains that need independent semantic counting.
        elements: _,
        connections_src: _,
        connections_dst: _,
        elements_by_name: _,
        dom_root: _,
    } = scene;

    let unsupported = unknowns.len()
        + line_curves.len()
        + nurbs_curves.len()
        + nurbs_surfaces.len()
        + nurbs_trim_surfaces.len()
        + nurbs_trim_boundaries.len()
        + procedural_geometries.len()
        + stereo_cameras.len()
        + camera_switchers.len()
        + markers.len()
        + lod_groups.len()
        + blend_channels.len()
        + blend_shapes.len()
        + cache_files.len()
        + shaders.len()
        + shader_bindings.len()
        + display_layers.len()
        + selection_sets.len()
        + selection_nodes.len()
        + characters.len()
        + constraints.len()
        + audio_layers.len()
        + audio_clips.len()
        + poses.len()
        + metadata_objects.len();
    (unsupported, texture_files.len())
}

fn project_resources(scene: &ufbx::Scene, builder: &mut RawSourceFactsBuilderV1) {
    for resource in resource_declarations(scene) {
        if builder.remaining_resource_rows() == 0 || builder.remaining_observation_rows() == 0 {
            builder.mark_budget_exceeded(SourceFactDomainV1::Resources);
            return;
        }
        let (value, field, redacted_locator) = resource.source_locator();
        // Reserve exactly what classification retains before the only possible
        // source-string clone. Unsafe, absolute, remote, data, malformed, and
        // oversized spellings are redacted and therefore reserve zero bytes.
        let retained_locator_bytes =
            value.map_or(0, SourceResourceLocatorV1::retained_relative_bytes);
        let provenance_locator_bytes = "fbx:"
            .len()
            .saturating_add(resource.type_name.len())
            .saturating_add(1)
            .saturating_add(decimal_len_u64(resource.source_index()))
            .saturating_add(1)
            .saturating_add(field.len());
        if provenance_locator_bytes.saturating_add(retained_locator_bytes)
            > builder.remaining_text_bytes()
        {
            builder.mark_budget_exceeded(SourceFactDomainV1::Resources);
            return;
        }
        let provenance_locator = format!(
            "fbx:{}/{}/{field}",
            resource.type_name,
            resource.source_index()
        );
        let locator = redacted_locator.unwrap_or_else(|| {
            value.map_or(SourceResourceLocatorV1::Missing, |value| {
                SourceResourceLocatorV1::classify(value)
            })
        });
        let row = SourceResourceReferenceV1::new(
            resource.source_order_index(),
            resource.kind(),
            resource.source_index(),
            locator,
            resource.disposition,
            parser_provenance(&provenance_locator),
        );
        if !builder.push_resource(row) {
            return;
        }
    }
    builder.mark_complete(SourceFactDomainV1::Resources);
}

#[derive(Clone, Copy)]
enum FbxResourceList {
    Texture,
    Video,
    Cache,
}

impl FbxResourceList {
    const fn tie_breaker(self) -> u8 {
        match self {
            Self::Texture => 0,
            Self::Video => 1,
            Self::Cache => 2,
        }
    }
}

fn next_resource_list(
    scene: &ufbx::Scene,
    texture_index: usize,
    video_index: usize,
    cache_index: usize,
) -> Option<FbxResourceList> {
    [
        scene
            .textures
            .get(texture_index)
            .map(|texture| (texture.element.element_id, FbxResourceList::Texture)),
        scene
            .videos
            .get(video_index)
            .map(|video| (video.element.element_id, FbxResourceList::Video)),
        scene
            .cache_files
            .get(cache_index)
            .map(|cache| (cache.element.element_id, FbxResourceList::Cache)),
    ]
    .into_iter()
    .flatten()
    .min_by_key(|(element_id, list)| (*element_id, list.tie_breaker()))
    .map(|(_, list)| list)
}

/// One deterministic typed FBX resource declaration.
///
/// The iterator below is the sole authority for raw resource rows and the
/// dependency-closure capture pass. It retains only a boolean presence marker
/// for the parser-resolved absolute path so an external declaration is never
/// misreported as absent. That path is never retained, exposed, normalized,
/// or opened; only embedded content and source-relative declarations are
/// eligible for capture.
pub(crate) struct ResourceDeclaration<'a> {
    source_order_index: usize,
    kind: SourceResourceKindV1,
    source_index: u64,
    embedded: bool,
    relative_filename: &'a str,
    filename: &'a str,
    absolute_filename_present: bool,
    disposition: SourceLoaderDispositionV1,
    type_name: &'static str,
}

impl ResourceDeclaration<'_> {
    /// Stable source-order index shared by raw facts and closure rows.
    pub(crate) const fn source_order_index(&self) -> usize {
        self.source_order_index
    }

    /// Typed source declaration kind.
    pub(crate) const fn kind(&self) -> SourceResourceKindV1 {
        self.kind
    }

    /// Parser-stable source declaration index.
    pub(crate) const fn source_index(&self) -> u64 {
        self.source_index
    }

    /// Whether content is carried by the exact primary FBX bytes.
    pub(crate) const fn is_embedded(&self) -> bool {
        self.embedded
    }

    fn source_locator(&self) -> (Option<&str>, &'static str, Option<SourceResourceLocatorV1>) {
        if self.is_embedded() {
            (None, "content", Some(SourceResourceLocatorV1::Embedded))
        } else if !self.relative_filename.is_empty() {
            (Some(self.relative_filename), "relative_filename", None)
        } else if !self.filename.is_empty() {
            // ufbx may surface a resolved absolute spelling here. It is only
            // classified/redacted by core and can never become a capture key.
            (Some(self.filename), "filename", None)
        } else if self.absolute_filename_present {
            // Preserve positive declaration evidence without retaining or
            // resolving the parser's host-specific absolute spelling.
            (
                None,
                "absolute_filename",
                Some(SourceResourceLocatorV1::Absolute),
            )
        } else {
            (None, "filename", Some(SourceResourceLocatorV1::Missing))
        }
    }
}

/// Bounded, allocation-free traversal of the typed FBX resource lists.
pub(crate) struct ResourceDeclarations<'a> {
    scene: &'a ufbx::Scene,
    source_order_index: usize,
    texture_index: usize,
    video_index: usize,
    cache_index: usize,
}

/// Iterate texture, video, and cache declarations in stable scene-wide order.
pub(crate) fn resource_declarations(scene: &ufbx::Scene) -> ResourceDeclarations<'_> {
    ResourceDeclarations {
        scene,
        source_order_index: 0,
        texture_index: 0,
        video_index: 0,
        cache_index: 0,
    }
}

impl<'a> Iterator for ResourceDeclarations<'a> {
    type Item = ResourceDeclaration<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let list = next_resource_list(
            self.scene,
            self.texture_index,
            self.video_index,
            self.cache_index,
        )?;
        let source_order_index = self.source_order_index;
        self.source_order_index = self.source_order_index.saturating_add(1);
        Some(match list {
            FbxResourceList::Texture => {
                let texture = &self.scene.textures[self.texture_index];
                self.texture_index = self.texture_index.saturating_add(1);
                ResourceDeclaration {
                    source_order_index,
                    kind: SourceResourceKindV1::Texture,
                    source_index: u64::from(texture.element.typed_id),
                    embedded: !texture.content.is_empty(),
                    relative_filename: texture.relative_filename.as_ref(),
                    filename: texture.filename.as_ref(),
                    absolute_filename_present: !texture.absolute_filename.is_empty(),
                    disposition: SourceLoaderDispositionV1::Unknown,
                    type_name: "textures",
                }
            }
            FbxResourceList::Video => {
                let video = &self.scene.videos[self.video_index];
                self.video_index = self.video_index.saturating_add(1);
                ResourceDeclaration {
                    source_order_index,
                    kind: SourceResourceKindV1::Video,
                    source_index: u64::from(video.element.typed_id),
                    embedded: !video.content.is_empty(),
                    relative_filename: video.relative_filename.as_ref(),
                    filename: video.filename.as_ref(),
                    absolute_filename_present: !video.absolute_filename.is_empty(),
                    disposition: SourceLoaderDispositionV1::Discarded,
                    type_name: "videos",
                }
            }
            FbxResourceList::Cache => {
                let cache = &self.scene.cache_files[self.cache_index];
                self.cache_index = self.cache_index.saturating_add(1);
                ResourceDeclaration {
                    source_order_index,
                    kind: SourceResourceKindV1::Cache,
                    source_index: u64::from(cache.element.typed_id),
                    embedded: false,
                    relative_filename: cache.relative_filename.as_ref(),
                    filename: cache.filename.as_ref(),
                    absolute_filename_present: !cache.absolute_filename.is_empty(),
                    disposition: SourceLoaderDispositionV1::Unsupported,
                    type_name: "cache_files",
                }
            }
        })
    }
}

fn parser_provenance(locator: &str) -> SourceProvenanceV1 {
    SourceProvenanceV1::parser_projected(
        SourceLogicalLocatorV1::fbx_parser_path(locator)
            .expect("generated FBX logical locator is bounded and structural"),
    )
}

fn derived_provenance(locator: &str) -> SourceProvenanceV1 {
    SourceProvenanceV1::derived_from_source(
        SourceLogicalLocatorV1::fbx_parser_path(locator)
            .expect("generated FBX logical locator is bounded and structural"),
    )
}

fn decimal_len_usize(mut value: usize) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn decimal_len_u64(mut value: u64) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}
