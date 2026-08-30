#![cfg(feature = "report")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use animsmith_core::InputIdentity;
use serde_json::{Value, json};

const DASHBOARD_SCHEMA: &str =
    include_str!("../../../docs/schemas/collection-dashboard-v1.schema.json");

fn spike_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/collection-spike")
        .join(relative)
}

fn collection(manifest: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "lint",
            manifest.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("collection lint runs")
}

fn write_bevy_v3_track_config(
    dir: &Path,
    name: &str,
    bevy_animation_feature: bool,
    load_animations: bool,
) -> PathBuf {
    let path = dir.join(format!("{name}.animsmith.toml"));
    fs::write(
        &path,
        format!(
            r#"
[engine]
profile = "bevy"
profile_revision = 3
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
bevy_animation_feature = {bevy_animation_feature}
load_animations = {load_animations}
"#
        ),
    )
    .unwrap();
    path
}

fn write_track_support_gltf(path: &Path, channels_per_animation: &[usize]) {
    let animations = channels_per_animation
        .iter()
        .enumerate()
        .map(|(index, &channel_count)| {
            let channels = (0..channel_count)
                .map(|_| json!({ "sampler": 0, "target": { "node": 0, "path": "translation" } }))
                .collect::<Vec<_>>();
            json!({
                "name": format!("Take {:03}", index + 1),
                "samplers": if channel_count == 0 { vec![] } else { vec![json!({ "input": 0, "output": 1 })] },
                "channels": channels,
            })
        })
        .collect::<Vec<_>>();
    fs::write(
        path,
        serde_json::to_vec(&json!({
            "asset": { "version": "2.0" },
            "buffers": [{
                "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                "byteLength": 24
            }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 24 }],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0.0], "max": [1.0] },
                { "bufferView": 0, "componentType": 5126, "count": 2, "type": "VEC3" }
            ],
            "nodes": [{ "name": "root" }],
            "animations": animations,
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }))
        .unwrap(),
    )
    .unwrap();
}

fn stabilize_collection_serialized_bytes(evidence: &mut Value) {
    for _ in 0..8 {
        let bytes = serde_json::to_vec(evidence).unwrap().len() as u64;
        if evidence["work"]["serialized_bytes"].as_u64() == Some(bytes) {
            return;
        }
        evidence["work"]["serialized_bytes"] = bytes.into();
    }
    panic!("collection serialized byte count did not converge");
}

