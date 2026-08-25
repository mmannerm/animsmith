#!/usr/bin/env python3
"""Render the fixed V1 evaluation report and evidence appendix from one model."""

from __future__ import annotations

import argparse
import os
import re
import stat
import sys
import tempfile
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
    return f"[{_text(label)}](<{destination.replace('>', '%3E')}>)"


def _availability(value: dict[str, Any]) -> str:
    return str(value["value"]) if value["state"] == "available" else str(value["state"])


def _evidence(model: dict[str, Any], refs: list[str]) -> str:
    records = {record["id"]: record for record in model["evidence"]}
    return "; ".join(
        f"{_code(reference)}: {_link(records[reference]['summary'], records[reference]['locator'])}"
        for reference in refs
    ) or "No linked evidence."


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
        timing = "; ".join(
            ([f"duration={durations[0]} s"] if durations else [])
            + ([f"rm_speed={speeds[0]} m/s"] if speeds else [])
        ) or "N/A"
        rows.append(
            f"| {_code(runtime_set['id'])} | kind={_text(runtime_set['kind'])} | {members} | "
            f"set_type={_text(runtime_set['kind'])} | {timing} | "
            f"state={_text(runtime_set['coverage'])} |"
        )
    return rows


def _engine_rows(model: dict[str, Any]) -> list[str]:
    records = {record["runtime"]: record for record in model["engine_evidence"]}
    rows = []
    for runtime in report_validator.ENGINE_LABELS:
        record = records.get(runtime)
        if record is None:
            rows.append(f"| {runtime} | not-evaluated | No V1 engine record. | Add engine evidence. |")
        else:
            rows.append(f"| {_text(runtime)} {_text(record['version'])} | {_text(record['level'])} | {_text(record['procedure'])} | {_text(record['settings'])} |")
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
        ("Binding source witnesses", ("key", "locator", "input", "digest", "config", "loader", "take_inventory", "observed_takes", "result"), binding["sources"]),
        ("Binding clip witnesses", ("id", "source", "take_index", "take_name", "binding"), binding["clips"]),
        ("Binding runtime-set witnesses", ("id", "kind", "members", "lifecycle", "decision", "gaps", "evidence"), binding["runtime_sets"]),
    ]


