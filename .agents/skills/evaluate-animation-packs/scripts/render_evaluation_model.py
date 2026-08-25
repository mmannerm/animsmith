#!/usr/bin/env python3
"""Render the fixed V1 evaluation report and evidence appendix from one model."""

from __future__ import annotations

import argparse
import inspect
import os
import re
import stat
import sys
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import evaluation_contract_v1 as contract
import evaluation_model_v1 as model_contract
import validate_evaluation_model as model_validator
import validate_report as report_validator


RENDERER_VERSION = "1"
READINESS_LADDER = "../game-ready-clips.md#the-readiness-ladder"


@dataclass(frozen=True)
class RenderedViews:
    """The only two fixed V1 Markdown projections."""

    report: str
    appendix: str


def _text(value: Any) -> str:
    """Escape untrusted model prose for a Markdown cell or paragraph."""
    return (
        str(value).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
        .replace("\\", "\\\\").replace("|", "\\|").replace("[", "\\[")
        .replace("]", "\\]").replace("\r", " ").replace("\n", " ")
    )


def _code(value: Any) -> str:
    return "`" + _text(value).replace("`", "\\`") + "`"


def _literal(value: Any) -> str:
    """Put one exact JSON value in a table cell without changing its meaning.

    The prose escaping helper is deliberately lossy for Markdown safety.  The
    authority ledger instead needs every scalar/array/object exactly as the
    model supplied it, so use a variable-width code fence and escape only the
    GFM table separator.  Parsed code-span text consequently equals the
    canonical JSON representation checked by :func:`validate_views`.
    """
    encoded = model_contract.canonical_json(value).decode("utf-8")
    fence = "`" * (max((len(run) for run in re.findall(r"`+", encoded)), default=0) + 1)
    return fence + encoded.replace("|", "\\|") + fence


def _literal_value(value: Any) -> str:
    """The AST text expected for a ledger cell."""
    return model_contract.canonical_json(value).decode("utf-8")


def _link(label: Any, destination: str) -> str:
    """Render a pinned-parser-safe link without changing its public destination."""
    # Validation rejects raw angle brackets in public locators.  Do not repair
    # a bad destination here: doing so would make rendered evidence differ
    # from the signed model authority.  Escape label backticks so the AST
    # preserves evidence prose literally rather than turning it into code.
    return f"[{_text(label).replace('`', '\\`')}](<{destination}>)"


def _availability(value: dict[str, Any]) -> str:
    return str(value["value"]) if value["state"] == "available" else str(value["state"])


def _evidence(model: dict[str, Any], refs: list[str]) -> str:
    records = {record["id"]: record for record in model["evidence"]}
    return "; ".join(
        f"{_code(reference)}: {_link(records[reference]['summary'], records[reference]['locator'])}"
        for reference in refs
    ) or "No linked evidence."


def _binding_status(value: Any, fields: tuple[str, ...] = ("state", "reason")) -> dict[str, Any]:
    """Keep only logical state from an unpublishable collection projection."""
    return {
        field: value[field]
        for field in fields
        if isinstance(value, dict) and field in value and isinstance(value[field], (str, int, float, bool, type(None)))
    }


def _identity(value: Any) -> dict[str, Any]:
    """Retain a digest/size identity without a path-bearing envelope."""
    return _binding_status(value, ("sha256", "bytes"))


def _public_clip_binding(value: Any) -> dict[str, Any]:
    """Preserve clip binding state and safe logical relationships only."""
    projected = _binding_status(value)
    if not isinstance(value, dict) or value.get("state") != "established":
        return projected
    projected.update({
        key: value[key]
        for key in ("observed_source_take_index", "observed_take_name", "normalized_clip_index")
        if key in value
    })
    reference = value.get("check_reference")
    if isinstance(reference, dict):
        check = _binding_status(reference)
        if isinstance(reference.get("reference"), dict):
            check["reference"] = {
                key: reference["reference"][key]
                for key in ("source", "normalized_clip_index")
                if key in reference["reference"]
            }
        projected["check_reference"] = check
    return projected


def _public_binding_projection(binding: dict[str, Any]) -> dict[str, Any]:
    """Return the non-recoverable, renderer-safe collection binding witness.

    Collection-output bindings can legitimately contain absolute evaluator
    paths and detailed loader/configuration JSON.  They are accepted as input,
    but neither those locators nor raw subtrees belong in a public Markdown
    view.  This projection retains only logical keys, statuses, digest/input
    identities, and manifest relations needed to audit model derivation.
    """
    manifest = binding["manifest"]
    return {
        "schema": binding["schema"], "schema_version": binding["schema_version"],
        "command": binding["command"],
        "manifest": {"collection_id": manifest["collection_id"], "input": _identity(manifest["input"])},
        "summary": dict(binding["summary"]),
        "sources": [
            {
                "key": source["key"],
                "input": {**_binding_status(source.get("input"), ("state", "reason", "inspected_bytes")), **({"input": _identity(source["input"]["input"])} if isinstance(source.get("input"), dict) and isinstance(source["input"].get("input"), dict) else {})},
                "digest": _binding_status(source.get("digest"), ("state", "expected_sha256", "observed_sha256")),
                "config": {**_binding_status(source.get("config"), ("state",)), **({"input": _identity(source["config"]["input"])} if isinstance(source.get("config"), dict) and isinstance(source["config"].get("input"), dict) else {})},
                "loader": _binding_status(source.get("loader")),
                "take_inventory": source.get("take_inventory"),
                "observed_take_count": len(source.get("observed_takes", [])),
                "result": _binding_status(source.get("result")),
            }
            for source in binding["sources"]
        ],
        "clips": [
            {**{key: clip[key] for key in ("id", "source", "take_index", "take_name")}, "binding": _public_clip_binding(clip.get("binding"))}
            for clip in binding["clips"]
        ],
        "runtime_sets": [
            {
                "id": runtime_set["id"], "kind": runtime_set["kind"],
                "members": [member.get("id") for member in runtime_set["members"]],
                "lifecycle": runtime_set.get("lifecycle"), "decision": runtime_set.get("decision"),
                "gap_count": len(runtime_set.get("gaps", [])),
            }
            for runtime_set in binding["runtime_sets"]
        ],
    }


