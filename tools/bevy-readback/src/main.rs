//! Owner-run, headless exact-Bevy observation executable.

use bevy::{
    app::TaskPoolPlugin,
    asset::{AssetApp, AssetLoadError, AssetPlugin, AssetServer, Assets, LoadState, RecursiveDependencyLoadState},
    gltf::{Gltf, GltfPlugin},
    image::Image,
    prelude::{AnimationClip, App, Handle},
    world_serialization::WorldSerializationPlugin,
};
use std::{env, fs, io::Cursor, path::{Path, PathBuf}, process::ExitCode};
use std::sync::{Arc, Mutex};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan, Layer};

const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
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

impl<S> Layer<S> for RedactedWarnings where S: Subscriber + for<'a> LookupSpan<'a> {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let metadata = event.metadata();
        if !matches!(*metadata.level(), Level::WARN | Level::ERROR) { return; }
        let Ok(mut capture) = self.0.lock() else { return; };
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

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(a), Some(root_arg), Some(b), Some(asset), Some(c), Some(prediction_path)) = (args.next(), args.next(), args.next(), args.next(), args.next(), args.next()) else { return usage(); };
    if a != "--asset-root" || b != "--asset" || c != "--prediction" || args.next().is_some() || !safe_key(&asset) { return usage(); }
    let root = match fs::canonicalize(root_arg) { Ok(path) if path.is_dir() => path, _ => return operator_error("cannot access authorized asset root") };
    let Some(asset_root) = root.to_str().map(str::to_owned) else { return operator_error("authorized asset root is not UTF-8"); };
    let primary_path = match rooted_file(&root, Path::new(&asset)) { Ok(path) => path, Err(()) => return operator_error("asset is not a readable file below authorized root") };
    let bytes = match fs::read(prediction_path) { Ok(bytes) => bytes, Err(_) => return operator_error("cannot read prediction") };
    let prediction_input = animsmith_core::InputIdentity::from_bytes(&bytes);
    let prediction = match animsmith_engine::GltfAddressabilityV2::read_from(Cursor::new(&bytes)) { Ok(value) => value, Err(_) => return operator_error("prediction is not strict rich addressability V2") };
    let Some(adapter) = prediction.bevy() else { return conformance_error("prediction has no exact Bevy revision-3 adapter"); };
    let primary_bytes = match fs::read(&primary_path) { Ok(bytes) => bytes, Err(_) => return operator_error("cannot read primary asset") };
    let primary = animsmith_core::InputIdentity::from_bytes(&primary_bytes);
    if primary != *prediction.input() || primary != *adapter.prediction_provenance().raw_source().primary_input() { return conformance_error("primary artifact does not match strict prediction identity"); }
    if !verify_closure(&root, adapter.prediction_provenance().dependency_closure()) { return conformance_error("prediction dependency closure does not match authorized root"); }
    let reference = animsmith_engine::BevyPredictionReferenceV1::new(prediction_input.clone(), adapter.prediction_provenance().contract_id().into(), adapter.prediction_provenance().identity().input_identity().clone());
    let lock = animsmith_core::InputIdentity::from_bytes(include_bytes!("../Cargo.lock"));
    let warnings = RedactedWarnings::default();
    if tracing::subscriber::set_global_default(tracing_subscriber::registry().with(warnings.clone())).is_err() { return operator_error("cannot install bounded warning capture"); }
    let mut app = App::new();
    app.add_plugins((TaskPoolPlugin::default(), AssetPlugin { file_path: asset_root, ..Default::default() }, WorldSerializationPlugin, GltfPlugin::default()));
    app.init_asset::<AnimationClip>();
    app.init_asset::<Image>();
    // `App::run` normally performs this after plugin construction. This
    // bounded manual-update harness must finish plugin registration itself.
    app.finish();
    let handle: Handle<Gltf> = app.world().resource::<AssetServer>().load(asset);
    for update_count in 0..=max_updates() {
        app.update();
        // Asset I/O runs on Bevy's task pool; avoid starving it in this loop.
        std::thread::yield_now();
        let state = { let server = app.world().resource::<AssetServer>(); match (server.get_load_state(handle.id()), server.get_recursive_dependency_load_state(handle.id())) { (Some(LoadState::Failed(error)), _) => Some(animsmith_engine::BevyTerminalStateV1::RootFailure { error: error_code(&error) }), (_, Some(RecursiveDependencyLoadState::Failed(error))) => Some(animsmith_engine::BevyTerminalStateV1::DependencyFailure { error: error_code(&error) }), (Some(LoadState::Loaded), Some(RecursiveDependencyLoadState::Loaded)) => Some(animsmith_engine::BevyTerminalStateV1::Loaded), _ => None } };
        let Some(state) = state else { continue; };
        let Some((warnings, warnings_truncated)) = warnings.snapshot() else {
            return operator_error("cannot read bounded warning capture");
        };
        let observation = if matches!(state, animsmith_engine::BevyTerminalStateV1::Loaded) {
            match observe(&app, &handle, state, warnings, warnings_truncated) {
                Some(observation) => observation,
                None => return operator_error("loaded asset is unavailable for observation"),
            }
        } else {
            empty_observation(state, warnings, warnings_truncated)
        };
        return match publish(primary, reference, lock, update_count, observation, &prediction, &prediction_input) {
            Ok(exit) => exit,
            Err(_) => operator_error("cannot form validated readback"),
        };
    }
    let Some((warnings, warnings_truncated)) = warnings.snapshot() else {
        return operator_error("cannot read bounded warning capture");
    };
    match publish(primary, reference, lock, max_updates(), empty_observation(animsmith_engine::BevyTerminalStateV1::WorkLimit, warnings, warnings_truncated), &prediction, &prediction_input) {
        Ok(exit) => exit,
        Err(_) => operator_error("cannot form validated readback"),
    }
}