def _authority_ledger(model: dict[str, Any], binding: dict[str, Any]) -> str:
    """Render all authority-ledger tables in their closed fixed order."""
    return "\n".join(_ledger_table(title, columns, records) for title, columns, records in _ledger_sections(model, binding))


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
        f"- {_code(record['id'])}: commit {_code(record['source_commit'])}; report digest {_code(record['report_sha256'])}; acquisition {_text(record['acquisition_scope'])}; license {_text(record['license_scope'])}; evidence kind {_text(record['evidence_kind'])}."
        for record in model["sources"]
    ] or ["- No source records were recorded."]
    capabilities = {"pass": [], "finding": [], "other": []}
    for capability in model["capabilities"]:
        capabilities["pass" if capability["state"] == "pass" else "finding" if capability["state"] == "finding" else "other"].append(_code(capability["id"]))
    recipe = [
        f"{step['order']}. **{label}:** `{key}={_text(step['action'])}`; {_text(step['coordinates_or_thresholds'])}; movement owner={_text(step['movement_owner'])}; phase owner={_text(step['phase_owner'])}; step={_code(step['id'])}."
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
        "### Absent", "\n".join("- " + value for value in capabilities["other"]) or "- None recorded.", "",
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
        role_rows.append(f"| {_code(role)} | {total} | {physical} | Derived from the independently validated binding witnesses. |")
    clip_rows = [f"| {_code(clip['id'])} | {_code(clip['source'])} | {clip['take_index']} | {_text(clip['take_name'])} | {_code(clip['primary_role'])} | {_text(clip['loop'])} | {_availability(clip['duration_s'])} | {_availability(clip['root_motion_speed_mps'])} | {_text(clip['movement_owner'])} | {_text(clip['assessment'])}/{_text(clip['coverage'])} |" for clip in model["clips"]]
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
        "## Evaluation manifest and taxonomy", "", "### Canonical clip-role inventory", "| Canonical primary role | Logical motions | Delivered files | Evidence boundary |", "|---|---:|---:|---|", *role_rows, f"| **Total** | **{len(model['clips'])}** | **{len(binding['sources'])}** | V1 binding: {_code(model['binding']['collection_id'])}. |", "",
        "### Runtime-set inventory", "\n".join(["| Runtime set | Type | Members/variants | Grouping evidence | Validation status |", "|---|---|---|---|---|"] + [f"| {_code(record['id'])} | {_text(record['kind'])} | " + "; ".join(_code(member['clip_id']) + "=" + _text(member['eligibility']) for member in record['members']) + f" | {_evidence(model, record['evidence_refs'])} | {_text(record['assessment'])}/{_text(record['coverage'])} |" for record in model['runtime_sets']]) if model['runtime_sets'] else "No runtime sets were identified.", "",
        "### Pipeline-stage coverage", "| Stage | Coverage state | Evidence / remaining gate |", "|---|---|---|", *[f"| {_text(dict(contract.PIPELINE_STAGE_ROWS)[record['id']])} | {_code(record['coverage'])} | id={_code(record['id'])}; {_evidence(model, record['evidence_refs'])} |" for record in model['pipeline_stages']], "",
        "### Readiness evidence by clip set", "| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |", "|---|---|---|---|", *[f"| {_code(record['id'])} | {_text(record['state'])} | {_text(record['adoption_consequence'])} | {_evidence(model, record['evidence_refs'])} |" for record in model['readiness']], "",
        "### Validation-profile status", "| Validation profile | Selection | Result / next evidence |", "|---|---|---|", *[f"| {_text(dict(contract.PROFILE_ROWS)[record['id']])} | " + (f"{_code('selected')} — {_code(record['activation_basis'])}" if record['status'] == 'selected' else _code(record['status'])) + f" | id={_code(record['id'])}; {_evidence(model, record['evidence_refs'])} |" for record in model['profiles']], "",
        "## Pack inventory and content evidence", _narrative(model, "pack-inventory", "No inventory narrative was recorded."), "| Clip | Source | Take | Take name | Role | Loop | Duration | RM speed | Movement owner | Assessment |", "|---|---|---:|---|---|---|---|---|---|---|", *clip_rows, "",
        "## Mechanical baseline", _narrative(model, "mechanical-baseline", "No mechanical-baseline narrative was recorded."), "",
        "## AnimSmith remediation evidence", "| ID | Run | State | Output | Historical output | Evidence |", "|---|---|---|---|---|---|", *[f"| {_code(record['id'])} | {_code(record['run_id'])} | {_text(record['state'])} | {_code(record['output_id'])} | {_code(record['historical_output_id'])} | {_evidence(model, record['input_evidence_refs'] + record['refusal_evidence_refs'])} |" for record in model['remediations']], "",
        "## Engine procedures and evidence", "| Runtime | Version | Procedure | Observed result | Remaining gate |", "|---|---|---|---|---|", *[f"| {_text(record['runtime'])} | {_text(record['version'])} | id={_code(record['id'])}; {_text(record['procedure'])} | {_text(record['level'])}/{_text(record['coverage'])} | {_text(record['settings'])} |" for record in model['engine_evidence']], "",
        "## Rig, masking, and compatibility evidence", "| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |", "|---|---|---|---|---|---|", *cross_rows, "",
        "## Limitations and unknowns", *limitation_rows, "",
        "## Reproduction", _narrative(model, "reproduction", "Use the model digest and source records below."), "", "### V1 authoritative field ledger", _authority_ledger(model, binding), "```json", model_contract.canonical_json({"binding": binding, "model": model}).decode("utf-8"), "```", "",
        "## Sources", *source_rows, "",
    ])
    return RenderedViews(report + "\n", appendix + "\n")


def validate_views(model: dict[str, Any], binding: dict[str, Any], views: RenderedViews, *, report_name: str, appendix_name: str) -> list[str]:
    """Use the pinned AST parser, then prove the fixed projections match V1."""
    errors = model_validator.validate_model(model, binding)
    errors.extend(report_validator.validate(views.report))
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
    if report_validator.rendered_word_count(views.report) > report_validator.MAX_PRIMARY_WORDS:
        errors.append("model-to-view primary report exceeds the word cap")
    return errors


