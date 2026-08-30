//! Owner-run, headless exact-Bevy observation executable.

use bevy::{
    app::TaskPoolPlugin,
    asset::{
        AssetApp, AssetLoadError, AssetPlugin, AssetServer, Assets, LoadState,
        RecursiveDependencyLoadState,
    },
    gltf::{Gltf, GltfAssetLabel, GltfLoaderSettings, GltfNode, GltfPlugin, GltfSkin},
    image::Image,
    mesh::skinning::SkinnedMeshInverseBindposes,
    prelude::{AnimationClip, App, Handle},
    world_serialization::WorldSerializationPlugin,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Read, Write},
    path::{Component, Path, PathBuf},
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{Layer, layer::Context, prelude::*, registry::LookupSpan};

const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
const RUSTC_VERSION: &str = env!("ANIMSMITH_BEVY_READBACK_RUSTC");
const MAX_WARNINGS: usize = 64;

#[derive(Clone, Default)]
struct RedactedWarnings(Arc<Mutex<WarningCapture>>);

#[derive(Default)]
struct WarningCapture {
    values: Vec<(String, String)>,
    truncated: bool,
}

impl RedactedWarnings {
    fn snapshot(&self) -> Option<(Vec<animsmith_engine::BevyWarningV1>, bool)> {
        let capture = self.0.lock().ok()?;
        let mut values = capture.values.clone();
        values.sort();
        values.dedup();
        Some((
            values
                .into_iter()
                .map(|(target, level)| animsmith_engine::BevyWarningV1::new(target, level))
                .collect(),
            capture.truncated,
        ))
    }
}

impl<S> Layer<S> for RedactedWarnings
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let metadata = event.metadata();
        if !matches!(*metadata.level(), Level::WARN | Level::ERROR) {
            return;
        }
        let Ok(mut capture) = self.0.lock() else {
            return;
        };
        if capture.values.len() < MAX_WARNINGS {
            capture.values.push((
                metadata.target().to_owned(),
                metadata.level().as_str().to_ascii_lowercase(),
            ));
        } else {
            capture.truncated = true;
        }
    }
}

#[derive(Debug)]
enum PredictionInputError {
    Io,
    NotRegular,
    TooLarge,
}