fn max_updates() -> u64 {
    #[cfg(feature = "test-support")]
    if let Some(value) = env::var("ANIMSMITH_BEVY_READBACK_TEST_MAX_UPDATES").ok().and_then(|value| value.parse::<u64>().ok()) {
        return value.min(animsmith_engine::BEVY_READBACK_V1_MAX_UPDATES);
    }
    animsmith_engine::BEVY_READBACK_V1_MAX_UPDATES
}

fn publish(primary: animsmith_core::InputIdentity, reference: animsmith_engine::BevyPredictionReferenceV1, lock: animsmith_core::InputIdentity, updates: u64, observation: animsmith_engine::BevyObservationV1, prediction: &animsmith_engine::GltfAddressabilityReadbackV2, prediction_input: &animsmith_core::InputIdentity) -> Result<ExitCode, animsmith_engine::BevyReadbackV1Error> {
    let harness = || animsmith_engine::BevyHarnessIdentityV1::new(TOOL_VERSION.into(), "rustc 1.95.0".into(), lock.clone(), updates);
    let provisional = animsmith_engine::BevyReadbackV1::new(harness(), primary.clone(), reference.clone(), observation.clone(), animsmith_engine::BevyConformanceV1::NotExact { mismatch_codes: vec![animsmith_engine::BevyConformanceCodeV1::LoadDidNotSucceed], unavailable_codes: Vec::new() })?;
    let conformance = animsmith_engine::compare_bevy_readback_v1(&provisional, prediction, prediction_input);
    let readback = animsmith_engine::BevyReadbackV1::new(harness(), primary, reference, observation, conformance.clone())?;
    if animsmith_engine::validate_bevy_readback_prediction_v1(&readback, prediction, prediction_input).is_err() { return Ok(ExitCode::from(2)); }
    if serde_json::to_writer(std::io::stdout(), &readback).is_err() { return Ok(ExitCode::from(2)); }
    Ok(if matches!(conformance, animsmith_engine::BevyConformanceV1::Exact) { ExitCode::SUCCESS } else { ExitCode::from(1) })
}