def _narrative(model: dict[str, Any], slot: str, fallback: str) -> str:
    for record in model["narratives"]:
        if record["slot"] == slot:
            return _text(record["text"])
    return fallback


def _runtime_rows(model: dict[str, Any]) -> list[str]:
    rows: list[str] = []
    for runtime_set in model["runtime_sets"]:
        members = "; ".join(_code(member["clip_id"]) for member in runtime_set["members"])
        clips = {clip["id"]: clip for clip in model["clips"]}
        durations = [clips[member["clip_id"]]["duration_s"]["value"] for member in runtime_set["members"] if clips[member["clip_id"]]["duration_s"]["state"] == "available"]
        speeds = [clips[member["clip_id"]]["root_motion_speed_mps"]["value"] for member in runtime_set["members"] if clips[member["clip_id"]]["root_motion_speed_mps"]["state"] == "available"]
        def metric(label: str, values: list[Any], total: int, unit: str) -> str:
            if not values:
                return f"{label} available=0/{total}"
            low, high = min(values), max(values)
            display = model_contract.canonical_number
            detail = f"value={display(low)}" if low == high else f"range={display(low)}..{display(high)}"
            return f"{label} available={len(values)}/{total}; {detail} {unit}"
        timing = "; ".join((
            metric("duration", durations, len(runtime_set["members"]), "s"),
            metric("rm_speed", speeds, len(runtime_set["members"]), "m/s"),
        ))
        rows.append(
            f"| {_code(runtime_set['id'])} | kind={_text(runtime_set['kind'])}; {_evidence(model, runtime_set['evidence_refs'])} | {members} | "
            f"set_type={_text(runtime_set['kind'])} | {timing} | "
            f"state={_text(runtime_set['coverage'])} |"
        )
    return rows


def _engine_rows(model: dict[str, Any]) -> list[str]:
    rows = []
    for runtime in report_validator.ENGINE_LABELS:
        record = next(record for record in model["engine_evidence"] if record["runtime"] == runtime)
        rows.append(f"| {_text(runtime)} {_text(record['version'])} | {_text(record['level'])} | {_text(record['procedure'])}; {_evidence(model, record['evidence_refs'])} | {_text(record['settings'])} |")
    return rows


def _ledger_table(title: str, columns: tuple[str, ...], records: list[dict[str, Any]]) -> str:
    """Render an explicit, readable fixed-slot projection for one authority."""
    return "\n".join([
        f"#### {title}",
        "| " + " | ".join(columns) + " |",
        "| " + " | ".join("---" for _ in columns) + " |",
        *[
            "| " + " | ".join(_literal(record.get(column)) for column in columns) + " |"
            for record in records
        ],
        "",
    ])


def _ledger_sections(model: dict[str, Any], binding: dict[str, Any]) -> list[tuple[str, tuple[str, ...], list[dict[str, Any]]]]:
    """Render every closed V1 fact family in individually labelled table cells.

    This is intentionally a human-readable projection, not a second authority:
    each row labels every persisted field and the later AST checker compares it
    exactly with the already validated in-memory model/binding.
    """
    binding = _public_binding_projection(binding)
    collection_totals = {
        "constituents": len(model["collection"]["constituents"]),
        "logical_clips": sum(len(record["clip_ids"]) for record in model["collection"]["constituents"]),
        "source_files": sum(record["source_file_count"] for record in model["collection"]["constituents"]),
        "runtime_sets": sum(len(record["runtime_set_ids"]) for record in model["collection"]["constituents"]),
        "pair_records": len(model["collection"]["cross_pack_records"]),
    }
    return [
        ("Model contract", ("schema", "schema_version", "canonical_digest"), [{"schema": model["schema"], "schema_version": model["schema_version"], "canonical_digest": model_contract.canonical_digest(model)}]),
        ("Presentation", ("id", "title", "evaluation_date", "verdict", "completeness", "confidence"), [model["presentation"]]),
        ("Evidence", ("id", "kind", "locator", "summary"), model["evidence"]),
        ("Runs", ("id", "state", "evidence_refs", "summary", "supersedes"), model["runs"]),
        ("Clips", ("id", "source", "take_index", "take_name", "primary_role", "tags", "classification_basis", "evidence_refs", "loop", "duration_s", "root_motion_speed_mps", "movement_owner", "assessment", "coverage"), model["clips"]),
        ("Runtime sets", ("id", "kind", "members", "assessment", "coverage", "evidence_refs"), model["runtime_sets"]),
        ("Validation profiles", ("id", "status", "activation_basis", "evidence_refs"), model["profiles"]),
        ("Pipeline stages", ("id", "coverage", "evidence_refs"), model["pipeline_stages"]),
        ("Readiness lanes", ("id", "state", "adoption_consequence", "evidence_refs"), model["readiness"]),
        ("Capabilities", ("id", "state", "evidence_refs"), model["capabilities"]),
        ("Integration steps", ("id", "order", "action", "movement_owner", "phase_owner", "coordinates_or_thresholds", "evidence_refs"), model["integration_steps"]),
        ("Issues", ("id", "severity", "impact", "primary_owner", "current_action", "future_candidate", "secondary_workaround", "evidence_refs"), model["issues"]),
        ("Remediations", ("id", "run_id", "state", "input_evidence_refs", "output_id", "refusal_evidence_refs", "historical_output_id"), model["remediations"]),
        ("Engine evidence", ("id", "runtime", "version", "level", "coverage", "settings", "procedure", "evidence_refs"), model["engine_evidence"]),
        ("Limitations", ("id", "summary", "evidence_refs"), model["limitations"]),
        ("Source provenance", ("id", "source_commit", "report_sha256", "acquisition_scope", "license_scope", "evidence_kind", "evidence_refs"), model["sources"]),
        ("Narratives", ("id", "slot", "text", "fact_refs"), model["narratives"]),
        ("Collection constituents", ("id", "model_sha256", "clip_ids", "source_file_count", "runtime_set_ids"), model["collection"]["constituents"]),
        ("Collection exclusions", ("id", "reason", "evidence_refs"), model["collection"]["exclusions"]),
        ("Cross-pack records", ("id", "left", "right", "result", "evidence_refs"), model["collection"]["cross_pack_records"]),
        ("Derived collection totals", ("constituents", "logical_clips", "source_files", "runtime_sets", "pair_records"), [collection_totals]),
        ("Binding identity", ("collection_id", "manifest_sha256", "manifest_bytes"), [model["binding"]]),
        ("Binding projection totals", ("sources", "readable_sources", "established_sources", "clips", "established_clips", "runtime_sets", "complete_runtime_sets", "incomplete"), [binding["summary"]]),
        ("Binding source witnesses", ("key", "input", "digest", "config", "loader", "take_inventory", "observed_take_count", "result"), binding["sources"]),
        ("Binding clip witnesses", ("id", "source", "take_index", "take_name", "binding"), binding["clips"]),
        ("Binding runtime-set witnesses", ("id", "kind", "members", "lifecycle", "decision", "gap_count"), binding["runtime_sets"]),
    ]