fn rendered_dashboard_state(html: &Path, filters: &Value) -> Value {
    let script = r#"
const fs=require('fs'),html=fs.readFileSync(process.argv[1],'utf8'),filters=JSON.parse(process.argv[2]);
let data=html.match(/<script type="application\/json" id="collection-dashboard-data">([\s\S]*?)<\/script>/)[1];
if(filters.client_fixture){const d=JSON.parse(data);d.view.sources=[
  {key:'source-a',locator:'source-a.gltf',input:d.collection_output,availability:'available',loader:'ready',dependency_closure:'complete',takes:[{source_take_index:0,take_name:'A',normalized_clip_index:0,normalized_clip_name:'A',availability:'established',outcome:'with_findings',findings:1,severities:['error'],coverage_gaps:0,prediction_unavailable:0,coverage:{complete:1,partial:0,excluded:0,not_evaluated:0}}],unscoped_findings:0,unscoped_severities:[],unscoped_prediction_unavailable:0,unscoped_prediction_reasons:[]},
  {key:'source-b',locator:'source-b.gltf',input:d.collection_output,availability:'available',loader:'unavailable',dependency_closure:'unavailable',takes:[{source_take_index:1,take_name:'B',normalized_clip_index:1,normalized_clip_name:'B',availability:'established',outcome:'partial',findings:0,severities:[],coverage_gaps:1,prediction_unavailable:0,coverage:{complete:0,partial:1,excluded:0,not_evaluated:0}}],unscoped_findings:0,unscoped_severities:[],unscoped_prediction_unavailable:0,unscoped_prediction_reasons:[]},
  {key:'source-c',locator:'source-c.gltf',availability:'unavailable',loader:'unavailable',dependency_closure:'unavailable',takes:[],unscoped_findings:0,unscoped_severities:[],unscoped_prediction_unavailable:1,unscoped_prediction_reasons:['runtime_animation_survival_unavailable']}
];d.view.clips=[
  {id:'clip-a',source:'source-a',take_index:0,take_name:'A',roles:['root'],availability:'established',outcome:'with_findings',findings:1,severities:['error'],coverage_gaps:0,prediction_unavailable:0,coverage:{complete:1,partial:0,excluded:0,not_evaluated:0},runtime_sets:['set-a']},
  {id:'clip-b',source:'source-b',take_index:1,take_name:'B',roles:['hips'],availability:'loader_unavailable',outcome:'partial',findings:0,severities:['warning'],coverage_gaps:1,prediction_unavailable:0,coverage:{complete:0,partial:1,excluded:0,not_evaluated:0},runtime_sets:['set-b']},
  {id:'clip-c',source:'source-c',take_index:2,take_name:'C',roles:[],availability:'source_unavailable',outcome:'unavailable',findings:0,severities:[],coverage_gaps:0,prediction_unavailable:0,coverage:{complete:0,partial:0,excluded:0,not_evaluated:1},runtime_sets:[]}
];d.view.runtime_sets=[{id:'set-a',lifecycle:'complete',members:['clip-a'],gaps:[]},{id:'set-b',lifecycle:'incomplete',members:['clip-b'],gaps:['member_unavailable']}];d.summary={sources:3,physical_takes:2,clips:3,runtime_sets:2,findings:1,unscoped_findings:0,coverage_gaps:1,prediction_unavailable:1,unscoped_prediction_unavailable:1,with_findings:1,evaluated:0,partial:1,excluded:0,unavailable:1,not_evaluated:0};data=JSON.stringify(d)}
if(filters.hostile){const d=JSON.parse(data),c=d.view.clips[0],s=d.view.sources[0];c.id='</td><img src=x>';c.source='<source>';c.take_name='"quoted"';c.report_link='reports/a&b.html';s.key='</td><img src=source>';s.locator='<source-path>';data=JSON.stringify(d)}
const code=html.match(/<script>\s*([\s\S]*?)<\/script><\/body>/)[1];
const elements=new Map();
function element(id){if(!elements.has(id)){elements.set(id,{id,value:id==='group'?'source':'',children:[],append(value){this.children.push(value)}})}return elements.get(id)}
global.document={getElementById:id=>id==='collection-dashboard-data'?{textContent:data}:element(id),createElement:()=>({})};
new Function(code)();
const initialSummary=element('summary').textContent;
for(const [id,value] of Object.entries(filters)){if(!['client_fixture','hostile'].includes(id))element(id).value=value}
element('group').onchange();
console.log(JSON.stringify({count:element('count').textContent,groups:element('groups').textContent,summary:element('summary').textContent,initialSummary,roles:element('role').children.map(x=>x.value),clips:element('clips').innerHTML,sources:element('sources').innerHTML,takes:element('takes').innerHTML,sourceCount:element('source-count').textContent}));
"#;
    let output = Command::new("node")
        .args(["-e", script, html.to_str().unwrap(), &filters.to_string()])
        .output()
        .expect("node is required for the dashboard renderer behavior test");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn dashboard_binds_current_evidence_and_keeps_duplicate_takes_and_order() {
    let evidence = collection(&spike_path("collection.toml"));
    assert_eq!(evidence.status.code(), Some(0));
    let temp = tempfile::tempdir().unwrap();
    let collection_path = temp.path().join("collection-output.json");
    let html = temp.path().join("dashboard.html");
    let authority = temp.path().join("dashboard.json");
    let html_second = temp.path().join("dashboard-second.html");
    let authority_second = temp.path().join("dashboard-second.json");
    fs::write(&collection_path, &evidence.stdout).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            html.to_str().unwrap(),
            "--authority",
            authority.to_str().unwrap(),
            "--asset-report",
            "com.example.collection-spike/locomotion/walk-a=reports/walk-a.html",
        ])
        .output()
        .expect("dashboard runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("4 finding(s)"));
    let repeat = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            html_second.to_str().unwrap(),
            "--authority",
            authority_second.to_str().unwrap(),
            "--asset-report",
            "com.example.collection-spike/locomotion/walk-a=reports/walk-a.html",
        ])
        .output()
        .expect("repeat dashboard runs");
    assert_eq!(repeat.status.code(), Some(0));
    assert_eq!(
        fs::read(&authority).unwrap(),
        fs::read(&authority_second).unwrap()
    );
    assert_eq!(fs::read(&html).unwrap(), fs::read(&html_second).unwrap());
    let value: Value = serde_json::from_slice(&fs::read(&authority).unwrap()).unwrap();
    assert_eq!(
        value["schema"],
        "urn:animsmith:schema:collection-dashboard:1"
    );
    let schema: Value = serde_json::from_str(DASHBOARD_SCHEMA).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors = validator.iter_errors(&value).collect::<Vec<_>>();
    assert!(errors.is_empty(), "dashboard schema errors: {errors:?}");
    assert_eq!(
        value["collection_output"],
        serde_json::to_value(InputIdentity::from_bytes(&evidence.stdout)).unwrap()
    );
    let clips = value["view"]["clips"].as_array().unwrap();
    assert_eq!(clips.len(), 4);
    assert_ne!(
        clips[0]["id"], clips[1]["id"],
        "duplicate take names retain logical identity"
    );
    assert_eq!(clips[0]["take_name"], "Take 001");
    assert_eq!(clips[1]["take_name"], "Take 001");
    assert_eq!(clips[0]["report_link"], "reports/walk-a.html");
    let current: Value = serde_json::from_slice(&evidence.stdout).unwrap();
    assert_eq!(
        value["view"]["sources"][0]["input"],
        current["sources"][0]["input"]["input"]
    );
    assert_eq!(
        value["view"]["runtime_sets"][0]["members"][0],
        clips[0]["id"]
    );
    assert_eq!(
        value["view"]["runtime_sets"][0]["members"][1],
        clips[1]["id"]
    );
    let rendered = fs::read_to_string(&html).unwrap();
    assert!(rendered.contains("filters do not change collection completeness"));
    assert!(rendered.contains("id=\"group\""));
    assert!(rendered.contains("id=\"sources\""));
    assert!(rendered.contains("collection-dashboard-data"));
    for forbidden in ["https://", "http://", "<script src=", "<link "] {
        assert!(
            !rendered.to_ascii_lowercase().contains(forbidden),
            "{forbidden}"
        );
    }
    let state = rendered_dashboard_state(
        &html,
        &serde_json::json!({"source":"walk-a", "group":"runtime_sets"}),
    );
    assert_eq!(
        state["count"],
        "showing 1 of 4 declared clips; filters do not change collection completeness"
    );
    assert_eq!(
        state["groups"],
        "com.example.collection-spike/sets/cross-file-gait: 1 · com.example.collection-spike/sets/cross-file-sync: 1"
    );
    assert_eq!(state["roles"], serde_json::json!(["none"]));

    let unfiltered = rendered_dashboard_state(
        &html,
        &serde_json::json!({"client_fixture":true, "group":"source"}),
    );
    assert_eq!(
        unfiltered["count"],
        "showing 3 of 3 declared clips; filters do not change collection completeness"
    );
    assert_eq!(unfiltered["summary"], unfiltered["initialSummary"]);
    assert!(unfiltered["summary"].as_str().unwrap().contains(
        "3 sources · 2 physical takes · 3 clips · 2 runtime sets · 1 findings (0 unscoped)"
    ));
    assert!(
        unfiltered["summary"]
            .as_str()
            .unwrap()
            .contains("1 prediction unavailable (1 unscoped)")
    );
    let source_table = unfiltered["sources"].as_str().unwrap();
    assert!(source_table.contains("1 (runtime_animation_survival_unavailable)"));
    for (filter, value) in [
        ("source", "source-a"),
        ("role", "root"),
        ("set", "set-a"),
        ("severity", "error"),
        ("outcome", "with_findings"),
        ("availability", "established"),
    ] {
        let mut filters = serde_json::json!({"client_fixture":true});
        filters[filter] = Value::String(value.to_owned());
        let filtered = rendered_dashboard_state(&html, &filters);
        assert_eq!(
            filtered["count"],
            "showing 1 of 3 declared clips; filters do not change collection completeness",
            "filter {filter}"
        );
        assert_eq!(
            filtered["summary"], unfiltered["summary"],
            "filter {filter}"
        );
        let rows = filtered["clips"].as_str().unwrap();
        assert!(rows.contains("clip-a"), "filter {filter}");
        assert!(!rows.contains("clip-b"), "filter {filter}");
        assert!(!rows.contains("clip-c"), "filter {filter}");
    }
    for (group, expected) in [
        ("source", "source-a: 1 · source-b: 1 · source-c: 1"),
        ("roles", "hips: 1 · none: 1 · root: 1"),
        ("runtime_sets", "none: 1 · set-a: 1 · set-b: 1"),
        ("severities", "error: 1 · none: 1 · warning: 1"),
        ("outcome", "partial: 1 · unavailable: 1 · with_findings: 1"),
        (
            "availability",
            "established: 1 · loader_unavailable: 1 · source_unavailable: 1",
        ),
    ] {
        let grouped = rendered_dashboard_state(
            &html,
            &serde_json::json!({"client_fixture":true, "group":group}),
        );
        assert_eq!(grouped["groups"], expected, "group {group}");
        assert_eq!(grouped["count"], unfiltered["count"], "group {group}");
    }
    let explicit_none = rendered_dashboard_state(
        &html,
        &serde_json::json!({"client_fixture":true, "role":"none"}),
    );
    assert_eq!(
        explicit_none["roles"],
        serde_json::json!(["hips", "none", "root"])
    );
    assert_eq!(
        explicit_none["count"],
        "showing 1 of 3 declared clips; filters do not change collection completeness"
    );
    let no_match = rendered_dashboard_state(
        &html,
        &serde_json::json!({"client_fixture":true, "source":"not-declared"}),
    );
    assert_eq!(
        no_match["count"],
        "showing 0 of 3 declared clips; filters do not change collection completeness"
    );
    assert_eq!(no_match["groups"], "no matching declared clips");
    assert_eq!(no_match["summary"], unfiltered["summary"]);

    let hostile = rendered_dashboard_state(&html, &serde_json::json!({"hostile":true}));
    let table = hostile["clips"].as_str().unwrap();
    assert!(table.contains("&lt;/td&gt;&lt;img src=x&gt;"));
    assert!(table.contains("href=\"reports/a&amp;b.html\""));
    assert!(!table.contains("<img src=x>"));
    let source_table = hostile["sources"].as_str().unwrap();
    assert!(source_table.contains("&lt;/td&gt;&lt;img src=source&gt;"));
    assert!(source_table.contains("&lt;source-path&gt;"));
    assert!(!source_table.contains("<img src=source>"));
}