fn observe(app: &App, handle: &Handle<Gltf>, state: animsmith_engine::BevyTerminalStateV1, warnings: Vec<animsmith_engine::BevyWarningV1>, warnings_truncated: bool) -> Option<animsmith_engine::BevyObservationV1> {
    let world = app.world();
    let gltfs = world.get_resource::<Assets<Gltf>>()?;
    let clips = world.get_resource::<Assets<AnimationClip>>()?;
    let gltf = gltfs.get(handle)?;
    let animations = (0..gltf.animations.len()).map(|i| animsmith_engine::BevyIndexedLabelV1::new(i as u32, format!("Animation{i}"))).collect();
    let mut named = gltf.named_animations.iter().filter_map(|(name, winner)| gltf.animations.iter().position(|handle| handle.id() == winner.id()).map(|index| animsmith_engine::BevyNamedWinnerV1::new(name.to_string(), index as u32))).collect::<Vec<_>>();
    named.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    let mut named_scenes = gltf.named_scenes.iter().filter_map(|(name, winner)| gltf.scenes.iter().position(|handle| handle.id() == winner.id()).map(|index| animsmith_engine::BevyNamedWinnerV1::new(name.to_string(), index as u32))).collect::<Vec<_>>();
    named_scenes.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    let mut named_skins = gltf.named_skins.iter().filter_map(|(name, winner)| gltf.skins.iter().position(|handle| handle.id() == winner.id()).map(|index| animsmith_engine::BevyNamedWinnerV1::new(name.to_string(), index as u32))).collect::<Vec<_>>();
    named_skins.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    let default_scene = gltf.default_scene.as_ref().and_then(|winner| gltf.scenes.iter().position(|handle| handle.id() == winner.id())).map(|index| index as u32);
    let scenes = (0..gltf.scenes.len()).map(|i| animsmith_engine::BevyIndexedLabelV1::new(i as u32, format!("Scene{i}"))).collect();
    let nodes = (0..gltf.nodes.len()).map(|i| animsmith_engine::BevyIndexedLabelV1::new(i as u32, format!("Node{i}"))).collect();
    let skins = (0..gltf.skins.len()).map(|i| animsmith_engine::BevyIndexedLabelV1::new(i as u32, format!("Skin{i}"))).collect();
    let inverse_bind_matrices = (0..gltf.skins.len()).map(|i| animsmith_engine::BevyIndexedLabelV1::new(i as u32, format!("Skin{i}/InverseBindMatrices"))).collect();
    let mut targets = Vec::new(); for (i, handle) in gltf.animations.iter().enumerate() { if let Some(clip) = clips.get(handle) { for id in clip.curves().keys() { targets.push(animsmith_engine::BevyAnimationTargetV1::new(i as u32, id.0.to_string())); } } }
    targets.sort_by(|left, right| left.sort_key().cmp(&right.sort_key())); targets.dedup();
    Some(animsmith_engine::BevyObservationV1::new(state, animations, named, named_scenes, named_skins, default_scene, scenes, nodes, skins, inverse_bind_matrices, targets, warnings, warnings_truncated, true, true))
}