def _authority_ledger(model: dict[str, Any], binding: dict[str, Any]) -> str:
    """Render all authority-ledger tables in their closed fixed order."""
    return "\n".join(_ledger_table(title, columns, records) for title, columns, records in _ledger_sections(model, binding))


def _evidence_relationship_rows(model: dict[str, Any]) -> list[dict[str, str]]:
    """Flatten every declared evidence edge in stable record and field order."""
    rows: list[dict[str, str]] = []
    fields = (
        ("runs", ("evidence_refs",)), ("clips", ("evidence_refs",)),
        ("runtime_sets", ("evidence_refs",)), ("profiles", ("evidence_refs",)),
        ("pipeline_stages", ("evidence_refs",)), ("readiness", ("evidence_refs",)),
        ("capabilities", ("evidence_refs",)), ("integration_steps", ("evidence_refs",)),
        ("issues", ("evidence_refs",)), ("remediations", ("input_evidence_refs", "refusal_evidence_refs")),
        ("engine_evidence", ("evidence_refs",)), ("limitations", ("evidence_refs",)),
        ("sources", ("evidence_refs",)), ("collection.exclusions", ("evidence_refs",)),
        ("collection.cross_pack_records", ("evidence_refs",)),
    )
    for family, refs_fields in fields:
        records = model["collection"][family.split(".", 1)[1]] if family.startswith("collection.") else model[family]
        for record in records:
            for refs_field in refs_fields:
                for reference in record[refs_field]:
                    rows.append({"family": family, "record_id": record["id"], "field": refs_field, "evidence_ref": reference})
    return rows


def _evidence_relationship_table(model: dict[str, Any]) -> str:
    evidence = {record["id"]: record for record in model["evidence"]}
    rows = _evidence_relationship_rows(model)
    return "\n".join([
        "#### Evidence relationships",
        "| family | record_id | field | evidence_ref | evidence locator |",
        "| --- | --- | --- | --- | --- |",
        *[
            f"| {_literal(row['family'])} | {_literal(row['record_id'])} | {_literal(row['field'])} | {_literal(row['evidence_ref'])} | {_link(evidence[row['evidence_ref']]['summary'], evidence[row['evidence_ref']]['locator'])} |"
            for row in rows
        ], "",
    ])