def _checked_paths(model: Path, binding: Path, report: Path, appendix: Path) -> tuple[Path, Path, Path, Path]:
    """Refuse lexical, symlink, and existing hard-link input/output aliases."""
    inputs = (model.resolve(strict=True), binding.resolve(strict=True))
    outputs = tuple(path.resolve(strict=False) for path in (report, appendix))
    if outputs[0] == outputs[1] or (outputs[0].exists() and outputs[1].exists() and os.path.samefile(outputs[0], outputs[1])):
        raise ValueError("report and appendix outputs must be distinct")
    for output in outputs:
        for input_path in inputs:
            if output == input_path or (output.exists() and os.path.samefile(output, input_path)):
                raise ValueError("render output must not alias a model or binding input")
    return inputs[0], inputs[1], outputs[0], outputs[1]


def _stage(path: Path, content: str | bytes) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("wb", dir=path.parent, delete=False) as handle:
        handle.write(content.encode("utf-8") if isinstance(content, str) else content); handle.flush(); os.fsync(handle.fileno())
        return Path(handle.name)


def _backup_path(path: Path) -> Path:
    """Reserve an unused, same-directory backup name without retaining bytes."""
    descriptor, name = tempfile.mkstemp(prefix=".animsmith-render-backup-", dir=path.parent)
    os.close(descriptor)
    backup = Path(name)
    backup.unlink()
    return backup


def _publish_pair(report: Path, appendix: Path, views: RenderedViews) -> None:
    """Publish two staged files, restoring renamed regular-file backups on failure."""
    targets = (report, appendix)
    for target in targets:
        if target.exists() and not stat.S_ISREG(target.stat().st_mode):
            raise ValueError("render output must be a regular file when it already exists")
    staged: tuple[Path, ...] = ()
    backups: list[Path | None] = [None, None]
    published = [False, False]
    remove_backups = True
    try:
        staged = (_stage(report, views.report), _stage(appendix, views.appendix))
        for index, target in enumerate(targets):
            if target.exists():
                backups[index] = _backup_path(target)
                os.replace(target, backups[index])
        for index, target in enumerate(targets):
            os.replace(staged[index], target)
            published[index] = True
    except (OSError, ValueError):
        for index, target in enumerate(targets):
            backup = backups[index]
            try:
                if backup is not None and backup.exists():
                    if target.exists():
                        target.unlink()
                    os.replace(backup, target)
                elif published[index] and target.exists():
                    target.unlink()
            except OSError:
                # Preserve the primary publish refusal; remaining backups stay
                # beside their target for an operator rather than discarding data.
                remove_backups = False
        raise
    finally:
        for temporary in staged:
            if temporary.exists(): temporary.unlink()
        if remove_backups:
            for backup in (backup for backup in backups if backup is not None):
                if backup.exists(): backup.unlink()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path); parser.add_argument("--binding", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True); parser.add_argument("--appendix", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        model_path, binding_path, report_path, appendix_path = _checked_paths(args.model, args.binding, args.report, args.appendix)
        model, _ = model_validator.load_json(model_path); binding, _ = model_validator.load_json(binding_path)
        views = render_views(model, binding, report_name=report_path.name, appendix_name=appendix_path.name)
        errors = validate_views(model, binding, views, report_name=report_path.name, appendix_name=appendix_path.name)
        if errors:
            raise ValueError("; ".join(errors))
        if args.check:
            if not report_path.is_file() or not appendix_path.is_file() or report_path.read_bytes() != views.report.encode() or appendix_path.read_bytes() != views.appendix.encode():
                print("render_evaluation_model.py: generated views are stale", file=sys.stderr); return 1
        else:
            _publish_pair(report_path, appendix_path, views)
    except (OSError, TypeError, ValueError, RecursionError) as error:
        print(f"render_evaluation_model.py: {error}", file=sys.stderr); return 2
    print(f"rendered animation-pack evaluation views: {args.report} + {args.appendix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
