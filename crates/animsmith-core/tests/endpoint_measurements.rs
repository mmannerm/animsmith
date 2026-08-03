use animsmith_core::measure::{LoopEndpointMode, measure_document};
use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use animsmith_core::{Config, MetricGrids, ResolvedRoles};
use glam::Vec3;

fn document(values: [f32; 3]) -> Document {
    Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![Clip {
            name: "cycle".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 0.5, 1.0],
                values: TrackValues::Vec3s(
                    values.into_iter().map(|x| Vec3::new(x, 0.0, 0.0)).collect(),
                ),
            }],
        }],
        ..Document::default()
    }
}

fn config(looping: bool, fps: Option<f64>) -> Config {
    let mut clip = serde_json::json!({ "loop": looping });
    if let Some(fps) = fps {
        clip["fps"] = serde_json::json!(fps);
    }
    serde_json::from_value(serde_json::json!({ "clips": { "cycle": clip } }))
        .expect("valid test config")
}

fn measure(doc: &Document, config: &Config) -> animsmith_core::measure::ClipMeasurements {
    let grids = MetricGrids::new(doc);
    measure_document(&grids, &ResolvedRoles::default(), config)
        .remove("cycle")
        .expect("cycle measurements")
}

#[test]
fn loop_endpoint_modes_reuse_duplicate_analysis_and_effective_closure_caps() {
    assert_eq!(
        measure(&document([0.0, 1.0, 0.0]), &config(true, None)).loop_endpoint_mode,
        Some(LoopEndpointMode::DuplicateEndpoint),
    );
    assert_eq!(
        measure(&document([0.0, 0.005, 0.009]), &config(true, None)).loop_endpoint_mode,
        Some(LoopEndpointMode::UniqueCycle),
        "not a strict duplicate but within the inclusive 1 cm closure cap",
    );
    assert_eq!(
        measure(&document([0.0, 0.01, 0.011]), &config(true, None)).loop_endpoint_mode,
        Some(LoopEndpointMode::NonClosing),
    );

    let relaxed: Config = serde_json::from_value(serde_json::json!({
        "clips": { "cycle": { "loop": true, "max_loop_position_delta_m": 0.02 } }
    }))
    .expect("valid relaxed loop cap");
    assert_eq!(
        measure(&document([0.0, 0.01, 0.011]), &relaxed).loop_endpoint_mode,
        Some(LoopEndpointMode::UniqueCycle),
        "endpoint mode uses the effective per-clip loop-closure cap",
    );
}

#[test]
fn endpoint_mode_omits_non_loops_and_unavailable_short_evidence() {
    assert_eq!(
        measure(&document([0.0, 1.0, 0.0]), &config(false, None)).loop_endpoint_mode,
        None,
    );

    let mut short = document([0.0, 1.0, 0.0]);
    short.clips[0].tracks[0].times.truncate(2);
    let TrackValues::Vec3s(values) = &mut short.clips[0].tracks[0].values else {
        unreachable!();
    };
    values.truncate(2);
    assert_eq!(
        measure(&short, &config(true, None)).loop_endpoint_mode,
        None
    );
}

#[test]
fn valid_declared_frame_grid_reports_fps_and_intervals() {
    let grid = measure(&document([0.0, 1.0, 0.0]), &config(true, Some(30.0)))
        .frame_grid
        .expect("valid declared frame grid");
    assert_eq!(grid.fps, 30.0);
    assert_eq!(grid.frame_intervals, 30);
}

#[test]
fn off_grid_keys_and_unrepresentable_intervals_omit_frame_grid() {
    let mut off_grid = document([0.0, 1.0, 0.0]);
    off_grid.clips[0].tracks[0].times[1] = 0.51;
    assert_eq!(
        measure(&off_grid, &config(true, Some(30.0))).frame_grid,
        None
    );

    let mut overflow = document([0.0, 1.0, 0.0]);
    overflow.clips[0].duration_s = f64::from(u32::MAX) + 1.0;
    overflow.clips[0].tracks[0].times[2] = u32::MAX as f32;
    assert_eq!(
        measure(&overflow, &config(true, Some(1.0))).frame_grid,
        None
    );

    let mut zero_duration = document([0.0, 1.0, 0.0]);
    zero_duration.clips[0].duration_s = 0.0;
    assert_eq!(
        measure(&zero_duration, &config(true, Some(30.0))).frame_grid,
        None,
        "a degenerate zero-interval clip has no usable sampling grid",
    );
}