def render_views(model: dict[str, Any], binding: dict[str, Any], *, report_name: str, appendix_name: str) -> RenderedViews:
    """Render deterministic LF-only views after strict model/binding validation."""
    errors = model_validator.validate_model(model, binding)
    if errors:
        raise ValueError("invalid evaluation model: " + "; ".join(errors))
    digest = model_contract.canonical_digest(model)
    presentation = model["presentation"]
    runtime_rows = _runtime_rows(model)
    issue_rows = [
        f"| {_code(issue['id'])} | {_text(issue['severity'])} | {_text(issue['impact'])} Secondary workaround: {_text(issue['secondary_workaround'])}. {_link('Readiness guidance', READINESS_LADDER)} | "
        f"{_text(issue['primary_owner'])} | {_text(issue['current_action'])} | {_text(issue['future_candidate'])} | {_evidence(model, issue['evidence_refs'])} |"
        for issue in model["issues"]
    ]
    source_rows = [
        f"- {_code(record['id'])}: commit {_code(record['source_commit'])}; report digest {_code(record['report_sha256'])}; acquisition {_text(record['acquisition_scope'])}; license {_text(record['license_scope'])}; evidence kind {_text(record['evidence_kind'])}; {_evidence(model, record['evidence_refs'])}."
        for record in model["sources"]
    ] or ["- No source records were recorded."]
    capabilities = {"pass": [], "finding": [], "not-evaluated": [], "not-applicable": []}
    for capability in model["capabilities"]:
        capabilities[capability["state"]].append(f"{_code(capability['id'])}: {_evidence(model, capability['evidence_refs'])}")
    recipe = [
        f"{step['order']}. **{label}:** `{key}={_text(step['action'])}`; {_text(step['coordinates_or_thresholds'])}; movement owner={_text(step['movement_owner'])}; phase owner={_text(step['phase_owner'])}; step={_code(step['id'])}; {_evidence(model, step['evidence_refs'])}."
        for step, label, key in zip(model["integration_steps"], report_validator.RECIPE_LABELS, ("topology", "sync", "owner", "composition", "gate"))
    ]
    while len(recipe) < 5:
        index = len(recipe)
        label, key = report_validator.RECIPE_LABELS[index], ("topology", "sync", "owner", "composition", "gate")[index]
        recipe.append(f"{index + 1}. **{label}:** `{key}=not-evaluated`; no V1 step was recorded.")
    report = "\n".join([
        f"# Animation pack evaluation: {_text(presentation['title'])}", "",
        f"> Technical verdict: **{_text(presentation['verdict'])}**", ">",
        f"> Evaluation completeness: **{_text(presentation['completeness'])}** — {_narrative(model, 'evidence-status', 'Bounded to the V1 evidence records.')}", ">",
        f"> Confidence: **{_text(presentation['confidence'])}**", ">",
        f"> Evaluation date: **{_text(presentation['evaluation_date'])}**", ">",
        "> Report format: **1**", ">",
        f"> Detailed evidence: {_link(appendix_name, appendix_name)}", "",
        "## Technical decision", _narrative(model, "technical-decision", "No technical-decision narrative was recorded."), "",
        "## Capability coverage", "", "### Complete core", "\n".join("- " + value for value in capabilities["pass"]) or "- None recorded.", "",
        "### Partial supporting gameplay", "\n".join("- " + value for value in capabilities["finding"]) or "- None recorded.", "",
        "### Absent", "- No capability is established as absent by V1 evidence.", "",
        "### Not evaluated", "\n".join("- " + value for value in capabilities["not-evaluated"]) or "- None recorded.", "",
        "### Not applicable", "\n".join("- " + value for value in capabilities["not-applicable"]) or "- None recorded.", "",
        "## Runtime sets and authored motion",
        "\n".join(["| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |", "|---|---|---|---|---|---|"] + runtime_rows) if runtime_rows else "No important runtime sets were identified.", "",
        "## Integration recipe", "\n".join(recipe), "",
        "## Technical issue register",
        "\n".join(["| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |", "|---|---|---|---|---|---|---|"] + issue_rows) if issue_rows else "No material technical issues were found at the stated scope.", "",
        "## Engine status", "| Runtime | Evidence level | Technical result | Remaining gate |", "|---|---|---|---|", *(_engine_rows(model)), "",
        "## Fit and limitations", _narrative(model, "fit-and-limitations", "See the typed limitations in the appendix."), "",
        "## Evidence status", f"Model schema: {_code(model_contract.SCHEMA)}; schema version: {_code(model_contract.SCHEMA_VERSION)}; digest: {_code(digest)}; renderer: {_code(RENDERER_VERSION)}. {_link('Canonical readiness ladder', READINESS_LADDER)}.", "",
        "## Sources", *source_rows, "",
    ])
    role_rows = []
    for role in contract.PRIMARY_ROLES:
        total = sum(clip["primary_role"] == role for clip in model["clips"])
        physical = len({clip["source"] for clip in model["clips"] if clip["primary_role"] == role})
        role_rows.append(f"| {_code(role)} | {total} | {physical} | Unique source files used by this role; roles overlap and are non-additive. |")
    clip_rows = [f"| {_code(clip['id'])} | {_code(clip['source'])} | {clip['take_index']} | {_text(clip['take_name'])} | {_code(clip['primary_role'])} | {_text(clip['loop'])} | {_availability(clip['duration_s'])} | {_availability(clip['root_motion_speed_mps'])} | {_text(clip['movement_owner'])} | {_text(clip['assessment'])}/{_text(clip['coverage'])}; {_evidence(model, clip['evidence_refs'])} |" for clip in model["clips"]]
    cross_rows = [
        f"| {_code(record['left'])} / {_code(record['right'])} | {_text(record['result'])} | "
        f"collection pair | {_text(record['result'])} | model-owned result | {_evidence(model, record['evidence_refs'])} |"
        for record in model["collection"]["cross_pack_records"]
    ] or ["| No cross-pack pair records. | not-evaluated | not-evaluated | not-evaluated | not-evaluated | No linked evidence. |"]
    limitation_rows = [
        f"1. {_text(record['summary'])} {_evidence(model, record['evidence_refs'])} id={_code(record['id'])}"
        for record in model["limitations"]
    ] or ["1. No limitations were recorded."]
    appendix = "\n".join([
        f"# Animation pack evidence appendix: {_text(presentation['title'])}", "",
        f"> Companion report: {_link(report_name, report_name)}", ">",
        f"> Evidence status: **{_text(presentation['completeness'])}** — {_narrative(model, 'evidence-status', 'Bounded to the V1 evidence records.')}", ">",
        f"> Evaluation date: **{_text(presentation['evaluation_date'])}**", ">", "> Report format: **1**", "",
        f"Model schema: {_code(model_contract.SCHEMA)}; version: {_code(model_contract.SCHEMA_VERSION)}; canonical digest: {_code(digest)}; renderer: {_code(RENDERER_VERSION)}. {_link('Canonical readiness ladder', READINESS_LADDER)} is authoritative.", "",
        "## Evaluation scope and provenance", _narrative(model, "evidence-status", "Synthetic or declared V1 provenance is listed below."), "",
        "### Evidence coverage", f"Validated source records: {len(binding.get('sources', []))}.", "", "### Claim legend", "Evidence kinds are fixed V1 vocabulary.", "",
        "## Evaluation manifest and taxonomy", "", "### Canonical clip-role inventory", "| Canonical primary role | Logical motions | Unique source files used by role | Evidence boundary |", "|---|---:|---:|---|", *role_rows, f"| **Total** | **{len(model['clips'])}** | **{len(binding['sources'])}** | All-unique source files across overlapping, non-additive roles; V1 binding: {_code(model['binding']['collection_id'])}. |", "",
        "### Runtime-set inventory", "\n".join(["| Runtime set | Type | Members/variants | Grouping evidence | Validation status |", "|---|---|---|---|---|"] + [f"| {_code(record['id'])} | {_text(record['kind'])} | " + "; ".join(_code(member['clip_id']) + "=" + _text(member['eligibility']) for member in record['members']) + f" | {_evidence(model, record['evidence_refs'])} | {_text(record['assessment'])}/{_text(record['coverage'])} |" for record in model['runtime_sets']]) if model['runtime_sets'] else "No runtime sets were identified.", "",
        "### Pipeline-stage coverage", "| Stage | Coverage state | Evidence / remaining gate |", "|---|---|---|", *[f"| {_text(dict(contract.PIPELINE_STAGE_ROWS)[record['id']])} | {_code(record['coverage'])} | id={_code(record['id'])}; {_evidence(model, record['evidence_refs'])} |" for record in model['pipeline_stages']], "",
        "### Readiness evidence by clip set", "| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |", "|---|---|---|---|", *[f"| {_code(record['id'])} | {_text(record['state'])} | {_text(record['adoption_consequence'])} | {_evidence(model, record['evidence_refs'])} |" for record in model['readiness']], "",
        "### Validation-profile status", "| Validation profile | Selection | Result / next evidence |", "|---|---|---|", *[f"| {_text(dict(contract.PROFILE_ROWS)[record['id']])} | " + (f"{_code('selected')} — {_code(record['activation_basis'])}" if record['status'] == 'selected' else _code(record['status'])) + f" | id={_code(record['id'])}; {_evidence(model, record['evidence_refs'])} |" for record in model['profiles']], "",
        "## Pack inventory and content evidence", _narrative(model, "pack-inventory", "No inventory narrative was recorded."), "| Clip | Source | Take | Take name | Role | Loop | Duration | RM speed | Movement owner | Assessment |", "|---|---|---:|---|---|---|---|---|---|---|", *clip_rows, "",
        "## Mechanical baseline", _narrative(model, "mechanical-baseline", "No mechanical-baseline narrative was recorded."), "",
        "## AnimSmith remediation evidence", "| ID | Run | State | Output | Historical output | Evidence |", "|---|---|---|---|---|---|", *[f"| {_code(record['id'])} | {_code(record['run_id'])} | {_text(record['state'])} | {_code(record['output_id'])} | {_code(record['historical_output_id'])} | {_evidence(model, record['input_evidence_refs'] + record['refusal_evidence_refs'])} |" for record in model['remediations']], "",
        "## Engine procedures and evidence", "| Runtime | Version | Procedure | Observed result | Remaining gate |", "|---|---|---|---|---|", *[f"| {_text(record['runtime'])} | {_text(record['version'])} | id={_code(record['id'])}; {_text(record['procedure'])}; {_evidence(model, record['evidence_refs'])} | {_text(record['level'])}/{_text(record['coverage'])} | {_text(record['settings'])} |" for record in model['engine_evidence']], "",
        "## Rig, masking, and compatibility evidence", "| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |", "|---|---|---|---|---|---|", *cross_rows, "",
        "## Limitations and unknowns", *limitation_rows, "",
        "## Reproduction", _narrative(model, "reproduction", "Use the model digest and source records below."), "", "### V1 authoritative field ledger", _authority_ledger(model, binding), _evidence_relationship_table(model), "```json", model_contract.canonical_json({"binding": _public_binding_projection(binding), "model": model}).decode("utf-8"), "```", "",
        "## Sources", *source_rows, "",
    ])
    return RenderedViews(report + "\n", appendix + "\n")