fn read_prediction(path: impl AsRef<Path>) -> Result<Vec<u8>, PredictionInputError> {
    let path = path.as_ref();
    let initial = fs::symlink_metadata(path).map_err(|_| PredictionInputError::Io)?;
    if !initial.file_type().is_file() {
        return Err(PredictionInputError::NotRegular);
    }
    let limit = animsmith_engine::GLTF_ADDRESSABILITY_V2_MAX_REPORT_BYTES;
    if initial.len() > limit {
        return Err(PredictionInputError::TooLarge);
    }
    let file = open_prediction(path).map_err(|_| PredictionInputError::Io)?;
    let metadata = file.metadata().map_err(|_| PredictionInputError::Io)?;
    if !metadata.file_type().is_file() {
        return Err(PredictionInputError::NotRegular);
    }
    if metadata.len() > limit {
        return Err(PredictionInputError::TooLarge);
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| PredictionInputError::TooLarge)?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PredictionInputError::Io)?;
    if bytes.len() as u64 > limit {
        return Err(PredictionInputError::TooLarge);
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_prediction(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_prediction(path: &Path) -> io::Result<File> {
    File::open(path)
}

#[derive(Debug)]
enum SnapshotError {
    Source,
    Temporary,
}

struct VerifiedSnapshot {
    root: PathBuf,
    primary: (PathBuf, animsmith_core::InputIdentity),
    resources: Vec<(PathBuf, animsmith_core::InputIdentity)>,
}

impl VerifiedSnapshot {
    fn capture(
        source_root: &Path,
        asset: &str,
        primary: &animsmith_core::InputIdentity,
        closure: &animsmith_core::DependencyClosureV1,
    ) -> Result<Self, SnapshotError> {
        let root = private_snapshot_root().map_err(|_| SnapshotError::Temporary)?;
        let primary_relative = Path::new(asset);
        let primary_path = root.join(primary_relative);
        // Construct the owner before any file is staged so every early return
        // removes a partial private tree through `Drop`.
        let mut snapshot = Self {
            root,
            primary: (primary_path.clone(), primary.clone()),
            resources: Vec::with_capacity(closure.external_resources().len()),
        };
        let primary_source =
            rooted_file(source_root, primary_relative).map_err(|()| SnapshotError::Source)?;
        stage_verified_file(&primary_source, &primary_path, primary)?;
        for resource in closure.external_resources() {
            let relative = Path::new(resource.key().as_str());
            if !safe_relative_path(relative) {
                return Err(SnapshotError::Source);
            }
            let source = rooted_file(source_root, relative).map_err(|()| SnapshotError::Source)?;
            let destination = snapshot.root.join(relative);
            snapshot
                .resources
                .push((destination.clone(), resource.identity().clone()));
            stage_verified_file(&source, &destination, resource.identity())?;
        }
        Ok(snapshot)
    }

    fn root_str(&self) -> Option<&str> {
        self.root.to_str()
    }

    fn verify(&self) -> (bool, bool) {
        let primary = file_identity(&self.primary.0).is_ok_and(|value| value == self.primary.1);
        let dependencies = self
            .resources
            .iter()
            .all(|(path, expected)| file_identity(path).is_ok_and(|value| value == *expected));
        (primary, dependencies)
    }

    fn cleanup(&self) -> io::Result<()> {
        #[cfg(windows)]
        for (path, _) in std::iter::once(&self.primary).chain(self.resources.iter()) {
            let _ = make_writable(path);
        }
        match fs::remove_dir_all(&self.root) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }

    #[cfg(feature = "test-support")]
    fn corrupt_primary(&self) -> io::Result<()> {
        make_writable(&self.primary.0)?;
        fs::write(&self.primary.0, b"not a glTF artifact")
    }
}

impl Drop for VerifiedSnapshot {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn private_snapshot_root() -> io::Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let base = env::temp_dir();
    for attempt in 0..128_u32 {
        let path = base.join(format!(
            ".animsmith-bevy-readback-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Err(error) =
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                    {
                        let _ = fs::remove_dir(&path);
                        return Err(error);
                    }
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "cannot allocate private snapshot",
    ))
}

fn stage_verified_file(
    source: &Path,
    destination: &Path,
    expected: &animsmith_core::InputIdentity,
) -> Result<(), SnapshotError> {
    let parent = destination.parent().ok_or(SnapshotError::Temporary)?;
    fs::create_dir_all(parent).map_err(|_| SnapshotError::Temporary)?;
    let mut input = File::open(source).map_err(|_| SnapshotError::Source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| SnapshotError::Temporary)?;
    let mut hasher = Sha256::new();
    let mut count = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer).map_err(|_| SnapshotError::Source)?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .ok_or(SnapshotError::Source)?;
        if count > expected.bytes() {
            return Err(SnapshotError::Source);
        }
        hasher.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(|_| SnapshotError::Temporary)?;
    }
    output.sync_all().map_err(|_| SnapshotError::Temporary)?;
    let identity =
        animsmith_core::InputIdentity::from_sha256_digest(hasher.finalize().into(), count);
    if identity != *expected {
        return Err(SnapshotError::Source);
    }
    make_readonly(destination).map_err(|_| SnapshotError::Temporary)
}

fn file_identity(path: &Path) -> io::Result<animsmith_core::InputIdentity> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut count = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("snapshot byte count overflow"))?;
        hasher.update(&buffer[..read]);
    }
    Ok(animsmith_core::InputIdentity::from_sha256_digest(
        hasher.finalize().into(),
        count,
    ))
}

fn make_readonly(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o400))
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(path, permissions)
    }
}

#[cfg(any(windows, feature = "test-support"))]
fn make_writable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)
    }
}