fn empty_observation(state: animsmith_engine::BevyTerminalStateV1, warnings: Vec<animsmith_engine::BevyWarningV1>, warnings_truncated: bool) -> animsmith_engine::BevyObservationV1 { animsmith_engine::BevyObservationV1::new(state, Vec::new(), Vec::new(), Vec::new(), Vec::new(), None, Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), warnings, warnings_truncated, true, true) }
fn verify_closure(root: &Path, closure: &animsmith_core::DependencyClosureV1) -> bool { closure.coverage().is_complete() && closure.external_resources().iter().all(|resource| rooted_file(root, Path::new(resource.key().as_str())).ok().and_then(|path| fs::read(path).ok()).is_some_and(|bytes| animsmith_core::InputIdentity::from_bytes(&bytes) == *resource.identity())) }
fn rooted_file(root: &Path, relative: &Path) -> Result<PathBuf, ()> { let path = fs::canonicalize(root.join(relative)).map_err(|_| ())?; if path.starts_with(root) && path.is_file() { Ok(path) } else { Err(()) } }
fn safe_key(value: &str) -> bool {
    (value.ends_with(".gltf") || value.ends_with(".glb"))
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value
            .chars()
            .any(|character| character.is_control() || matches!(character, '#' | '?' | '%' | ':'))
        && value.split('/').all(|part| !matches!(part, "" | "." | ".."))
}
fn usage() -> ExitCode { eprintln!("usage: animsmith-bevy-readback --asset-root <dir> --asset <relative.gltf|relative.glb> --prediction <strict-addressability-v2.json>"); ExitCode::from(2) }
fn operator_error(message: &str) -> ExitCode { eprintln!("{message}"); ExitCode::from(2) }
fn conformance_error(message: &str) -> ExitCode { eprintln!("{message}"); ExitCode::from(1) }
fn error_code(error: &AssetLoadError) -> animsmith_engine::BevyLoadErrorCodeV1 { use animsmith_engine::BevyLoadErrorCodeV1 as C; match error { AssetLoadError::EmptyPath(_) => C::EmptyPath, AssetLoadError::RequestedHandleTypeMismatch { .. } => C::RequestedHandleTypeMismatch, AssetLoadError::MissingAssetLoader { .. } => C::MissingAssetLoader, AssetLoadError::MissingAssetLoaderForExtension(_) => C::MissingAssetLoaderForExtension, AssetLoadError::MissingAssetLoaderForTypeName(_) => C::MissingAssetLoaderForTypeName, AssetLoadError::MissingAssetLoaderForTypeIdError(_) => C::MissingAssetLoaderForTypeId, AssetLoadError::AssetReaderError(_) => C::AssetReader, AssetLoadError::MissingAssetSourceError(_) => C::MissingAssetSource, AssetLoadError::MissingProcessedAssetReaderError(_) => C::MissingProcessedAssetReader, AssetLoadError::AssetMetaReadError => C::AssetMetadata, AssetLoadError::DeserializeMeta { .. } => C::DeserializeMetadata, AssetLoadError::CannotLoadProcessedAsset { .. } => C::CannotLoadProcessedAsset, AssetLoadError::CannotLoadIgnoredAsset { .. } => C::CannotLoadIgnoredAsset, AssetLoadError::AssetLoaderPanic { .. } => C::LoaderPanic, AssetLoadError::AssetLoaderError(_) => C::Loader, AssetLoadError::AddAsyncError(_) => C::AddAsync, AssetLoadError::MissingLabel { .. } => C::MissingLabel } }

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
            tracing::info!(target: "fixture.info", "not retained");
        });
        let (warnings, truncated) = capture.snapshot().unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(!truncated);
        let json = serde_json::to_string(&warnings).unwrap();
        assert!(json.contains("fixture.warning"));
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
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("animsmith-bevy-readback-{unique}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_app(root: &Path) -> App {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin { file_path: root.to_str().unwrap().to_owned(), ..Default::default() }, WorldSerializationPlugin, GltfPlugin::default()));
        app.init_asset::<AnimationClip>();
        app.init_asset::<Image>();
        app.finish();
        app
    }

    fn wait_state(app: &mut App, handle: &Handle<Gltf>) -> (Option<LoadState>, Option<RecursiveDependencyLoadState>) {
        for _ in 0..256 {
            app.update();
            std::thread::sleep(Duration::from_millis(1));
            let server = app.world().resource::<AssetServer>();
            let root = server.get_load_state(handle.id());
            let recursive = server.get_recursive_dependency_load_state(handle.id());
            if root.as_ref().is_some_and(LoadState::is_failed) || recursive.as_ref().is_some_and(RecursiveDependencyLoadState::is_failed) { return (root, recursive); }
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
        let handle: Handle<Gltf> = app.world().resource::<AssetServer>().load("dependency.gltf");
        let (root_state, recursive) = wait_state(&mut app, &handle);
        assert!(matches!(root_state, Some(LoadState::Loaded)));
        assert!(matches!(recursive, Some(RecursiveDependencyLoadState::Failed(_))));
        fs::remove_dir_all(root).unwrap();
    }
}