def validate_views(model: dict[str, Any], binding: dict[str, Any], views: RenderedViews, *, report_name: str, appendix_name: str) -> list[str]:
    """Use the pinned AST parser, then prove the fixed projections match V1."""
    errors = model_validator.validate_model(model, binding)
    errors.extend(report_validator.validate(views.report, evaluation_schema=model_contract.SCHEMA))
    errors.extend(report_validator.validate_appendix(views.appendix, evaluation_schema=model_contract.SCHEMA))
    errors.extend(report_validator.validate_pair(views.report, views.appendix, report_name, appendix_name))
    report_ast, appendix_ast = report_validator.parse_markdown(views.report), report_validator.parse_markdown(views.appendix)
    for title, columns, records in _ledger_sections(model, binding):
        matching = [
            table for table in appendix_ast["tables"]
            if tuple(cell["text"] for cell in table["header"]) == columns
            and table["section"] == "Reproduction"
            and table["subsection"] == "V1 authoritative field ledger"
        ]
        if len(matching) != 1:
            errors.append(f"model-to-view missing fixed authority table: {title}")
            continue
        actual = matching[0]["rows"]
        if len(actual) != len(records):
            errors.append(f"model-to-view {title} row count differs from authority")
            continue
        for row_index, (row, record) in enumerate(zip(actual, records), start=1):
            if len(row) != len(columns):
                errors.append(f"model-to-view {title} row {row_index} cell count differs from authority")
                continue
            expected = [_literal_value(record.get(column)) for column in columns]
            observed = [cell["text"] for cell in row]
            if observed != expected:
                errors.append(f"model-to-view {title} row {row_index} differs from authority")
    expected_evidence_links = {(record["summary"], record["locator"]) for record in model["evidence"]}
    actual_links = {
        (link["text"], link["destination"])
        for document in (report_ast, appendix_ast)
        for link in document["links"]
    }
    for label, destination in expected_evidence_links:
        if (label, destination) not in actual_links:
            errors.append(f"model-to-view missing exact evidence link: {destination}")
    relationship_header = ("family", "record_id", "field", "evidence_ref", "evidence locator")
    relationship_tables = [table for table in appendix_ast["tables"] if tuple(cell["text"] for cell in table["header"]) == relationship_header]
    expected_relationships = _evidence_relationship_rows(model)
    if len(relationship_tables) != 1 or len(relationship_tables[0]["rows"]) != len(expected_relationships):
        errors.append("model-to-view evidence relationship rows differ from authority")
    elif relationship_tables:
        evidence = {record["id"]: record for record in model["evidence"]}
        for index, (row, expected) in enumerate(zip(relationship_tables[0]["rows"], expected_relationships), start=1):
            if [cell["text"] for cell in row[:4]] != [_literal_value(expected[key]) for key in ("family", "record_id", "field", "evidence_ref")]:
                errors.append(f"model-to-view evidence relationship {index} differs from authority")
                continue
            links = row[4]["links"] if len(row) == 5 else []
            record = evidence[expected["evidence_ref"]]
            if len(links) != 1 or links[0]["destination"] != record["locator"] or links[0]["text"] != record["summary"]:
                errors.append(f"model-to-view evidence relationship {index} has a misattached link")
    evidence = {record["id"]: record for record in model["evidence"]}
    def assert_projection_links(document: dict[str, Any], header: tuple[str, ...], records: list[dict[str, Any]], cell: int, fields: tuple[str, ...], label: str) -> None:
        tables = [table for table in document["tables"] if tuple(item["text"] for item in table["header"]) == header]
        if len(tables) != 1 or len(tables[0]["rows"]) != len(records):
            errors.append(f"model-to-view {label} evidence rows differ from authority")
            return
        for index, (row, record) in enumerate(zip(tables[0]["rows"], records), start=1):
            expected = {(evidence[ref]["summary"], evidence[ref]["locator"]) for field in fields for ref in record[field]}
            observed = {(link["text"], link["destination"]) for link in row[cell]["links"]}
            if observed != expected:
                errors.append(f"model-to-view {label} row {index} has misattached evidence")
    def assert_paragraph_links(document: dict[str, Any], records: list[dict[str, Any]], fields: tuple[str, ...], label: str) -> None:
        for record in records:
            expected = {(evidence[ref]["summary"], evidence[ref]["locator"]) for field in fields for ref in record[field]}
            matching = [paragraph for paragraph in document["paragraphs"] if record["id"] in paragraph["code"]]
            if len(matching) != 1:
                errors.append(f"model-to-view {label} {record['id']} is missing its human projection")
                continue
            observed = {(link["text"], link["destination"]) for link in matching[0]["links"]}
            if observed != expected:
                errors.append(f"model-to-view {label} {record['id']} has misattached evidence")
    assert_projection_links(report_ast, report_validator.RUNTIME_SET_HEADER, model["runtime_sets"], 1, ("evidence_refs",), "primary runtime-set")
    assert_projection_links(report_ast, report_validator.ISSUE_HEADER, model["issues"], 6, ("evidence_refs",), "issue")
    assert_projection_links(report_ast, report_validator.ENGINE_HEADER, [next(record for record in model["engine_evidence"] if record["runtime"] == runtime) for runtime in report_validator.ENGINE_LABELS], 2, ("evidence_refs",), "primary engine")
    assert_projection_links(appendix_ast, report_validator.APPENDIX_RUNTIME_HEADER, model["runtime_sets"], 3, ("evidence_refs",), "runtime-set")
    assert_projection_links(appendix_ast, report_validator.PIPELINE_HEADER, model["pipeline_stages"], 2, ("evidence_refs",), "pipeline")
    assert_projection_links(appendix_ast, ("Role or runtime set", "File-ready / clip-ready", "Set-ready / rig-use", "Runtime / acceptance boundary"), model["readiness"], 3, ("evidence_refs",), "readiness")
    assert_projection_links(appendix_ast, report_validator.PROFILE_HEADER, model["profiles"], 2, ("evidence_refs",), "profile")
    assert_projection_links(appendix_ast, ("ID", "Run", "State", "Output", "Historical output", "Evidence"), model["remediations"], 5, ("input_evidence_refs", "refusal_evidence_refs"), "remediation")
    assert_projection_links(appendix_ast, ("Runtime", "Version", "Procedure", "Observed result", "Remaining gate"), model["engine_evidence"], 2, ("evidence_refs",), "appendix engine")
    if model["collection"]["cross_pack_records"]:
        assert_projection_links(appendix_ast, ("Pack/rig/set pair", "Skeleton/retarget", "Scale/axes", "Root policy", "Timing/blend", "Overall evidence"), model["collection"]["cross_pack_records"], 5, ("evidence_refs",), "cross-pack")
    assert_projection_links(appendix_ast, ("Clip", "Source", "Take", "Take name", "Role", "Loop", "Duration", "RM speed", "Movement owner", "Assessment"), model["clips"], 9, ("evidence_refs",), "clip")
    assert_paragraph_links(report_ast, model["capabilities"], ("evidence_refs",), "capability")
    assert_paragraph_links(report_ast, model["integration_steps"], ("evidence_refs",), "integration step")
    assert_paragraph_links(report_ast, model["sources"], ("evidence_refs",), "source")
    assert_paragraph_links(appendix_ast, model["limitations"], ("evidence_refs",), "limitation")
    assert_paragraph_links(appendix_ast, model["sources"], ("evidence_refs",), "source")
    if report_validator.rendered_word_count(views.report) > report_validator.MAX_PRIMARY_WORDS:
        errors.append("model-to-view primary report exceeds the word cap")
    return errors