fn safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(a), Some(root_arg), Some(b), Some(asset), Some(c), Some(prediction_path)) = (
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
        args.next(),
    ) else {
        return usage();
    };
    if a != "--asset-root"
        || b != "--asset"
        || c != "--prediction"
        || args.next().is_some()
        || !safe_key(&asset)
    {
        return usage();
    }
    let root = match fs::canonicalize(root_arg) {
        Ok(path) if path.is_dir() => path,
        _ => return operator_error("cannot access authorized asset root"),
    };
    let bytes = match read_prediction(prediction_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return operator_error(
                "prediction must be a regular strict V2 document within its byte limit",
            );
        }
    };
    let prediction_input = animsmith_core::InputIdentity::from_bytes(&bytes);
    let prediction = match animsmith_engine::GltfAddressabilityV2::read_from(Cursor::new(&bytes)) {
        Ok(value) => value,
        Err(_) => return operator_error("prediction is not strict rich addressability V2"),
    };
    let Some(adapter) = prediction.bevy() else {
        return conformance_error("prediction has no exact Bevy revision-3 adapter");
    };
    let primary = prediction.input().clone();
    if primary != *adapter.prediction_provenance().raw_source().primary_input() {
        return conformance_error("primary artifact does not match strict prediction identity");
    }
    let closure = adapter.prediction_provenance().dependency_closure();
    if !closure.coverage().is_complete() {
        return conformance_error("prediction dependency closure is not complete");
    }
    let snapshot = match VerifiedSnapshot::capture(&root, &asset, &primary, closure) {
        Ok(snapshot) => snapshot,
        Err(SnapshotError::Source) => {
            return conformance_error(
                "prediction dependency closure does not match authorized root",
            );
        }
        Err(SnapshotError::Temporary) => {
            return operator_error("cannot create private verified snapshot");
        }
    };
    #[cfg(feature = "test-support")]
    if env::var_os("ANIMSMITH_BEVY_READBACK_TEST_MUTATE_ORIGINAL_AFTER_SNAPSHOT").is_some() {
        let Ok(primary_path) = rooted_file(&root, Path::new(&asset)) else {
            return operator_error("cannot locate original source for mutation");
        };
        if OpenOptions::new()
            .append(true)
            .open(primary_path)
            .and_then(|mut file| file.write_all(b"mutation after snapshot"))
            .is_err()
        {
            return operator_error("cannot inject original-source mutation");
        }
    }
    #[cfg(feature = "test-support")]
    if let Some(key) =
        env::var_os("ANIMSMITH_BEVY_READBACK_TEST_MUTATE_ORIGINAL_DEPENDENCY_AFTER_SNAPSHOT")
    {
        let Some(key) = key
            .to_str()
            .filter(|key| safe_relative_path(Path::new(key)))
        else {
            return operator_error("invalid original-dependency mutation key");
        };
        if OpenOptions::new()
            .append(true)
            .open(root.join(key))
            .and_then(|mut file| file.write_all(b"mutation after snapshot"))
            .is_err()
        {
            return operator_error("cannot inject original-dependency mutation");
        }
    }
    let Some(asset_root) = snapshot.root_str().map(str::to_owned) else {
        return operator_error("private snapshot root is not UTF-8");
    };
    let provenance_schema = adapter.prediction_provenance().contract_id().into();
    let observed_provenance_identity = adapter
        .prediction_provenance()
        .identity()
        .input_identity()
        .clone();
    #[cfg(feature = "test-support")]
    let mut provenance_identity = observed_provenance_identity;
    #[cfg(not(feature = "test-support"))]
    let provenance_identity = observed_provenance_identity;
    #[cfg(feature = "test-support")]
    let mut referenced_prediction = prediction_input.clone();
    #[cfg(not(feature = "test-support"))]
    let referenced_prediction = prediction_input.clone();
    #[cfg(feature = "test-support")]
    if let Ok(mismatch) = env::var("ANIMSMITH_BEVY_READBACK_TEST_REFERENCE_MISMATCH") {
        match mismatch.as_str() {
            "prediction_document" => {
                referenced_prediction = animsmith_core::InputIdentity::from_bytes(
                    b"deliberately different strict prediction document",
                );
            }
            "provenance" => {
                provenance_identity = animsmith_core::InputIdentity::from_bytes(
                    b"deliberately different prediction provenance",
                );
            }
            _ => return operator_error("invalid prediction-reference mismatch injection"),
        }
    }
    let reference = animsmith_engine::BevyPredictionReferenceV1::new(
        referenced_prediction,
        provenance_schema,
        provenance_identity,
    );
    let lock = animsmith_core::InputIdentity::from_bytes(include_bytes!("../Cargo.lock"));
    let warnings = RedactedWarnings::default();
    if tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(warnings.clone()),
    )
    .is_err()
    {
        return operator_error("cannot install bounded warning capture");
    }
    let mut app = App::new();
    app.add_plugins((
        TaskPoolPlugin::default(),
        AssetPlugin {
            file_path: asset_root,
            ..Default::default()
        },
        WorldSerializationPlugin,
        GltfPlugin::default(),
    ));
    app.init_asset::<AnimationClip>();
    app.init_asset::<Image>();
    app.init_asset::<SkinnedMeshInverseBindposes>();
    // `App::run` normally performs this after plugin construction. This
    // bounded manual-update harness must finish plugin registration itself.
    app.finish();
    #[cfg(feature = "test-support")]
    let load_asset = if env::var_os("ANIMSMITH_BEVY_READBACK_TEST_MISSING_ROOT_LABEL").is_some() {
        format!("{asset}#MissingAuditLabel")
    } else {
        asset.clone()
    };
    #[cfg(not(feature = "test-support"))]
    let load_asset = asset.clone();
    let handle: Handle<Gltf> = app
        .world()
        .resource::<AssetServer>()
        .load_builder()
        .with_settings(|settings: &mut GltfLoaderSettings| {
            settings.include_source = true;
            settings.load_animations = true;
        })
        .load(load_asset);
    let mut inverse_handles: Option<Vec<Handle<SkinnedMeshInverseBindposes>>> = None;
    // The inclusive range starts at one so the reported count is the exact
    // number of `App::update` calls, never an off-by-one loop index.
    for update_count in 1..=max_updates() {
        app.update();
        // Asset I/O runs on Bevy's task pool; avoid starving it in this loop.
        std::thread::yield_now();
        let mut state = {
            let server = app.world().resource::<AssetServer>();
            match (
                server.get_load_state(handle.id()),
                server.get_recursive_dependency_load_state(handle.id()),
            ) {
                (Some(LoadState::Failed(error)), _) => {
                    Some(animsmith_engine::BevyTerminalStateV1::RootFailure {
                        error: error_code(&error),
                    })
                }
                (_, Some(RecursiveDependencyLoadState::Failed(error))) => {
                    Some(animsmith_engine::BevyTerminalStateV1::DependencyFailure {
                        error: error_code(&error),
                    })
                }
                (Some(LoadState::Loaded), Some(RecursiveDependencyLoadState::Loaded)) => {
                    Some(animsmith_engine::BevyTerminalStateV1::Loaded)
                }
                _ => None,
            }
        };
        if matches!(state, Some(animsmith_engine::BevyTerminalStateV1::Loaded)) {
            if inverse_handles.is_none() {
                let Some(gltf) = app.world().resource::<Assets<Gltf>>().get(&handle) else {
                    continue;
                };
                let Some(source) = gltf.source.as_ref() else {
                    return operator_error("loaded glTF omitted requested source inventory");
                };
                let server = app.world().resource::<AssetServer>();
                inverse_handles = Some(
                    source
                        .skins()
                        .map(|skin| {
                            server.load(
                                GltfAssetLabel::InverseBindMatrices(skin.index())
                                    .from_asset(asset.clone()),
                            )
                        })
                        .collect(),
                );
                if inverse_handles
                    .as_ref()
                    .is_some_and(|handles| !handles.is_empty())
                {
                    continue;
                }
            }
            let server = app.world().resource::<AssetServer>();
            let mut all_loaded = true;
            for inverse in inverse_handles.iter().flatten() {
                match server.get_load_state(inverse.id()) {
                    Some(LoadState::Loaded) => {}
                    Some(LoadState::Failed(error)) => {
                        state = Some(animsmith_engine::BevyTerminalStateV1::DependencyFailure {
                            error: error_code(&error),
                        });
                        break;
                    }
                    _ => all_loaded = false,
                }
            }
            if state
                .as_ref()
                .is_some_and(|value| matches!(value, animsmith_engine::BevyTerminalStateV1::Loaded))
                && !all_loaded
            {
                continue;
            }
        }
        let Some(state) = state else {
            continue;
        };
        let Some((warnings, warnings_truncated)) = warnings.snapshot() else {
            return operator_error("cannot read bounded warning capture");
        };
        let observation = if matches!(state, animsmith_engine::BevyTerminalStateV1::Loaded) {
            match observe(
                &app,
                &handle,
                inverse_handles.as_deref().unwrap_or_default(),
                state,
                warnings,
                warnings_truncated,
                true,
                true,
            ) {
                Some(observation) => observation,
                None => return operator_error("loaded asset is unavailable for observation"),
            }
        } else {
            empty_observation(state, warnings, warnings_truncated, true, true)
        };
        if inject_post_observe_mutation(&snapshot).is_err() {
            return operator_error("cannot inject post-observation snapshot mutation");
        }
        let integrity = snapshot.verify();
        let cleanup = snapshot.cleanup();
        drop(snapshot);
        if cleanup.is_err() {
            return operator_error("cannot remove private verified snapshot");
        }
        if integrity != (true, true) {
            return conformance_error("private snapshot changed during Bevy observation");
        }
        return match publish(
            primary,
            reference,
            lock,
            update_count,
            observation,
            &prediction,
            &prediction_input,
        ) {
            Ok(exit) => exit,
            Err(_) => operator_error("cannot form validated readback"),
        };
    }
    let Some((warnings, warnings_truncated)) = warnings.snapshot() else {
        return operator_error("cannot read bounded warning capture");
    };
    let observation = empty_observation(
        animsmith_engine::BevyTerminalStateV1::WorkLimit,
        warnings,
        warnings_truncated,
        true,
        true,
    );
    if inject_post_observe_mutation(&snapshot).is_err() {
        return operator_error("cannot inject post-observation snapshot mutation");
    }
    let integrity = snapshot.verify();
    let cleanup = snapshot.cleanup();
    drop(snapshot);
    if cleanup.is_err() {
        return operator_error("cannot remove private verified snapshot");
    }
    if integrity != (true, true) {
        return conformance_error("private snapshot changed during Bevy observation");
    }
    match publish(
        primary,
        reference,
        lock,
        max_updates(),
        observation,
        &prediction,
        &prediction_input,
    ) {
        Ok(exit) => exit,
        Err(_) => operator_error("cannot form validated readback"),
    }
}