#[test]
fn dashboard_refuses_an_input_output_alias_before_staging() {
    let evidence = collection(&spike_path("collection.toml"));
    assert_eq!(evidence.status.code(), Some(0));
    let temp = tempfile::tempdir().unwrap();
    let collection_path = temp.path().join("collection-output.json");
    let authority = temp.path().join("dashboard.json");
    fs::write(&collection_path, &evidence.stdout).unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            collection_path.to_str().unwrap(),
            "--authority",
            authority.to_str().unwrap(),
        ])
        .output()
        .expect("dashboard runs");
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(fs::read(&collection_path).unwrap(), evidence.stdout);
    assert!(!authority.exists());
}

#[test]
fn dashboard_accepts_only_exact_manifest_bound_transition_authority() {
    let evidence = collection(&spike_path("collection.toml"));
    assert_eq!(evidence.status.code(), Some(0));
    let temp = tempfile::tempdir().unwrap();
    let collection_path = temp.path().join("collection-output.json");
    let html = temp.path().join("dashboard.html");
    let authority = temp.path().join("dashboard.json");
    let evaluation = temp.path().join("evaluation.json");
    fs::write(&collection_path, &evidence.stdout).unwrap();
    let collection: Value = serde_json::from_slice(&evidence.stdout).unwrap();
    let identity = collection["manifest"]["input"].clone();
    let source_identity = |key: &str| {
        collection["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["key"] == key)
            .unwrap()["input"]["input"]
            .clone()
    };
    let exact = serde_json::json!({
        "schema": "urn:animsmith:schema:transition-pose-evaluation:1", "schema_version": 1,
        "status": "incomplete", "decision": "not_evaluated",
        "declaration_input": identity, "declaration_normalized": collection["manifest"]["input"].clone(),
        "subject_input": collection["manifest"]["input"].clone(), "families": [{
            "family_id": "com.example.dashboard/unmatched", "status": "incomplete", "decision": "not_evaluated",
            "reason": "member_unavailable", "members": [
                {"take_index": 0, "take_name": "Take 001", "source_input": source_identity("walk-a")},
                {"take_index": 0, "take_name": "Take 001", "source_input": source_identity("multi")}
            ], "pairs": []
        }]
    });
    fs::write(&evaluation, serde_json::to_vec(&exact).unwrap()).unwrap();
    let accepted = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            html.to_str().unwrap(),
            "--authority",
            authority.to_str().unwrap(),
            "--evaluation",
            evaluation.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        accepted.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let dashboard: Value = serde_json::from_slice(&fs::read(&authority).unwrap()).unwrap();
    let schema: Value = serde_json::from_str(DASHBOARD_SCHEMA).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors = validator.iter_errors(&dashboard).collect::<Vec<_>>();
    assert!(errors.is_empty(), "dashboard schema errors: {errors:?}");
    let family = &dashboard["evaluation"]["families"][0];
    assert_eq!(family["reason"], "member_unavailable");
    assert!(family["members"][0].get("logical_clip").is_none());
    assert_eq!(
        family["members"][1]["logical_clip"],
        "com.example.collection-spike/multi/first"
    );
    let mut wrong = exact;
    wrong["subject_input"]["bytes"] = Value::from(1_u64);
    fs::write(&evaluation, serde_json::to_vec(&wrong).unwrap()).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            temp.path().join("rejected.html").to_str().unwrap(),
            "--authority",
            temp.path().join("rejected.json").to_str().unwrap(),
            "--evaluation",
            evaluation.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
}

#[test]
fn dashboard_transition_reader_requires_canonical_complete_pair_coverage() {
    fn pair(left: u64, right: u64, boundary: &str) -> Value {
        serde_json::json!({
            "member_indices":[left,right], "boundary":boundary,
            "max_translation_delta_m":0.0, "max_rotation_delta_deg":0.0,
            "translation_tolerance_m":0.01, "rotation_tolerance_deg":1.0,
            "translation_offenders":[], "rotation_offenders":[]
        })
    }

    let evidence = collection(&spike_path("collection.toml"));
    assert_eq!(evidence.status.code(), Some(0));
    let temp = tempfile::tempdir().unwrap();
    let collection_path = temp.path().join("collection-output.json");
    fs::write(&collection_path, &evidence.stdout).unwrap();
    let collection: Value = serde_json::from_slice(&evidence.stdout).unwrap();
    let identity = collection["manifest"]["input"].clone();
    let sources = collection["sources"].as_array().unwrap();
    let source_identity = |key: &str| {
        sources.iter().find(|source| source["key"] == key).unwrap()["input"]["input"].clone()
    };
    let members = [
        (0_u64, "Take 001", source_identity("walk-a")),
        (0_u64, "Take 001", source_identity("walk-b")),
        (0_u64, "Take 001", source_identity("multi")),
    ]
    .into_iter()
    .map(|(take_index, take_name, input)| {
        let closure = input.clone();
        serde_json::json!({
            "take_index":take_index, "take_name":take_name,
            "source_input":input, "source_dependency_closure_identity":closure
        })
    })
    .collect::<Vec<_>>();
    let canonical_pairs = vec![
        pair(0, 1, "entry"),
        pair(0, 1, "exit"),
        pair(0, 2, "entry"),
        pair(0, 2, "exit"),
        pair(1, 2, "entry"),
        pair(1, 2, "exit"),
    ];
    let canonical = serde_json::json!({
        "schema":"urn:animsmith:schema:transition-pose-evaluation:1", "schema_version":1,
        "status":"complete", "decision":"pass",
        "declaration_input":identity, "declaration_normalized":collection["manifest"]["input"].clone(),
        "subject_input":collection["manifest"]["input"].clone(),
        "families":[{
            "family_id":"com.example.collection-spike/transition/canonical",
            "status":"complete", "decision":"pass", "members":members,
            "skeleton_basis_input":collection["manifest"]["input"].clone(),
            "pairs":canonical_pairs
        }]
    });
    let evaluation_path = temp.path().join("canonical.json");
    fs::write(&evaluation_path, serde_json::to_vec(&canonical).unwrap()).unwrap();
    let accepted = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            temp.path().join("canonical.html").to_str().unwrap(),
            "--authority",
            temp.path()
                .join("canonical-authority.json")
                .to_str()
                .unwrap(),
            "--evaluation",
            evaluation_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        accepted.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );

    let mut mutations = Vec::new();
    let mut self_pair = canonical.clone();
    self_pair["families"][0]["pairs"][0]["member_indices"] = serde_json::json!([0, 0]);
    mutations.push(("self-pair", self_pair));
    let mut reversed = canonical.clone();
    reversed["families"][0]["pairs"][0]["member_indices"] = serde_json::json!([1, 0]);
    mutations.push(("reversed-pair", reversed));
    let mut duplicate = canonical.clone();
    let repeated_pair = duplicate["families"][0]["pairs"][0].clone();
    duplicate["families"][0]["pairs"][2] = repeated_pair;
    mutations.push(("duplicate-pair", duplicate));
    let mut omitted = canonical.clone();
    omitted["families"][0]["pairs"]
        .as_array_mut()
        .unwrap()
        .pop();
    mutations.push(("omitted-pair", omitted));
    let mut pair_order = canonical.clone();
    pair_order["families"][0]["pairs"]
        .as_array_mut()
        .unwrap()
        .swap(2, 4);
    mutations.push(("pair-order", pair_order));
    let mut boundary_order = canonical;
    boundary_order["families"][0]["pairs"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    mutations.push(("boundary-order", boundary_order));

    for (name, mutation) in mutations {
        let evaluation_path = temp.path().join(format!("{name}.json"));
        let html = temp.path().join(format!("{name}.html"));
        let authority = temp.path().join(format!("{name}-authority.json"));
        fs::write(&evaluation_path, serde_json::to_vec(&mutation).unwrap()).unwrap();
        let rejected = Command::new(env!("CARGO_BIN_EXE_animsmith"))
            .args([
                "collection",
                "dashboard",
                "--collection",
                collection_path.to_str().unwrap(),
                "--output",
                html.to_str().unwrap(),
                "--authority",
                authority.to_str().unwrap(),
                "--evaluation",
                evaluation_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(
            rejected.status.code(),
            Some(2),
            "mutation {name} unexpectedly accepted: {}",
            String::from_utf8_lossy(&rejected.stderr)
        );
        assert!(!html.exists(), "mutation {name} staged HTML");
        assert!(!authority.exists(), "mutation {name} staged authority");
    }
}

#[test]
fn dashboard_keeps_current_unavailable_collection_rows_visible() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::create_dir(temp.path().join("source")).unwrap();
    fs::copy(
        spike_path("source/walk-a.gltf"),
        temp.path().join("source/present.gltf"),
    )
    .unwrap();
    fs::copy(
        spike_path("source/walk-a.gltf"),
        temp.path().join("source/unbound.gltf"),
    )
    .unwrap();
    fs::write(
        &manifest,
        "schema = \"urn:animsmith:schema:collection-manifest:1\"\nschema_version = 1\ncollection_id = \"com.example.dashboard-unavailable\"\ninput_root = \"source\"\n\n[[sources]]\nkey = \"missing\"\npath = \"missing.gltf\"\n\n[[sources]]\nkey = \"unbound\"\npath = \"unbound.gltf\"\n\n[[sources]]\nkey = \"present\"\npath = \"present.gltf\"\n\n[[clips]]\nid = \"com.example.dashboard-unavailable/present\"\nsource = \"present\"\ntake_index = 0\ntake_name = \"Take 001\"\n",
    )
    .unwrap();
    let evidence = collection(&manifest);
    assert_eq!(
        evidence.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&evidence.stderr)
    );
    let current: Value = serde_json::from_slice(&evidence.stdout).unwrap();
    assert_eq!(
        current["schema"],
        "urn:animsmith:schema:collection-output:11"
    );
    assert_eq!(current["sources"][0]["key"], "missing");
    assert_eq!(current["clips"].as_array().unwrap().len(), 1);
    let unbound_current = current["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["key"] == "unbound")
        .unwrap();
    assert_eq!(
        unbound_current["observed_takes"].as_array().unwrap().len(),
        1
    );
    assert!(
        unbound_current["result"]["envelope"]["files"][0]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|check| check["findings"].as_array().into_iter().flatten())
            .any(|finding| finding["clip"] == "Take 001"),
        "fixture must exercise a real clip-scoped finding"
    );
    let collection_path = temp.path().join("collection-output.json");
    let html = temp.path().join("dashboard.html");
    let authority = temp.path().join("dashboard.json");
    fs::write(&collection_path, &evidence.stdout).unwrap();
    let dashboard = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            html.to_str().unwrap(),
            "--authority",
            authority.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        dashboard.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&dashboard.stderr)
    );
    let authority: Value = serde_json::from_slice(&fs::read(authority).unwrap()).unwrap();
    let missing = authority["view"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["key"] == "missing")
        .unwrap();
    assert_eq!(missing["availability"], "unavailable");
    assert!(missing.get("input").is_none());
    assert_eq!(missing["loader"], "unavailable");
    assert_eq!(missing["dependency_closure"], "unavailable");
    let unbound = authority["view"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["key"] == "unbound")
        .unwrap();
    assert_eq!(unbound["availability"], "available");
    assert_eq!(unbound["takes"].as_array().unwrap().len(), 1);
    assert_eq!(unbound["takes"][0]["source_take_index"], 0);
    assert_eq!(unbound["takes"][0]["take_name"], "Take 001");
    assert_eq!(unbound["takes"][0]["normalized_clip_index"], 0);
    assert_eq!(unbound["takes"][0]["normalized_clip_name"], "Take 001");
    assert!(unbound["takes"][0]["findings"].as_u64().unwrap() > 0);
    assert_eq!(unbound["takes"][0]["outcome"], "with_findings");
    assert!(
        authority["view"]["clips"]
            .as_array()
            .unwrap()
            .iter()
            .all(|clip| clip["source"] != "unbound"),
        "a physical take must not invent a logical clip id"
    );
    assert_eq!(authority["summary"]["sources"], 3);
    assert_eq!(authority["summary"]["physical_takes"], 2);
    assert_eq!(authority["summary"]["clips"], 1);
    let physical_findings = authority["view"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|source| source["takes"].as_array().unwrap())
        .map(|take| take["findings"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(authority["summary"]["findings"], physical_findings);
    let state = rendered_dashboard_state(&html, &serde_json::json!({}));
    assert_eq!(
        state["count"],
        "showing 1 of 1 declared clips; filters do not change collection completeness"
    );
    assert_eq!(
        state["sourceCount"],
        "3 declared sources; sources with zero logical clips remain listed"
    );
    let source_table = state["sources"].as_str().unwrap();
    assert!(source_table.contains("missing"));
    assert!(source_table.contains("missing.gltf"));
    assert!(source_table.contains("unavailable"));
    assert!(source_table.contains("<td>0</td>"));
    let take_table = state["takes"].as_str().unwrap();
    assert!(take_table.contains("unbound"));
    assert!(take_table.contains("Take 001"));
    assert!(take_table.contains("with_findings"));
}

#[test]
fn dashboard_does_not_guess_name_addressed_evidence_across_duplicate_physical_names() {
    let temp = tempfile::tempdir().unwrap();
    let evidence = collection(&spike_path("collection.toml"));
    assert_eq!(evidence.status.code(), Some(0));
    let mut current: Value = serde_json::from_slice(&evidence.stdout).unwrap();
    let source = current["sources"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|source| source["key"] == "multi")
        .unwrap();
    let duplicate_name = source["observed_takes"][0]["normalized"]["name"]
        .as_str()
        .unwrap()
        .to_owned();
    source["observed_takes"][1]["normalized"]["name"] = duplicate_name.clone().into();
    for clip in current["clips"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|clip| clip["source"] == "multi")
    {
        clip["binding"]["check_reference"] = serde_json::json!({
            "state": "unavailable",
            "reason": "duplicate_embedded_take_name"
        });
    }
    let multi = current["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["key"] == "multi")
        .unwrap();
    assert!(
        multi["observed_takes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|take| take["normalized"]["name"] == duplicate_name),
        "fixture must be a strict V11 same-source normalized-name collision"
    );
    let nested_findings = multi["result"]["envelope"]["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|check| check["findings"].as_array().unwrap().len())
        .sum::<usize>();
    assert!(
        nested_findings > 0,
        "fixture must carry name-addressed evidence"
    );
    assert!(
        multi["result"]["envelope"]["files"][0]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|check| check["findings"].as_array().into_iter().flatten())
            .any(|finding| finding["clip"] == duplicate_name),
        "fixture must contain evidence addressed by the ambiguous name"
    );
    stabilize_collection_serialized_bytes(&mut current);

    let collection_path = temp.path().join("collection-output.json");
    let html = temp.path().join("dashboard.html");
    let authority_path = temp.path().join("dashboard.json");
    fs::write(&collection_path, serde_json::to_vec(&current).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            html.to_str().unwrap(),
            "--authority",
            authority_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let authority: Value = serde_json::from_slice(&fs::read(&authority_path).unwrap()).unwrap();
    let schema: Value = serde_json::from_str(DASHBOARD_SCHEMA).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors = validator.iter_errors(&authority).collect::<Vec<_>>();
    assert!(errors.is_empty(), "dashboard schema errors: {errors:?}");
    let source = authority["view"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["key"] == "multi")
        .unwrap();
    let takes = source["takes"].as_array().unwrap();
    assert_eq!(takes.len(), 2);
    for take in takes {
        assert_eq!(take["availability"], "duplicate_normalized_clip_name");
        assert_eq!(take["outcome"], "unavailable");
        assert_eq!(take["findings"], 0);
        assert_eq!(take["coverage_gaps"], 0);
        assert_eq!(take["prediction_unavailable"], 0);
    }
    for clip in authority["view"]["clips"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|clip| clip["source"] == "multi")
    {
        assert_eq!(clip["availability"], "duplicate_embedded_take_name");
        assert_eq!(clip["outcome"], "unavailable");
        assert_eq!(clip["findings"], 0);
    }
    assert_eq!(
        source["unscoped_findings"].as_u64(),
        Some(nested_findings as u64)
    );
    let physical_findings = authority["view"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|source| source["takes"].as_array().unwrap())
        .map(|take| take["findings"].as_u64().unwrap())
        .sum::<u64>();
    let unscoped_findings = authority["view"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| source["unscoped_findings"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(
        authority["summary"]["findings"].as_u64(),
        Some(physical_findings + unscoped_findings),
        "each nested finding is retained once, not copied to either take or clip"
    );
    let state = rendered_dashboard_state(&html, &serde_json::json!({}));
    assert!(
        state["takes"]
            .as_str()
            .unwrap()
            .contains("duplicate_normalized_clip_name")
    );
}

#[test]
fn dashboard_retains_real_unscoped_required_bones_findings_at_source_level() {
    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("source");
    fs::create_dir(&source_dir).unwrap();
    fs::copy(
        spike_path("source/walk-a.gltf"),
        source_dir.join("rig.gltf"),
    )
    .unwrap();
    fs::write(
        temp.path().join("required.animsmith.toml"),
        "[rig]\nrequired_bones = [\"missing_socket\"]\n",
    )
    .unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        "schema = \"urn:animsmith:schema:collection-manifest:1\"\nschema_version = 1\ncollection_id = \"com.example.dashboard-unscoped\"\ninput_root = \"source\"\n\n[[sources]]\nkey = \"rig\"\npath = \"rig.gltf\"\nconfig = \"required.animsmith.toml\"\n\n[[clips]]\nid = \"com.example.dashboard-unscoped/take\"\nsource = \"rig\"\ntake_index = 0\ntake_name = \"Take 001\"\n",
    )
    .unwrap();
    let evidence = collection(&manifest);
    assert_eq!(
        evidence.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&evidence.stderr)
    );
    let current: Value = serde_json::from_slice(&evidence.stdout).unwrap();
    let required_bones = current["sources"][0]["result"]["envelope"]["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["check_id"] == "required-bones")
        .expect("required-bones check is present and enabled");
    assert_eq!(required_bones["evaluation"], "complete");
    assert_eq!(required_bones["findings"].as_array().unwrap().len(), 1);
    assert!(required_bones["findings"][0].get("clip").is_none());

    let collection_path = temp.path().join("collection-output.json");
    let html = temp.path().join("dashboard.html");
    let authority_path = temp.path().join("dashboard.json");
    fs::write(&collection_path, &evidence.stdout).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            html.to_str().unwrap(),
            "--authority",
            authority_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let authority: Value = serde_json::from_slice(&fs::read(&authority_path).unwrap()).unwrap();
    let source = &authority["view"]["sources"][0];
    assert_eq!(source["key"], "rig");
    assert_eq!(source["unscoped_findings"], 1);
    assert_eq!(source["unscoped_severities"], serde_json::json!(["error"]));
    assert_eq!(authority["summary"]["unscoped_findings"], 1);
    let clip_findings = authority["view"]["clips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|clip| clip["findings"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(
        authority["summary"]["findings"].as_u64().unwrap(),
        clip_findings + 1
    );
    let state = rendered_dashboard_state(&html, &serde_json::json!({}));
    assert!(
        state["summary"]
            .as_str()
            .unwrap()
            .contains("findings (1 unscoped)")
    );
    assert!(state["sources"].as_str().unwrap().contains("1 (error)"));
}

#[test]
fn dashboard_retains_real_unmapped_prediction_unavailable_at_source_level() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("track.gltf");
    write_track_support_gltf(&source, &[1]);
    let config = write_bevy_v3_track_config(temp.path(), "track-support", true, true);
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        format!(
            "schema = \"urn:animsmith:schema:collection-manifest:1\"\nschema_version = 1\ncollection_id = \"com.example.dashboard-unscoped-prediction\"\n\n[[sources]]\nkey = \"track\"\npath = \"{}\"\nconfig = \"{}\"\n\n[[clips]]\nid = \"com.example.dashboard-unscoped-prediction/track\"\nsource = \"track\"\ntake_index = 0\ntake_name = \"Take 001\"\n",
            source.file_name().unwrap().to_str().unwrap(),
            config.file_name().unwrap().to_str().unwrap(),
        ),
    )
    .unwrap();
    let evidence = collection(&manifest);
    assert_eq!(
        evidence.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&evidence.stderr)
    );
    let current: Value = serde_json::from_slice(&evidence.stdout).unwrap();
    let track_support = current["sources"][0]["result"]["envelope"]["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["check_id"] == "engine-track-support")
        .expect("track-support check is present and enabled");
    assert_eq!(track_support["evaluation"], "not_evaluated");
    assert_eq!(track_support["findings"], json!([]));
    let facets = track_support["prediction"]["prediction"]["facets"]
        .as_array()
        .expect("track-support prediction facets");
    assert!(!facets.is_empty());
    for facet in facets {
        assert_eq!(facet["state"], "required_prediction_unavailable");
        assert_eq!(
            facet["reasons"],
            json!(["runtime_animation_survival_unavailable"])
        );
        assert!(
            facet["scope"]["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("engine-track-support:animation")),
            "{facet:#}"
        );
        assert!(
            facet["scope"]["subject"]
                .as_str()
                .is_some_and(|subject| subject.starts_with("source_animation:0")),
            "{facet:#}"
        );
    }

    let collection_path = temp.path().join("collection-output.json");
    let html = temp.path().join("dashboard.html");
    let authority_path = temp.path().join("dashboard.json");
    fs::write(&collection_path, &evidence.stdout).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            html.to_str().unwrap(),
            "--authority",
            authority_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let authority: Value = serde_json::from_slice(&fs::read(&authority_path).unwrap()).unwrap();
    let schema: Value = serde_json::from_str(DASHBOARD_SCHEMA).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let errors = validator.iter_errors(&authority).collect::<Vec<_>>();
    assert!(errors.is_empty(), "dashboard schema errors: {errors:?}");
    assert_eq!(authority["summary"]["clips"], 1);
    assert_eq!(
        authority["summary"]["prediction_unavailable"].as_u64(),
        Some(facets.len() as u64)
    );
    assert_eq!(
        authority["summary"]["unscoped_prediction_unavailable"].as_u64(),
        Some(facets.len() as u64)
    );
    let source = &authority["view"]["sources"][0];
    assert_eq!(source["key"], "track");
    assert_eq!(
        source["unscoped_prediction_unavailable"].as_u64(),
        Some(facets.len() as u64)
    );
    assert_eq!(
        source["unscoped_prediction_reasons"],
        json!(["runtime_animation_survival_unavailable"])
    );
    let takes = source["takes"].as_array().unwrap();
    assert_eq!(takes.len(), 1);
    assert_eq!(takes[0]["prediction_unavailable"], 0);
    assert_eq!(authority["view"]["clips"].as_array().unwrap().len(), 1);
    assert_eq!(authority["view"]["clips"][0]["prediction_unavailable"], 0);
    let state = rendered_dashboard_state(&html, &json!({}));
    assert_eq!(
        state["count"],
        "showing 1 of 1 declared clips; filters do not change collection completeness"
    );
    assert!(state["summary"].as_str().unwrap().contains(&format!(
        "{} prediction unavailable ({} unscoped)",
        facets.len(),
        facets.len()
    )));
    assert!(state["sources"].as_str().unwrap().contains(&format!(
        "{} (runtime_animation_survival_unavailable)",
        facets.len()
    )));
}

#[test]
fn dashboard_counts_complete_and_excluded_coverage_without_scope_lists() {
    let evidence = collection(&spike_path("collection.toml"));
    assert_eq!(evidence.status.code(), Some(0));
    let temp = tempfile::tempdir().unwrap();
    let collection_path = temp.path().join("collection-output.json");
    let html = temp.path().join("dashboard.html");
    let authority = temp.path().join("dashboard.json");
    let current: Value = serde_json::from_slice(&evidence.stdout).unwrap();
    fs::write(&collection_path, serde_json::to_vec(&current).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            html.to_str().unwrap(),
            "--authority",
            authority.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let authority: Value = serde_json::from_slice(&fs::read(authority).unwrap()).unwrap();
    let clips = authority["view"]["clips"].as_array().unwrap();
    assert!(
        current["sources"][0]["result"]["envelope"]["files"][0]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["evaluation"] == "complete"
                && check.get("evaluated_scopes").is_none())
    );
    for clip in clips {
        assert!(clip["coverage"]["complete"].as_u64().unwrap() > 0);
        assert!(clip["coverage"]["excluded"].as_u64().unwrap() > 0);
    }
    assert_eq!(
        authority["summary"]["with_findings"],
        clips
            .iter()
            .filter(|clip| clip["findings"].as_u64().unwrap() > 0)
            .count()
    );
    let partition = [
        "with_findings",
        "evaluated",
        "partial",
        "excluded",
        "unavailable",
        "not_evaluated",
    ]
    .into_iter()
    .map(|field| authority["summary"][field].as_u64().unwrap())
    .sum::<u64>();
    assert_eq!(partition, clips.len() as u64);
}

#[test]
fn dashboard_rejects_a_current_reader_summary_mutation() {
    let evidence = collection(&spike_path("collection.toml"));
    assert_eq!(evidence.status.code(), Some(0));
    let temp = tempfile::tempdir().unwrap();
    let collection_path = temp.path().join("collection-output.json");
    let html = temp.path().join("dashboard.html");
    let authority = temp.path().join("dashboard.json");
    let mut current: Value = serde_json::from_slice(&evidence.stdout).unwrap();
    current["summary"]["established_clips"] = Value::from(0_u64);
    fs::write(&collection_path, serde_json::to_vec(&current).unwrap()).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            collection_path.to_str().unwrap(),
            "--output",
            html.to_str().unwrap(),
            "--authority",
            authority.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
    assert!(!html.exists());
    assert!(!authority.exists());
}

#[test]
fn dashboard_refuses_noncurrent_evidence_and_unsafe_report_links() {
    let temp = tempfile::tempdir().unwrap();
    let old = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.agents/skills/evaluate-animation-packs/fixtures/collection-output-v11-complete.json");
    let html = temp.path().join("dashboard.html");
    let authority = temp.path().join("dashboard.json");
    let unsafe_link = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            old.to_str().unwrap(),
            "--output",
            html.to_str().unwrap(),
            "--authority",
            authority.to_str().unwrap(),
            "--asset-report",
            "com.example.evaluation-v2/walk-a=../escape.html",
        ])
        .output()
        .expect("dashboard runs");
    assert_eq!(unsafe_link.status.code(), Some(2));
    assert!(!html.exists());
    assert!(!authority.exists());

    let mut historical: Value =
        serde_json::from_slice(&collection(&spike_path("collection.toml")).stdout).unwrap();
    historical["schema"] = Value::String("urn:animsmith:schema:collection-output:10".into());
    historical["schema_version"] = Value::from(10);
    let historical_path = temp.path().join("old.json");
    fs::write(&historical_path, serde_json::to_vec(&historical).unwrap()).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "dashboard",
            "--collection",
            historical_path.to_str().unwrap(),
            "--output",
            html.to_str().unwrap(),
            "--authority",
            authority.to_str().unwrap(),
        ])
        .output()
        .expect("dashboard runs");
    assert_eq!(rejected.status.code(), Some(2));
}