@dataclass
class _OutputTarget:
    """A final filename anchored to a retained, no-follow parent directory."""

    parent: Path
    name: str
    directory_fd: int
    parent_device: int
    parent_inode: int

    @property
    def path(self) -> Path:
        return self.parent / self.name


@dataclass(frozen=True)
class _StagedEntry:
    """A staged regular file whose device/inode identify a safe rollback target."""

    name: str
    device: int
    inode: int


@dataclass(frozen=True)
class _BackupEntry:
    """The exact original regular output retained for an interrupted publish."""

    name: str
    device: int
    inode: int


class PublicationRecoveryError(ValueError):
    """A publication preserved unexpected data and requires operator recovery."""


def _replace_dir_fd_available() -> bool:
    """Check the Python build exposes both descriptor arguments to replace."""
    try:
        replace_parameters = inspect.signature(os.replace).parameters
    except (TypeError, ValueError):
        return False
    return "src_dir_fd" in replace_parameters and "dst_dir_fd" in replace_parameters


REPLACE_DIR_FD_AVAILABLE = _replace_dir_fd_available()


def _safe_dir_fd_available() -> bool:
    """Whether this host can perform every publication operation by dir_fd."""
    return (
        os.name != "nt"
        and hasattr(os, "O_DIRECTORY")
        and hasattr(os, "O_NOFOLLOW")
        and os.open in os.supports_dir_fd
        and os.stat in os.supports_dir_fd
        and os.unlink in os.supports_dir_fd
        and os.rename in os.supports_dir_fd
        and REPLACE_DIR_FD_AVAILABLE
    )


def _canonical_output_path(requested: Path) -> Path:
    """Require a pre-existing real parent and retain no final-component alias."""
    try:
        parent = requested.parent.resolve(strict=True)
    except (OSError, RuntimeError) as error:
        raise ValueError("render output parent must already exist as a directory") from error
    try:
        mode = parent.lstat().st_mode
    except OSError as error:
        raise ValueError("render output parent must already exist as a directory") from error
    if stat.S_ISLNK(mode) or not stat.S_ISDIR(mode):
        raise ValueError("render output parent must be a real directory")
    return parent / requested.name


def _checked_paths(model: Path, binding: Path, report: Path, appendix: Path) -> tuple[Path, Path, Path, Path]:
    """Refuse lexical, symlink, and existing hard-link input/output aliases."""
    for requested in (report, appendix):
        try:
            mode = requested.lstat().st_mode
        except FileNotFoundError:
            continue
        if stat.S_ISLNK(mode):
            raise ValueError("render output must not be a symlink")
        if not stat.S_ISREG(mode):
            raise ValueError("render output must be a regular file when it already exists")
    inputs = (model.resolve(strict=True), binding.resolve(strict=True))
    # Canonicalize only pre-existing parents; publication subsequently opens
    # those canonical directories with O_NOFOLLOW and uses their descriptors
    # for every stage, backup, and replacement operation.  Never resolve the
    # final output component.
    outputs = tuple(_canonical_output_path(path) for path in (report, appendix))
    if outputs[0] == outputs[1] or (outputs[0].exists() and outputs[1].exists() and os.path.samefile(outputs[0], outputs[1])):
        raise ValueError("report and appendix outputs must be distinct")
    for output in outputs:
        for input_path in inputs:
            if output == input_path or (output.exists() and os.path.samefile(output, input_path)):
                raise ValueError("render output must not alias a model or binding input")
    return inputs[0], inputs[1], outputs[0], outputs[1]