#[cfg(feature = "test-support")]
fn inject_post_observe_mutation(snapshot: &VerifiedSnapshot) -> io::Result<()> {
    if env::var_os("ANIMSMITH_BEVY_READBACK_TEST_MUTATE_SNAPSHOT_AFTER_OBSERVE").is_some() {
        snapshot.corrupt_primary()?;
    }
    Ok(())
}

#[cfg(not(feature = "test-support"))]
fn inject_post_observe_mutation(_: &VerifiedSnapshot) -> io::Result<()> {
    Ok(())
}

fn max_updates() -> u64 {
    #[cfg(feature = "test-support")]
    if let Some(value) = env::var("ANIMSMITH_BEVY_READBACK_TEST_MAX_UPDATES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        return value.min(animsmith_engine::BEVY_READBACK_V1_MAX_UPDATES);
    }
    animsmith_engine::BEVY_READBACK_V1_MAX_UPDATES
}

fn publish(
    primary: animsmith_core::InputIdentity,
    reference: animsmith_engine::BevyPredictionReferenceV1,
    lock: animsmith_core::InputIdentity,
    updates: u64,
    observation: animsmith_engine::BevyObservationV1,
    prediction: &animsmith_engine::GltfAddressabilityReadbackV2,
    prediction_input: &animsmith_core::InputIdentity,
) -> Result<ExitCode, animsmith_engine::BevyReadbackV1Error> {
    let harness = || {
        animsmith_engine::BevyHarnessIdentityV1::new(
            TOOL_VERSION.into(),
            RUSTC_VERSION.into(),
            true,
            true,
            lock.clone(),
            updates,
        )
    };
    let provisional = animsmith_engine::BevyReadbackV1::new(
        harness(),
        primary.clone(),
        reference.clone(),
        observation.clone(),
        animsmith_engine::BevyConformanceV1::NotExact {
            mismatch_codes: vec![animsmith_engine::BevyConformanceCodeV1::LoadDidNotSucceed],
            unavailable_codes: Vec::new(),
        },
    )?;
    let conformance =
        animsmith_engine::compare_bevy_readback_v1(&provisional, prediction, prediction_input);
    let readback = animsmith_engine::BevyReadbackV1::new(
        harness(),
        primary,
        reference,
        observation,
        conformance.clone(),
    )?;
    if animsmith_engine::validate_bevy_readback_prediction_v1(
        &readback,
        prediction,
        prediction_input,
    )
    .is_err()
    {
        return Ok(ExitCode::from(2));
    }
    let Ok(bytes) = serde_json::to_vec(&readback) else {
        return Ok(ExitCode::from(2));
    };
    if bytes.len() as u64 > animsmith_engine::BEVY_READBACK_V1_MAX_REPORT_BYTES
        || std::io::stdout().lock().write_all(&bytes).is_err()
    {
        return Ok(ExitCode::from(2));
    }
    Ok(
        if matches!(conformance, animsmith_engine::BevyConformanceV1::Exact) {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn observe(
    app: &App,
    handle: &Handle<Gltf>,
    inverse_handles: &[Handle<SkinnedMeshInverseBindposes>],
    state: animsmith_engine::BevyTerminalStateV1,
    warnings: Vec<animsmith_engine::BevyWarningV1>,
    warnings_truncated: bool,
    primary_verified: bool,
    dependencies_verified: bool,
) -> Option<animsmith_engine::BevyObservationV1> {
    let world = app.world();
    let gltfs = world.get_resource::<Assets<Gltf>>()?;
    let clips = world.get_resource::<Assets<AnimationClip>>()?;
    let gltf_nodes = world.get_resource::<Assets<GltfNode>>()?;
    let gltf_skins = world.get_resource::<Assets<GltfSkin>>()?;
    let server = world.get_resource::<AssetServer>()?;
    let gltf = gltfs.get(handle)?;
    let root_path = server.get_path(handle.id())?.path().to_owned();
    let mut animations = gltf
        .animations
        .iter()
        .filter_map(|handle| {
            observed_indexed_label(server, handle.id(), &root_path, "Animation", "")
        })
        .collect::<Vec<_>>();
    animations.sort_by_key(animsmith_engine::BevyIndexedLabelV1::index);
    let mut named = gltf
        .named_animations
        .iter()
        .filter_map(|(name, winner)| {
            observed_index(server, winner.id(), &root_path, "Animation", "")
                .map(|index| animsmith_engine::BevyNamedWinnerV1::new(name.to_string(), index))
        })
        .collect::<Vec<_>>();
    named.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    let mut named_scenes = gltf
        .named_scenes
        .iter()
        .filter_map(|(name, winner)| {
            observed_index(server, winner.id(), &root_path, "Scene", "")
                .map(|index| animsmith_engine::BevyNamedWinnerV1::new(name.to_string(), index))
        })
        .collect::<Vec<_>>();
    named_scenes.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    let mut named_skins = gltf
        .named_skins
        .iter()
        .filter_map(|(name, winner)| {
            let skin = gltf_skins.get(winner)?;
            let index = observed_index(server, winner.id(), &root_path, "Skin", "")?;
            (index == skin.index as u32)
                .then(|| animsmith_engine::BevyNamedWinnerV1::new(name.to_string(), index))
        })
        .collect::<Vec<_>>();
    named_skins.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    let observed_default_scene = gltf
        .default_scene
        .as_ref()
        .and_then(|winner| observed_index(server, winner.id(), &root_path, "Scene", ""));
    let mut scenes = gltf
        .scenes
        .iter()
        .filter_map(|handle| observed_indexed_label(server, handle.id(), &root_path, "Scene", ""))
        .collect::<Vec<_>>();
    scenes.sort_by_key(animsmith_engine::BevyIndexedLabelV1::index);
    let mut nodes = gltf
        .nodes
        .iter()
        .filter_map(|handle| {
            let node = gltf_nodes.get(handle)?;
            let observed = observed_indexed_label(server, handle.id(), &root_path, "Node", "")?;
            (observed.index() == node.index as u32).then_some(observed)
        })
        .collect::<Vec<_>>();
    nodes.sort_by_key(animsmith_engine::BevyIndexedLabelV1::index);
    let mut skins = gltf
        .skins
        .iter()
        .filter_map(|handle| {
            let skin = gltf_skins.get(handle)?;
            let observed = observed_indexed_label(server, handle.id(), &root_path, "Skin", "")?;
            (observed.index() == skin.index as u32).then_some(observed)
        })
        .collect::<Vec<_>>();
    skins.sort_by_key(animsmith_engine::BevyIndexedLabelV1::index);
    let mut inverse_bind_matrices = inverse_handles
        .iter()
        .filter_map(|handle| {
            observed_indexed_label(
                server,
                handle.id(),
                &root_path,
                "Skin",
                "/InverseBindMatrices",
            )
        })
        .collect::<Vec<_>>();
    inverse_bind_matrices.sort_by_key(animsmith_engine::BevyIndexedLabelV1::index);
    let mut targets = Vec::new();
    for handle in &gltf.animations {
        if let (Some(index), Some(clip)) = (
            observed_index(server, handle.id(), &root_path, "Animation", ""),
            clips.get(handle),
        ) {
            for id in clip.curves().keys() {
                targets.push(animsmith_engine::BevyAnimationTargetV1::new(
                    index,
                    id.0.to_string(),
                ));
            }
        }
    }
    targets.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    targets.dedup();
    #[cfg(feature = "test-support")]
    let mut default_scene = observed_default_scene;
    #[cfg(not(feature = "test-support"))]
    let default_scene = observed_default_scene;
    #[cfg(feature = "test-support")]
    if let Ok(mutation) = env::var("ANIMSMITH_BEVY_READBACK_TEST_MUTATE_OBSERVATION") {
        match mutation.as_str() {
            "inventory" => {
                animations.pop();
            }
            "scene" => {
                scenes.pop();
            }
            "default_scene" => {
                default_scene = None;
            }
            "skin" => {
                skins.pop();
            }
            "named" => {
                named.clear();
                named_scenes.clear();
                named_skins.clear();
            }
            "target" => {
                targets.pop();
            }
            _ => return None,
        }
    }
    Some(animsmith_engine::BevyObservationV1::new(
        state,
        animations,
        named,
        named_scenes,
        named_skins,
        default_scene,
        scenes,
        nodes,
        skins,
        inverse_bind_matrices,
        targets,
        warnings,
        warnings_truncated,
        primary_verified,
        dependencies_verified,
    ))
}

fn observed_indexed_label(
    id_server: &AssetServer,
    id: impl Into<bevy::asset::UntypedAssetId>,
    root_path: &Path,
    prefix: &str,
    suffix: &str,
) -> Option<animsmith_engine::BevyIndexedLabelV1> {
    let path = id_server.get_path(id)?;
    if path.path() != root_path {
        return None;
    }
    let label = path.label()?;
    let index = parse_observed_label(label, prefix, suffix)?;
    Some(animsmith_engine::BevyIndexedLabelV1::new(
        index,
        label.to_owned(),
    ))
}
fn observed_index(
    id_server: &AssetServer,
    id: impl Into<bevy::asset::UntypedAssetId>,
    root_path: &Path,
    prefix: &str,
    suffix: &str,
) -> Option<u32> {
    let path = id_server.get_path(id)?;
    (path.path() == root_path).then_some(())?;
    parse_observed_label(path.label()?, prefix, suffix)
}
fn parse_observed_label(label: &str, prefix: &str, suffix: &str) -> Option<u32> {
    label
        .strip_prefix(prefix)?
        .strip_suffix(suffix)?
        .parse()
        .ok()
}
fn empty_observation(
    state: animsmith_engine::BevyTerminalStateV1,
    warnings: Vec<animsmith_engine::BevyWarningV1>,
    warnings_truncated: bool,
    primary_verified: bool,
    dependencies_verified: bool,
) -> animsmith_engine::BevyObservationV1 {
    animsmith_engine::BevyObservationV1::new(
        state,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        warnings,
        warnings_truncated,
        primary_verified,
        dependencies_verified,
    )
}
fn rooted_file(root: &Path, relative: &Path) -> Result<PathBuf, ()> {
    let path = fs::canonicalize(root.join(relative)).map_err(|_| ())?;
    if path.starts_with(root) && path.is_file() {
        Ok(path)
    } else {
        Err(())
    }
}
fn safe_key(value: &str) -> bool {
    (value.ends_with(".gltf") || value.ends_with(".glb"))
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value
            .chars()
            .any(|character| character.is_control() || matches!(character, '#' | '?' | '%' | ':'))
        && value
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".."))
}
fn usage() -> ExitCode {
    eprintln!(
        "usage: animsmith-bevy-readback --asset-root <dir> --asset <relative.gltf|relative.glb> --prediction <strict-addressability-v2.json>"
    );
    ExitCode::from(2)
}
fn operator_error(message: &str) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(2)
}
fn conformance_error(message: &str) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(1)
}
fn error_code(error: &AssetLoadError) -> animsmith_engine::BevyLoadErrorCodeV1 {
    use animsmith_engine::BevyLoadErrorCodeV1 as C;
    match error {
        AssetLoadError::EmptyPath(_) => C::EmptyPath,
        AssetLoadError::RequestedHandleTypeMismatch { .. } => C::RequestedHandleTypeMismatch,
        AssetLoadError::MissingAssetLoader { .. } => C::MissingAssetLoader,
        AssetLoadError::MissingAssetLoaderForExtension(_) => C::MissingAssetLoaderForExtension,
        AssetLoadError::MissingAssetLoaderForTypeName(_) => C::MissingAssetLoaderForTypeName,
        AssetLoadError::MissingAssetLoaderForTypeIdError(_) => C::MissingAssetLoaderForTypeId,
        AssetLoadError::AssetReaderError(_) => C::AssetReader,
        AssetLoadError::MissingAssetSourceError(_) => C::MissingAssetSource,
        AssetLoadError::MissingProcessedAssetReaderError(_) => C::MissingProcessedAssetReader,
        AssetLoadError::AssetMetaReadError => C::AssetMetadata,
        AssetLoadError::DeserializeMeta { .. } => C::DeserializeMetadata,
        AssetLoadError::CannotLoadProcessedAsset { .. } => C::CannotLoadProcessedAsset,
        AssetLoadError::CannotLoadIgnoredAsset { .. } => C::CannotLoadIgnoredAsset,
        AssetLoadError::AssetLoaderPanic { .. } => C::LoaderPanic,
        AssetLoadError::AssetLoaderError(_) => C::Loader,
        AssetLoadError::AddAsyncError(_) => C::AddAsync,
        AssetLoadError::MissingLabel { .. } => C::MissingLabel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn warning_capture_retains_only_bounded_redacted_metadata() {
        let capture = RedactedWarnings::default();
        let dispatch = tracing::Dispatch::new(tracing_subscriber::registry().with(capture.clone()));
        tracing::dispatcher::with_default(&dispatch, || {
            tracing::warn!(target: "fixture.warning", private_path = "/not/retained", "not retained");
            tracing::error!(target: "fixture.error", secret = "not retained", "not retained");
            tracing::info!(target: "fixture.info", "not retained");
        });
        let (warnings, truncated) = capture.snapshot().unwrap();
        assert_eq!(warnings.len(), 2);
        assert!(!truncated);
        let json = serde_json::to_string(&warnings).unwrap();
        assert!(json.contains("fixture.warning"));
        assert!(json.contains("fixture.error"));
        assert!(!json.contains("not/retained"));
        assert!(!json.contains("not retained"));
    }

    #[test]
    fn safe_key_rejects_asset_path_syntax_and_control_characters() {
        for key in ["rig.gltf", "nested/rig.glb", "space name.gltf"] {
            assert!(safe_key(key), "expected {key:?} to be accepted");
        }
        for key in [
            "#scene.gltf",
            "rig?.gltf",
            "rig%.gltf",
            "source:rig.gltf",
            "rig\n.gltf",
            "nested/../rig.glb",
            "/absolute.gltf",
            "windows\\rig.glb",
        ] {
            assert!(!safe_key(key), "expected {key:?} to be rejected");
        }
    }

    #[test]
    fn prediction_reader_rejects_oversize_regular_file_before_reading_it() {
        let root = test_root();
        let path = root.join("oversize.json");
        let file = File::create(&path).unwrap();
        file.set_len(animsmith_engine::GLTF_ADDRESSABILITY_V2_MAX_REPORT_BYTES + 1)
            .unwrap();
        assert!(matches!(
            read_prediction(&path),
            Err(PredictionInputError::TooLarge)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn prediction_reader_rejects_special_files_before_reading_them() {
        assert!(matches!(
            read_prediction("/dev/zero"),
            Err(PredictionInputError::NotRegular)
        ));
    }

    #[test]
    fn build_records_the_compiler_that_actually_compiled_the_harness() {
        assert_eq!(RUSTC_VERSION, animsmith_engine::BEVY_READBACK_V1_RUSTC);
    }

    #[test]
    fn warning_capture_reports_events_beyond_the_bounded_prefix() {
        let capture = RedactedWarnings::default();
        let dispatch = tracing::Dispatch::new(tracing_subscriber::registry().with(capture.clone()));
        tracing::dispatcher::with_default(&dispatch, || {
            for _ in 0..=MAX_WARNINGS {
                tracing::warn!(target: "fixture.warning", "not retained");
            }
        });
        let (warnings, truncated) = capture.snapshot().unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(truncated);
    }

    fn test_root() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("animsmith-bevy-readback-{unique}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_app(root: &Path) -> App {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin {
                file_path: root.to_str().unwrap().to_owned(),
                ..Default::default()
            },
            WorldSerializationPlugin,
            GltfPlugin::default(),
        ));
        app.init_asset::<AnimationClip>();
        app.init_asset::<Image>();
        app.init_asset::<SkinnedMeshInverseBindposes>();
        app.finish();
        app
    }

    fn wait_state(
        app: &mut App,
        handle: &Handle<Gltf>,
    ) -> (Option<LoadState>, Option<RecursiveDependencyLoadState>) {
        for _ in 0..256 {
            app.update();
            std::thread::sleep(Duration::from_millis(1));
            let server = app.world().resource::<AssetServer>();
            let root = server.get_load_state(handle.id());
            let recursive = server.get_recursive_dependency_load_state(handle.id());
            if root.as_ref().is_some_and(LoadState::is_failed)
                || recursive
                    .as_ref()
                    .is_some_and(RecursiveDependencyLoadState::is_failed)
            {
                return (root, recursive);
            }
        }
        panic!("stock AssetServer did not reach a failure state within bounded updates");
    }

    #[test]
    fn stock_asset_server_reports_missing_root_as_root_failure() {
        let root = test_root();
        let mut app = test_app(&root);
        let handle: Handle<Gltf> = app.world().resource::<AssetServer>().load("missing.glb");
        let (state, _) = wait_state(&mut app, &handle);
        assert!(matches!(state, Some(LoadState::Failed(_))));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stock_asset_server_reports_missing_image_dependency_recursively() {
        let root = test_root();
        fs::write(root.join("dependency.gltf"), r#"{"asset":{"version":"2.0"},"images":[{"uri":"missing.png"}],"textures":[{"source":0}]}"#).unwrap();
        let mut app = test_app(&root);
        let handle: Handle<Gltf> = app
            .world()
            .resource::<AssetServer>()
            .load("dependency.gltf");
        let (root_state, recursive) = wait_state(&mut app, &handle);
        assert!(matches!(root_state, Some(LoadState::Loaded)));
        assert!(matches!(
            recursive,
            Some(RecursiveDependencyLoadState::Failed(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
