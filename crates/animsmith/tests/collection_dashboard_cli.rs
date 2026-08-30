#![cfg(feature = "report")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use animsmith_core::InputIdentity;
use serde_json::Value;

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

fn rendered_dashboard_state(html: &Path, filters: &Value) -> Value {
    let script = r#"
const fs=require('fs'),html=fs.readFileSync(process.argv[1],'utf8'),filters=JSON.parse(process.argv[2]);
let data=html.match(/<script type="application\/json" id="collection-dashboard-data">([\s\S]*?)<\/script>/)[1];
if(filters.hostile){const d=JSON.parse(data),c=d.view.clips[0];c.id='</td><img src=x>';c.source='<source>';c.take_name='"quoted"';c.report_link='reports/a&b.html';data=JSON.stringify(d)}
const code=html.match(/<script>\s*([\s\S]*?)<\/script><\/body>/)[1];
const elements=new Map();
function element(id){if(!elements.has(id)){elements.set(id,{id,value:id==='group'?'source':'',children:[],append(value){this.children.push(value)}})}return elements.get(id)}
global.document={getElementById:id=>id==='collection-dashboard-data'?{textContent:data}:element(id),createElement:()=>({})};
new Function(code)();
for(const [id,value] of Object.entries(filters)){element(id).value=value}
element('group').onchange();
console.log(JSON.stringify({count:element('count').textContent,groups:element('groups').textContent,roles:element('role').children.map(x=>x.value),clips:element('clips').innerHTML}));
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
    assert!(rendered.contains("Object.entries(counts)"));
    assert!(rendered.contains("collection-dashboard-data"));
    assert!(rendered.contains("facet=(item,key)"));
    assert!(!rendered.contains("localeCompare"));
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
    let hostile = rendered_dashboard_state(&html, &serde_json::json!({"hostile":true}));
    let table = hostile["clips"].as_str().unwrap();
    assert!(table.contains("&lt;/td&gt;&lt;img src=x&gt;"));
    assert!(table.contains("href=\"reports/a&amp;b.html\""));
    assert!(!table.contains("<img src=x>"));
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
fn dashboard_keeps_current_unavailable_collection_rows_visible() {
    let temp = tempfile::tempdir().unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::create_dir(temp.path().join("source")).unwrap();
    fs::write(
        &manifest,
        "schema = \"urn:animsmith:schema:collection-manifest:1\"\nschema_version = 1\ncollection_id = \"com.example.dashboard-unavailable\"\ninput_root = \"source\"\n\n[[sources]]\nkey = \"missing\"\npath = \"missing.gltf\"\n\n[[clips]]\nid = \"com.example.dashboard-unavailable/missing\"\nsource = \"missing\"\ntake_index = 0\ntake_name = \"Take 001\"\n",
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
    assert_eq!(current["clips"][0]["binding"]["state"], "unavailable");
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
    assert_eq!(
        authority["view"]["sources"][0]["availability"],
        "unavailable"
    );
    assert_eq!(
        authority["view"]["clips"][0]["availability"],
        "source_unavailable"
    );
    assert_eq!(authority["view"]["clips"][0]["outcome"], "unavailable");
    assert_eq!(authority["summary"]["unavailable"], 1);
    assert!(
        fs::read_to_string(html)
            .unwrap()
            .contains("source_unavailable")
    );
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