def _open_output_targets(report: Path, appendix: Path) -> tuple[_OutputTarget, _OutputTarget]:
    """Open already-canonical output parents once, refusing unsafe hosts."""
    if not _safe_dir_fd_available():
        raise ValueError("safe descriptor-relative render publication is unavailable on this platform")
    opened: list[_OutputTarget] = []
    try:
        for path in (report, appendix):
            parent = path.parent
            try:
                parent_metadata = parent.lstat()
            except OSError as error:
                raise ValueError("render output parent must already exist as a directory") from error
            if stat.S_ISLNK(parent_metadata.st_mode) or not stat.S_ISDIR(parent_metadata.st_mode):
                raise ValueError("render output parent must be a real directory")
            descriptor = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
            metadata = os.fstat(descriptor)
            if not stat.S_ISDIR(metadata.st_mode):
                os.close(descriptor)
                raise ValueError("render output parent must be a real directory")
            if (metadata.st_dev, metadata.st_ino) != (parent_metadata.st_dev, parent_metadata.st_ino):
                os.close(descriptor)
                raise ValueError("render output parent changed during validation")
            opened.append(_OutputTarget(parent, path.name, descriptor, metadata.st_dev, metadata.st_ino))
        if opened[0].parent_device == opened[1].parent_device and opened[0].parent_inode == opened[1].parent_inode and opened[0].name == opened[1].name:
            raise ValueError("report and appendix outputs must be distinct")
        return opened[0], opened[1]
    except BaseException:
        for target in opened:
            os.close(target.directory_fd)
        raise


def _close_output_targets(targets: tuple[_OutputTarget, _OutputTarget]) -> None:
    for target in targets:
        os.close(target.directory_fd)


def _checked_targets(model: Path, binding: Path, report: Path, appendix: Path) -> tuple[Path, Path, _OutputTarget, _OutputTarget]:
    """Validate and immediately retain output parent handles for publication."""
    model_path, binding_path, report_path, appendix_path = _checked_paths(model, binding, report, appendix)
    report_target, appendix_target = _open_output_targets(report_path, appendix_path)
    return model_path, binding_path, report_target, appendix_target


def _lstat_at(target: _OutputTarget, name: str | None = None) -> os.stat_result | None:
    try:
        return os.stat(target.name if name is None else name, dir_fd=target.directory_fd, follow_symlinks=False)
    except FileNotFoundError:
        return None


def _temporary_name(prefix: str) -> str:
    return f".{prefix}-{uuid.uuid4().hex}"


def _stage(target: _OutputTarget, content: str | bytes) -> _StagedEntry:
    """Create and fsync a unique staging entry through the retained directory fd."""
    payload = content.encode("utf-8") if isinstance(content, str) else content
    for _ in range(16):
        name = _temporary_name("animsmith-render-stage")
        try:
            descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600, dir_fd=target.directory_fd)
        except FileExistsError:
            continue
        try:
            remaining = memoryview(payload)
            while remaining:
                written = os.write(descriptor, remaining)
                if written == 0:
                    raise OSError("short write while staging rendered view")
                remaining = remaining[written:]
            os.fsync(descriptor)
            metadata = os.fstat(descriptor)
        except BaseException:
            try:
                os.unlink(name, dir_fd=target.directory_fd)
            except OSError:
                pass
            raise
        finally:
            os.close(descriptor)
        return _StagedEntry(name, metadata.st_dev, metadata.st_ino)
    raise OSError("unable to reserve a render staging name")


def _backup_name(target: _OutputTarget) -> str:
    """Reserve an unused same-directory backup name through the retained fd."""
    for _ in range(16):
        name = _temporary_name("animsmith-render-backup")
        try:
            descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600, dir_fd=target.directory_fd)
        except FileExistsError:
            continue
        os.close(descriptor)
        os.unlink(name, dir_fd=target.directory_fd)
        return name
    raise OSError("unable to reserve a render backup name")


def _identity_matches(metadata: os.stat_result | None, device: int, inode: int) -> bool:
    return (
        metadata is not None
        and stat.S_ISREG(metadata.st_mode)
        and (metadata.st_dev, metadata.st_ino) == (device, inode)
    )


def _cleanup_exact_regular_entry(target: _OutputTarget, name: str, device: int, inode: int, label: str) -> str | None:
    """Remove only the exact internal entry, preserving any substituted name."""
    metadata = _lstat_at(target, name)
    if metadata is None:
        return None
    if not _identity_matches(metadata, device, inode):
        return f"unexpected {label} entry at {target.parent / name} preserved"
    os.unlink(name, dir_fd=target.directory_fd)
    return None


def _parent_identity_matches(target: _OutputTarget) -> bool:
    """Whether the canonical parent name still identifies the retained fd."""
    try:
        metadata = target.parent.lstat()
    except OSError:
        return False
    return (
        not stat.S_ISLNK(metadata.st_mode)
        and stat.S_ISDIR(metadata.st_mode)
        and (metadata.st_dev, metadata.st_ino) == (target.parent_device, target.parent_inode)
    )


def _assert_parent_identities(targets: tuple[_OutputTarget, _OutputTarget]) -> None:
    """Fail closed if either parent has been renamed, replaced, or redirected."""
    if not all(_parent_identity_matches(target) for target in targets):
        raise ValueError("render output parent is no longer the retained real directory")


def _require_regular_or_absent(target: _OutputTarget, name: str | None = None) -> os.stat_result | None:
    """Return an absent/regular entry, refusing a raced nonregular final name."""
    metadata = _lstat_at(target, name)
    if metadata is not None and not stat.S_ISREG(metadata.st_mode):
        raise ValueError("render output must be a regular file when it already exists")
    return metadata


def _publish_open_pair(targets: tuple[_OutputTarget, _OutputTarget], views: RenderedViews) -> None:
    """Publish via retained descriptors, checking parent identity around every phase.

    These checks prevent path redirection and fail closed on an observed parent
    rename.  They cannot prevent a rename after the final check and successful
    return; no pathname-based publisher can make that post-return guarantee.
    """
    _assert_parent_identities(targets)
    for target in targets:
        _require_regular_or_absent(target)
    staged: tuple[_StagedEntry, ...] = ()
    backups: list[_BackupEntry | None] = [None, None]
    published = [False, False]
    remove_backups = True
    recovery: list[str] = []
    try:
        staged = (_stage(targets[0], views.report), _stage(targets[1], views.appendix))
        _assert_parent_identities(targets)
        for index, target in enumerate(targets):
            if _require_regular_or_absent(target) is not None:
                _assert_parent_identities(targets)
                backups[index] = _backup_name(target)
                # Recheck the final component immediately before mutating it:
                # a nonregular entry must never be hidden under a backup name.
                if _require_regular_or_absent(target) is None:
                    backups[index] = None
                    continue
                backup_name = backups[index]
                os.replace(target.name, backup_name, src_dir_fd=target.directory_fd, dst_dir_fd=target.directory_fd)
                backup_metadata = _require_regular_or_absent(target, backup_name)
                if backup_metadata is None:
                    raise OSError("render output backup disappeared during publication")
                backups[index] = _BackupEntry(backup_name, backup_metadata.st_dev, backup_metadata.st_ino)
        for index, target in enumerate(targets):
            _assert_parent_identities(targets)
            os.replace(staged[index].name, target.name, src_dir_fd=target.directory_fd, dst_dir_fd=target.directory_fd)
            published[index] = True
        _assert_parent_identities(targets)
    except (OSError, ValueError) as error:
        for index, target in enumerate(targets):
            backup = backups[index]
            try:
                target_metadata = _lstat_at(target)
                expected_stage = staged[index] if index < len(staged) else None
                target_is_our_stage = expected_stage is not None and _identity_matches(target_metadata, expected_stage.device, expected_stage.inode)
                if backup is not None:
                    backup_metadata = _lstat_at(target, backup.name)
                    if not _identity_matches(backup_metadata, backup.device, backup.inode):
                        recovery.append(f"original output recovery entry {target.parent / backup.name} changed")
                        remove_backups = False
                        continue
                    if target_metadata is not None and not target_is_our_stage:
                        recovery.append(f"unexpected output entry at {target.path}; original regular output retained at {target.parent / backup.name}")
                        remove_backups = False
                        continue
                    if target_is_our_stage:
                        os.unlink(target.name, dir_fd=target.directory_fd)
                    os.replace(backup.name, target.name, src_dir_fd=target.directory_fd, dst_dir_fd=target.directory_fd)
                elif published[index] and target_metadata is not None:
                    if target_is_our_stage:
                        os.unlink(target.name, dir_fd=target.directory_fd)
                    else:
                        recovery.append(f"unexpected output entry at {target.path} has no original backup")
                        remove_backups = False
            except OSError:
                # Preserve the primary publish refusal; remaining backups stay
                # beside their target for an operator rather than discarding data.
                if backup is not None:
                    recovery.append(f"unable to restore original regular output; retained backup at {target.parent / backup.name}")
                else:
                    recovery.append(f"unable to remove renderer-owned staged output at {target.path}")
                remove_backups = False
        if recovery:
            raise PublicationRecoveryError("render publication requires manual recovery: " + "; ".join(recovery)) from error
        raise
    finally:
        for index, temporary in enumerate(staged):
            message = _cleanup_exact_regular_entry(
                targets[index], temporary.name, temporary.device, temporary.inode, "staging"
            )
            if message is not None:
                recovery.append(message)
        if remove_backups:
            for index, backup in enumerate(backups):
                if backup is not None:
                    message = _cleanup_exact_regular_entry(
                        targets[index], backup.name, backup.device, backup.inode, "backup"
                    )
                    if message is not None:
                        recovery.append(message)
        if recovery:
            raise PublicationRecoveryError("render publication requires manual recovery: " + "; ".join(recovery))


def _publish_pair(report: _OutputTarget, appendix: _OutputTarget, views: RenderedViews) -> None:
    """Publish two views without reopening a validated parent path."""
    _publish_open_pair((report, appendix), views)


def _matches_expected(path: Path, expected: bytes) -> bool:
    """Compare a generated view without allocating an arbitrary existing file."""
    try:
        if not path.is_file() or path.stat().st_size != len(expected):
            return False
        with path.open("rb") as handle:
            for offset in range(0, len(expected), 64 * 1024):
                if handle.read(min(64 * 1024, len(expected) - offset)) != expected[offset:offset + 64 * 1024]:
                    return False
            return handle.read(1) == b""
    except OSError:
        return False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path); parser.add_argument("--binding", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True); parser.add_argument("--appendix", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        if args.check:
            # Check-only comparison is cross-platform and never publishes.
            model_path, binding_path, report_path, appendix_path = _checked_paths(args.model, args.binding, args.report, args.appendix)
            model, _ = model_validator.load_json(model_path); binding, _ = model_validator.load_json(binding_path)
            views = render_views(model, binding, report_name=report_path.name, appendix_name=appendix_path.name)
            errors = validate_views(model, binding, views, report_name=report_path.name, appendix_name=appendix_path.name)
            if errors:
                raise ValueError("; ".join(errors))
            if not _matches_expected(report_path, views.report.encode()) or not _matches_expected(appendix_path, views.appendix.encode()):
                print("render_evaluation_model.py: generated views are stale", file=sys.stderr); return 1
        else:
            # Write mode fails closed on hosts without descriptor-relative
            # no-follow operations; no path-based publication fallback exists.
            model_path, binding_path, report_target, appendix_target = _checked_targets(args.model, args.binding, args.report, args.appendix)
            try:
                report_path, appendix_path = report_target.path, appendix_target.path
                model, _ = model_validator.load_json(model_path); binding, _ = model_validator.load_json(binding_path)
                views = render_views(model, binding, report_name=report_path.name, appendix_name=appendix_path.name)
                errors = validate_views(model, binding, views, report_name=report_path.name, appendix_name=appendix_path.name)
                if errors:
                    raise ValueError("; ".join(errors))
                _publish_pair(report_target, appendix_target, views)
            finally:
                _close_output_targets((report_target, appendix_target))
    except (OSError, TypeError, ValueError, RecursionError) as error:
        print(f"render_evaluation_model.py: {error}", file=sys.stderr); return 2
    print(f"rendered animation-pack evaluation views: {args.report} + {args.appendix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
