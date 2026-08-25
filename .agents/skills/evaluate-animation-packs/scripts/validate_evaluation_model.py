#!/usr/bin/env python3
"""Strictly validate a bounded V1 evaluation model and V2 binding projection."""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from pathlib import Path
from typing import Any

import jsonschema

from evaluation_contract_v1 import (
    ACTIVATION_BASES, CLASSIFICATION_BASES, CONFIDENCE, COVERAGE_STATES,
    EVALUATION_COMPLETENESS, ISSUE_SEVERITIES, PIPELINE_STAGES,
    PRIMARY_OWNERS, PRIMARY_ROLES, PROFILE_IDS, PROFILE_STATUSES, SET_TYPES,
    TECHNICAL_VERDICTS, VARIANTS,
)
from evaluation_model_v1 import (
    ASSESSMENT_STATES, AVAILABILITY, COLLECTION_OUTPUT_SCHEMA,
    COLLECTION_PAIR_RESULTS, ENGINE_LEVELS, EVIDENCE_KINDS, IDENTIFIER,
    LOOP_STATES, MAX_COLLECTION_CONSTITUENTS, MAX_DEPTH, MAX_MODEL_BYTES,
    MAX_RECORDS, MAX_TEXT_BYTES, MEMBER_ELIGIBILITY, NARRATIVE_SLOTS,
    READINESS_LANES, READINESS_STATES, RECIPE_ACTIONS, REMEDIATION_STATES, RUN_STATES, SCHEMA,
    SCHEMA_VERSION, SHA256, canonical_json,
)


_MODEL_SCHEMA_PATH = Path(__file__).parents[1] / "schemas" / "evaluation-model-v1.schema.json"
_MODEL_SCHEMA = json.loads(_MODEL_SCHEMA_PATH.read_text(encoding="utf-8"))
_MODEL_SCHEMA_VALIDATOR = jsonschema.Draft202012Validator(_MODEL_SCHEMA)


def _schema_errors(model: Any) -> list[str]:
    """Normalize the checked-in schema's structural failures for this CLI."""
    normalized: list[str] = []
    for error in sorted(_MODEL_SCHEMA_VALIDATOR.iter_errors(model), key=lambda item: tuple(map(str, item.absolute_path))):
        path = "model" + "".join(f"[{part}]" if isinstance(part, int) else f".{part}" for part in error.absolute_path)
        message = error.message
        if error.validator == "additionalProperties":
            message = f"unknown fields: {message}"
        normalized.append(f"{path} schema: {message}")
    return normalized


def _constant(value: str) -> None:
    raise ValueError(f"non-finite JSON token: {value}")


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> tuple[Any, bytes]:
    """Read at most N+1 bytes before JSON parsing or a full allocation."""
    with path.open("rb") as handle:
        raw = handle.read(MAX_MODEL_BYTES + 1)
    if len(raw) > MAX_MODEL_BYTES:
        raise ValueError(f"{path} exceeds the {MAX_MODEL_BYTES}-byte limit")
    try:
        value = json.loads(raw, object_pairs_hook=_pairs, parse_constant=_constant)
    except (json.JSONDecodeError, RecursionError, UnicodeDecodeError) as error:
        raise ValueError(f"invalid bounded JSON: {error}") from error
    _depth(value)
    return value, raw


def _depth(value: Any, depth: int = 0) -> None:
    if depth > MAX_DEPTH:
        raise ValueError(f"JSON nesting exceeds {MAX_DEPTH}")
    if isinstance(value, dict):
        for key, child in value.items():
            if not isinstance(key, str):
                raise ValueError("JSON object key is not a string")
            _depth(child, depth + 1)
    elif isinstance(value, list):
        for child in value:
            _depth(child, depth + 1)


def _choice(value: Any, allowed: Any) -> bool:
    return isinstance(value, str) and value in allowed


def _object(value: Any, path: str, errors: list[str], required: set[str], optional: set[str] | None = None) -> dict[str, Any]:
    if not isinstance(value, dict):
        errors.append(f"{path} must be an object")
        return {}
    optional = optional or set()
    unknown, missing = sorted(set(value) - required - optional), sorted(required - set(value))
    if unknown:
        errors.append(f"{path} has unknown fields: {', '.join(unknown)}")
    if missing:
        errors.append(f"{path} is missing fields: {', '.join(missing)}")
    return value


def _text(value: Any, path: str, errors: list[str]) -> bool:
    if not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > MAX_TEXT_BYTES:
        errors.append(f"{path} must be a non-empty string within {MAX_TEXT_BYTES} bytes")
        return False
    return True


def _finite_nonnegative(value: Any, path: str, errors: list[str]) -> bool:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
        errors.append(f"{path} must be a finite non-negative number")
        return False
    return True


def _public_locator(value: Any, path: str, errors: list[str]) -> bool:
    if not _text(value, path, errors):
        return False
    # Evidence is publishable metadata only: repository-relative lowercase
    # Markdown/JSON paths or direct https citations. Absolute,
    # credential-bearing, data, parent-traversal, and arbitrary asset paths
    # could leak a local evaluation workspace.
    https = re.fullmatch(r"https://[^/@\s?#]+(?:[/?#][^@\s]*)?", value)
    relative = re.fullmatch(r"[a-z0-9][a-z0-9._/-]*\.(?:md|json)", value)
    parent_segment = re.search(r"(?:^|/)\.\.(?:/|$)", value)
    if https and not parent_segment:
        return True
    if relative and not value.startswith("/") and "//" not in value and not parent_segment:
        return True
    errors.append(f"{path} must be a public https or repository-relative locator")
    return False


def _plain_narrative(value: Any, path: str, errors: list[str]) -> bool:
    if not _text(value, path, errors):
        return False
    # Narrative may interpret linked facts, but must never introduce another
    # Markdown table/list/heading/HTML block or a status/count/numeric fact.
    forbidden = ("\n", "|", "```", "<", ">", "{", "}", "#", "*", "`", "- ")
    authority = re.compile(
        r"\b\d+(?:\.\d+)?\b|\b(?:pass|finding|ready|conditional|poor-fit|unknown|complete|partial|not-evaluated|"
        r"usable(?: with conditions)?|restricted use|poor fit|insufficient technical evidence|not applicable|"
        r"preview only|preview-only|selected|not selected|evaluated clean|evaluated finding|partially evaluated|"
        r"unsupported input|unavailable evidence|documentation only|prototype observed)\b",
        re.I,
    )
    locator = re.compile(
        r"(?:^|[^A-Za-z0-9._-])/[^\s]|"
        r"(?:^|[^A-Za-z0-9._-])\.\.(?:[\\/])|"
        r"\\\\[^\s\\]+\\|\b[A-Za-z]:[\\/]+|\b(?:file|data):|"
        r"https?://[^/\s@]+@|\b[A-Za-z0-9._-]+/[A-Za-z0-9._/-]*\.[A-Za-z0-9._-]+\b",
        re.I,
    )
    identity = re.compile(r"\b[a-z0-9][a-z0-9._-]*:[a-z0-9._:-]*\b", re.I)
    if any(token in value for token in forbidden) or authority.search(value) or locator.search(value) or identity.search(value):
        errors.append(f"{path} must be plain interpretation, not Markdown or a second fact authority")
        return False
    return True


def _id(value: Any, path: str, errors: list[str]) -> bool:
    if not _text(value, path, errors):
        return False
    if not IDENTIFIER.fullmatch(value):
        errors.append(f"{path} must be a lowercase stable identifier")
        return False
    return True


def _sha(value: Any, path: str, errors: list[str]) -> bool:
    if not isinstance(value, str) or not SHA256.fullmatch(value):
        errors.append(f"{path} must be a lowercase SHA-256 digest")
        return False
    return True


def _array(value: Any, path: str, errors: list[str], maximum: int = MAX_RECORDS) -> list[Any]:
    if not isinstance(value, list):
        errors.append(f"{path} must be an array")
        return []
    if len(value) > maximum:
        errors.append(f"{path} exceeds its {maximum}-record bound")
    return value


def _id_records(value: Any, path: str, errors: list[str], required: set[str], optional: set[str] | None = None) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    records: list[dict[str, Any]] = []
    indexed: dict[str, dict[str, Any]] = {}
    ids: list[str] = []
    for index, raw in enumerate(_array(value, path, errors)):
        item = _object(raw, f"{path}[{index}]", errors, required, optional)
        identifier = item.get("id")
        if _id(identifier, f"{path}[{index}].id", errors):
            ids.append(identifier)
            if identifier in indexed:
                errors.append(f"{path} has duplicate id: {identifier}")
            indexed[identifier] = item
        records.append(item)
    if ids != sorted(ids):
        errors.append(f"{path} must be sorted by id")
    return records, indexed


def _refs(value: Any, path: str, known: set[str], errors: list[str], *, check_dangling: bool = True) -> None:
    items = _array(value, path, errors)
    if not all(_id(item, f"{path}[]", errors) for item in items):
        return
    if items != sorted(items) or len(items) != len(set(items)):
        errors.append(f"{path} must be uniquely sorted")
    if check_dangling:
        dangling = sorted(set(items) - known)
        if dangling:
            errors.append(f"{path} has dangling references: {', '.join(dangling)}")


def _binding(binding: Any, errors: list[str]) -> tuple[dict[str, Any], dict[str, tuple[Any, ...]], dict[str, tuple[str, list[str]]], dict[str, dict[str, Any]], bool]:
    # This is a projection consumer, not a reimplementation of the TOML parser.
    root = _object(binding, "binding", errors, {"schema", "command", "manifest", "clips", "runtime_sets", "sources"}, {"schema_version", "tool", "budget", "summary", "work"})
    if root.get("schema") != COLLECTION_OUTPUT_SCHEMA or root.get("command") != "collection lint":
        errors.append("binding must be an independently validated collection-output:2 lint envelope")
    manifest = _object(root.get("manifest"), "binding.manifest", errors, {"schema", "schema_version", "collection_id", "input"})
    _id(manifest.get("collection_id"), "binding.manifest.collection_id", errors)
    identity = _object(manifest.get("input"), "binding.manifest.input", errors, {"sha256", "bytes"})
    _sha(identity.get("sha256"), "binding.manifest.input.sha256", errors)
    if not isinstance(identity.get("bytes"), int) or isinstance(identity.get("bytes"), bool) or identity.get("bytes", -1) < 0:
        errors.append("binding.manifest.input.bytes must be a non-negative integer")
    clips: dict[str, tuple[Any, ...]] = {}
    sources: dict[str, dict[str, Any]] = {}
    for index, raw in enumerate(_array(root.get("sources", []), "binding.sources", errors)):
        if not isinstance(raw, dict):
            errors.append(f"binding.sources[{index}] must be an object")
            continue
        source = raw
        if "key" not in source:
            errors.append(f"binding.sources[{index}] is missing fields: key")
            continue
        key = source.get("key")
        if not _id(key, f"binding.sources[{index}].key", errors):
            continue
        if key in sources:
            errors.append(f"binding.sources has duplicate key: {key}")
            continue
        sources[key] = source
    for index, raw in enumerate(_array(root.get("clips"), "binding.clips", errors)):
        item = _object(raw, f"binding.clips[{index}]", errors, {"id", "source", "take_index", "take_name"}, {"binding"})
        if all(key in item for key in ("id", "source", "take_index", "take_name")):
            valid = _id(item["id"], f"binding.clips[{index}].id", errors)
            valid = _id(item["source"], f"binding.clips[{index}].source", errors) and valid
            if not isinstance(item["take_index"], int) or isinstance(item["take_index"], bool) or item["take_index"] < 0:
                errors.append(f"binding.clips[{index}].take_index must be a non-negative integer")
                valid = False
            valid = _text(item["take_name"], f"binding.clips[{index}].take_name", errors) and valid
            if not valid:
                continue
            if item["id"] in clips:
                errors.append(f"binding.clips has duplicate id: {item['id']}")
                continue
            clips[item["id"]] = (item["source"], item["take_index"], item["take_name"])
    sets: dict[str, tuple[str, list[str]]] = {}
    for index, raw in enumerate(_array(root.get("runtime_sets"), "binding.runtime_sets", errors)):
        item = _object(raw, f"binding.runtime_sets[{index}]", errors, {"id", "kind", "members"})
        members = _array(item.get("members"), f"binding.runtime_sets[{index}].members", errors)
        member_ids = [member.get("id") for member in members if isinstance(member, dict)]
        if isinstance(item.get("id"), str) and isinstance(item.get("kind"), str):
            if item["id"] in sets:
                errors.append(f"binding.runtime_sets has duplicate id: {item['id']}")
            else:
                sets[item["id"]] = (item["kind"], member_ids)
    return manifest, clips, sets, sources, "sources" in root


def validate_model(model: Any, binding: Any) -> list[str]:
    errors: list[str] = []
    manifest, bound_clips, bound_sets, bound_sources, has_source_projection = _binding(binding, errors)
    # JSON Schema is the sole structural authority. The remainder of this
    # function closes only cross-record and binding-derived relationships.
    errors.extend(_schema_errors(model))
    if errors:
        return errors
    top = {
        "schema", "schema_version", "binding", "presentation", "evidence", "runs", "clips", "runtime_sets", "profiles", "pipeline_stages", "readiness", "capabilities", "integration_steps", "issues", "remediations", "engine_evidence", "limitations", "sources", "narratives", "collection",
    }
    root = _object(model, "model", errors, top)
    if root.get("schema") != SCHEMA or root.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"model must identify {SCHEMA} version {SCHEMA_VERSION}")
    declared = _object(root.get("binding"), "model.binding", errors, {"collection_id", "manifest_sha256", "manifest_bytes"})
    input_identity = _object(manifest.get("input"), "binding.manifest.input", errors, {"sha256", "bytes"})
    if (declared.get("collection_id"), declared.get("manifest_sha256"), declared.get("manifest_bytes")) != (manifest.get("collection_id"), input_identity.get("sha256"), input_identity.get("bytes")):
        errors.append("model.binding is stale against the validated collection projection")
    _id(declared.get("collection_id"), "model.binding.collection_id", errors)
    _sha(declared.get("manifest_sha256"), "model.binding.manifest_sha256", errors)
    if not isinstance(declared.get("manifest_bytes"), int) or isinstance(declared.get("manifest_bytes"), bool) or declared.get("manifest_bytes", -1) < 0:
        errors.append("model.binding.manifest_bytes must be a non-negative integer")
    presentation = _object(root.get("presentation"), "model.presentation", errors, {"id", "title", "verdict", "completeness", "confidence"})
    _id(presentation.get("id"), "model.presentation.id", errors)
    _text(presentation.get("title"), "model.presentation.title", errors)
    if not _choice(presentation.get("verdict"), TECHNICAL_VERDICTS): errors.append("model.presentation.verdict has an unknown token")
    if not _choice(presentation.get("completeness"), EVALUATION_COMPLETENESS): errors.append("model.presentation.completeness has an unknown token")
    if not _choice(presentation.get("confidence"), CONFIDENCE): errors.append("model.presentation.confidence has an unknown token")

    evidence, evidence_by_id = _id_records(root.get("evidence"), "model.evidence", errors, {"id", "kind", "locator", "summary"})
    for index, item in enumerate(evidence):
        if not _choice(item.get("kind"), EVIDENCE_KINDS): errors.append(f"model.evidence[{index}].kind has an unknown token")
        _public_locator(item.get("locator"), f"model.evidence[{index}].locator", errors)
        _text(item.get("summary"), f"model.evidence[{index}].summary", errors)

    runs, run_by_id = _id_records(root.get("runs"), "model.runs", errors, {"id", "state", "evidence_refs", "summary"}, {"supersedes"})
    current = []
    for index, item in enumerate(runs):
        if not _choice(item.get("state"), RUN_STATES): errors.append(f"model.runs[{index}].state has an unknown token")
        _refs(item.get("evidence_refs"), f"model.runs[{index}].evidence_refs", set(evidence_by_id), errors)
        _text(item.get("summary"), f"model.runs[{index}].summary", errors)
        current.extend([item] if item.get("state") == "current" else [])
    if len(current) != 1: errors.append("model.runs must contain exactly one current run")
    for index, item in enumerate(runs):
        supersedes = item.get("supersedes")
        if "supersedes" in item and (not _id(supersedes, f"model.runs[{index}].supersedes", errors) or item.get("state") != "current" or supersedes not in run_by_id or run_by_id[supersedes].get("state") != "historical"):
            errors.append(f"model.runs[{index}].supersedes must target a historical run")

    clips, clip_by_id = _id_records(root.get("clips"), "model.clips", errors, {"id", "source", "take_index", "take_name", "primary_role", "tags", "classification_basis", "evidence_refs", "loop", "duration_s", "root_motion_speed_mps", "movement_owner", "assessment", "coverage"})
    if set(clip_by_id) != set(bound_clips): errors.append("model.clips must contain exactly the binding clip IDs")
    for index, item in enumerate(clips):
        clip_id = item.get("id")
        bound = bound_clips.get(clip_id) if isinstance(clip_id, str) else None
        if tuple(item.get(key) for key in ("source", "take_index", "take_name")) != bound: errors.append(f"model.clips[{index}] has a stale source/take witness")
        _id(item.get("source"), f"model.clips[{index}].source", errors)
        if not isinstance(item.get("take_index"), int) or isinstance(item.get("take_index"), bool) or item.get("take_index", -1) < 0:
            errors.append(f"model.clips[{index}].take_index must be a non-negative integer")
        _text(item.get("take_name"), f"model.clips[{index}].take_name", errors)
        if not _choice(item.get("primary_role"), PRIMARY_ROLES): errors.append(f"model.clips[{index}].primary_role has an unknown token")
        # Tags are closed, self-contained labels: validate their ID spelling,
        # order, and uniqueness without building a set from malformed input.
        _refs(item.get("tags"), f"model.clips[{index}].tags", set(), errors, check_dangling=False)
        bases = _array(item.get("classification_basis"), f"model.clips[{index}].classification_basis", errors)
        if not all(_choice(value, CLASSIFICATION_BASES) for value in bases): errors.append(f"model.clips[{index}].classification_basis has an unknown token")
        if all(isinstance(value, str) for value in bases) and (bases != sorted(bases) or len(bases) != len(set(bases))):
            errors.append(f"model.clips[{index}].classification_basis must be uniquely sorted")
        _refs(item.get("evidence_refs"), f"model.clips[{index}].evidence_refs", set(evidence_by_id), errors)
        if not _choice(item.get("loop"), LOOP_STATES) or not _choice(item.get("assessment"), ASSESSMENT_STATES) or not _choice(item.get("coverage"), COVERAGE_STATES) or not _choice(item.get("movement_owner"), PRIMARY_OWNERS): errors.append(f"model.clips[{index}] has an unknown lifecycle token")
        for field in ("duration_s", "root_motion_speed_mps"):
            state = _object(item.get(field), f"model.clips[{index}].{field}", errors, {"state"}, {"value"})
            if not _choice(state.get("state"), AVAILABILITY): errors.append(f"model.clips[{index}].{field}.state has an unknown token")
            if state.get("state") == "available": _finite_nonnegative(state.get("value"), f"model.clips[{index}].{field}.value", errors)
            if state.get("state") != "available" and "value" in state: errors.append(f"model.clips[{index}].{field} may only carry value when available")

    runtime_sets, set_by_id = _id_records(root.get("runtime_sets"), "model.runtime_sets", errors, {"id", "kind", "members", "assessment", "coverage", "evidence_refs"})
    if set(set_by_id) != set(bound_sets): errors.append("model.runtime_sets must contain exactly the binding runtime-set IDs")
    for index, item in enumerate(runtime_sets):
        set_id = item.get("id")
        bound = bound_sets.get(set_id) if isinstance(set_id, str) else None
        if bound and (item.get("kind") != bound[0] or [member.get("clip_id") for member in _array(item.get("members"), f"model.runtime_sets[{index}].members", errors) if isinstance(member, dict)] != bound[1]): errors.append(f"model.runtime_sets[{index}] must preserve binding kind and member order")
        if not _choice(item.get("kind"), SET_TYPES) or not _choice(item.get("assessment"), ASSESSMENT_STATES) or not _choice(item.get("coverage"), COVERAGE_STATES): errors.append(f"model.runtime_sets[{index}] has an unknown token")
        for member_index, member in enumerate(_array(item.get("members"), f"model.runtime_sets[{index}].members", errors)):
            entry = _object(member, f"model.runtime_sets[{index}].members[{member_index}]", errors, {"clip_id", "eligibility"})
            if entry.get("clip_id") not in clip_by_id or not _choice(entry.get("eligibility"), MEMBER_ELIGIBILITY): errors.append(f"model.runtime_sets[{index}].members[{member_index}] is dangling or invalid")
        _refs(item.get("evidence_refs"), f"model.runtime_sets[{index}].evidence_refs", set(evidence_by_id), errors)

    profiles, profile_by_id = _id_records(root.get("profiles"), "model.profiles", errors, {"id", "status", "activation_basis", "evidence_refs"})
    if set(profile_by_id) != set(PROFILE_IDS): errors.append("model.profiles must contain every canonical validation profile")
    for index, item in enumerate(profiles):
        if not _choice(item.get("status"), PROFILE_STATUSES) or not _choice(item.get("activation_basis"), ACTIVATION_BASES): errors.append(f"model.profiles[{index}] has an unknown token")
        _refs(item.get("evidence_refs"), f"model.profiles[{index}].evidence_refs", set(evidence_by_id), errors)
    stages, stage_by_id = _id_records(root.get("pipeline_stages"), "model.pipeline_stages", errors, {"id", "coverage", "evidence_refs"})
    if set(stage_by_id) != set(PIPELINE_STAGES): errors.append("model.pipeline_stages must contain all ten canonical stages")
    for index, item in enumerate(stages):
        if not _choice(item.get("coverage"), COVERAGE_STATES): errors.append(f"model.pipeline_stages[{index}].coverage has an unknown token")
        _refs(item.get("evidence_refs"), f"model.pipeline_stages[{index}].evidence_refs", set(evidence_by_id), errors)

    # All remaining repeated claim records carry evidence refs; their domain
    # specific relationships are checked below rather than reduced to prose.
    for field, required in (("readiness", {"id", "state", "adoption_consequence", "evidence_refs"}), ("capabilities", {"id", "state", "evidence_refs"}), ("integration_steps", {"id", "order", "action", "movement_owner", "phase_owner", "coordinates_or_thresholds", "evidence_refs"}), ("issues", {"id", "severity", "impact", "primary_owner", "current_action", "future_candidate", "secondary_workaround", "evidence_refs"}), ("engine_evidence", {"id", "runtime", "version", "level", "coverage", "settings", "procedure", "evidence_refs"}), ("limitations", {"id", "summary", "evidence_refs"}), ("sources", {"id", "source_commit", "report_sha256", "acquisition_scope", "license_scope", "evidence_kind", "evidence_refs"}), ("narratives", {"id", "slot", "text", "fact_refs"})):
        records, indexed = _id_records(root.get(field), f"model.{field}", errors, required)
        if field == "narratives":
            slots = [record.get("slot") for record in records]
            if all(isinstance(slot, str) for slot in slots) and len(slots) != len(set(slots)): errors.append("model.narratives has duplicate fixed slots")
        if field == "integration_steps":
            orders = [record.get("order") for record in records]
            if len(records) > 5:
                errors.append("model.integration_steps exceeds its 5-record bound")
            if orders != list(range(1, len(records) + 1)):
                errors.append("model.integration_steps must use unique contiguous order starting at one")
        if field == "readiness" and set(indexed) != set(READINESS_LANES):
            errors.append("model.readiness must contain every canonical readiness lane")
        for index, item in enumerate(records):
            if "evidence_refs" in item: _refs(item["evidence_refs"], f"model.{field}[{index}].evidence_refs", set(evidence_by_id), errors)
            if field == "readiness":
                if not _choice(item.get("state"), READINESS_STATES) or item.get("id") not in READINESS_LANES: errors.append(f"model.readiness[{index}] has an unknown lane or state")
                _text(item.get("adoption_consequence"), f"model.readiness[{index}].adoption_consequence", errors)
            if field == "capabilities" and not _choice(item.get("state"), ASSESSMENT_STATES): errors.append(f"model.capabilities[{index}].state has an unknown token")
            if field == "integration_steps":
                if not _choice(item.get("action"), RECIPE_ACTIONS) or not isinstance(item.get("order"), int) or isinstance(item.get("order"), bool) or item.get("order", 0) < 1 or not _choice(item.get("movement_owner"), PRIMARY_OWNERS) or not _choice(item.get("phase_owner"), PRIMARY_OWNERS): errors.append(f"model.integration_steps[{index}] has an invalid typed recipe field")
                _text(item.get("coordinates_or_thresholds"), f"model.integration_steps[{index}].coordinates_or_thresholds", errors)
            if field == "issues":
                if not _choice(item.get("severity"), ISSUE_SEVERITIES) or not _choice(item.get("primary_owner"), PRIMARY_OWNERS): errors.append(f"model.issues[{index}] has an unknown ownership token")
                for name in ("impact", "current_action", "future_candidate", "secondary_workaround"): _text(item.get(name), f"model.issues[{index}].{name}", errors)
            if field == "engine_evidence":
                if not _choice(item.get("level"), ENGINE_LEVELS) or not _choice(item.get("coverage"), COVERAGE_STATES): errors.append(f"model.engine_evidence[{index}] has an unknown token")
                for name in ("runtime", "version", "settings", "procedure"): _text(item.get(name), f"model.engine_evidence[{index}].{name}", errors)
            if field == "sources":
                if not _sha(item.get("report_sha256"), f"model.sources[{index}].report_sha256", errors) or not _choice(item.get("evidence_kind"), EVIDENCE_KINDS): errors.append(f"model.sources[{index}] has an invalid provenance token")
                for name in ("source_commit", "acquisition_scope", "license_scope"): _text(item.get(name), f"model.sources[{index}].{name}", errors)
            if field == "limitations": _text(item.get("summary"), f"model.limitations[{index}].summary", errors)
            if field == "narratives":
                if not _choice(item.get("slot"), NARRATIVE_SLOTS): errors.append(f"model.narratives[{index}].slot has an unknown token")
                _plain_narrative(item.get("text"), f"model.narratives[{index}].text", errors)
                _refs(item.get("fact_refs"), f"model.narratives[{index}].fact_refs", set(evidence_by_id) | set(clip_by_id) | set(set_by_id), errors)

    remediations, remediation_by_id = _id_records(root.get("remediations"), "model.remediations", errors, {"id", "run_id", "state", "input_evidence_refs", "output_id", "refusal_evidence_refs", "historical_output_id"})
    outputs: dict[str, str] = {}
    for index, item in enumerate(remediations):
        run_id = item.get("run_id")
        if not _id(run_id, f"model.remediations[{index}].run_id", errors) or run_id not in run_by_id or not _choice(item.get("state"), REMEDIATION_STATES): errors.append(f"model.remediations[{index}] has a dangling run or state")
        _id(item.get("output_id"), f"model.remediations[{index}].output_id", errors)
        _id(item.get("historical_output_id"), f"model.remediations[{index}].historical_output_id", errors)
        _refs(item.get("input_evidence_refs"), f"model.remediations[{index}].input_evidence_refs", set(evidence_by_id), errors)
        _refs(item.get("refusal_evidence_refs"), f"model.remediations[{index}].refusal_evidence_refs", set(evidence_by_id), errors)
        state = item.get("state")
        if state == "produced":
            if item.get("output_id") == "none" or item.get("historical_output_id") != "none" or item.get("refusal_evidence_refs"):
                errors.append(f"model.remediations[{index}] has invalid produced-state fields")
            output_id = item.get("output_id")
            if isinstance(output_id, str):
                if output_id in outputs: errors.append(f"model.remediations has duplicate produced output_id: {output_id}")
                elif isinstance(run_id, str): outputs[output_id] = run_id
        elif state == "refused":
            if item.get("output_id") != "none" or item.get("historical_output_id") == "none" or not item.get("refusal_evidence_refs"):
                errors.append(f"model.remediations[{index}] has invalid refused-state fields")
        elif state == "not-run":
            if item.get("output_id") != "none" or item.get("historical_output_id") != "none" or item.get("refusal_evidence_refs"):
                errors.append(f"model.remediations[{index}] has invalid not-run-state fields")
    for index, item in enumerate(remediations):
        historical_output = item.get("historical_output_id")
        produced_run = outputs.get(historical_output) if isinstance(historical_output, str) else None
        if item.get("state") == "refused" and (produced_run is None or run_by_id.get(produced_run, {}).get("state") != "historical"):
            errors.append(f"model.remediations[{index}].historical_output_id must name an output produced by a historical run")

    collection = _object(root.get("collection"), "model.collection", errors, {"constituents", "exclusions", "cross_pack_records"})
    constituents, constituent_by_id = _id_records(collection.get("constituents"), "model.collection.constituents", errors, {"id", "model_sha256", "clip_ids", "source_file_count", "runtime_set_ids"})
    if len(constituents) > MAX_COLLECTION_CONSTITUENTS: errors.append("model.collection.constituents exceeds configured N")
    exclusions, exclusion_by_id = _id_records(collection.get("exclusions"), "model.collection.exclusions", errors, {"id", "reason", "evidence_refs"})
    if set(constituent_by_id) & set(exclusion_by_id): errors.append("model.collection constituents and exclusions must reconcile disjointly")
    if constituents:
        declared_clips: list[str] = []
        declared_sets: list[str] = []
        source_files = 0
        for index, item in enumerate(constituents):
            _sha(item.get("model_sha256"), f"model.collection.constituents[{index}].model_sha256", errors)
            _refs(item.get("clip_ids"), f"model.collection.constituents[{index}].clip_ids", set(bound_clips), errors)
            _refs(item.get("runtime_set_ids"), f"model.collection.constituents[{index}].runtime_set_ids", set(bound_sets), errors)
            declared_clips.extend(item.get("clip_ids", [])); declared_sets.extend(item.get("runtime_set_ids", []))
            if not isinstance(item.get("source_file_count"), int) or isinstance(item.get("source_file_count"), bool) or item.get("source_file_count", -1) < 0:
                errors.append(f"model.collection.constituents[{index}].source_file_count must be a non-negative integer")
            else: source_files += item["source_file_count"]
        if not all(isinstance(clip_id, str) for clip_id in declared_clips) or set(declared_clips) != set(bound_clips) or len(declared_clips) != len(set(declared_clips)):
            errors.append("model.collection constituent clip refs must derive the exact binding logical-clip total")
        if not all(isinstance(set_id, str) for set_id in declared_sets) or set(declared_sets) != set(bound_sets) or len(declared_sets) != len(set(declared_sets)):
            errors.append("model.collection constituent runtime-set refs must derive the exact binding runtime-set total")
        if has_source_projection and source_files != len(bound_sources):
            errors.append("model.collection source_file_count total must derive the validated binding source-file total")
    for index, item in enumerate(exclusions):
        _text(item.get("reason"), f"model.collection.exclusions[{index}].reason", errors)
        _refs(item.get("evidence_refs"), f"model.collection.exclusions[{index}].evidence_refs", set(evidence_by_id), errors)
    pairs, pair_by_id = _id_records(collection.get("cross_pack_records"), "model.collection.cross_pack_records", errors, {"id", "left", "right", "result", "evidence_refs"})
    ordered = list(constituent_by_id)
    expected = {(left, right) for position, left in enumerate(ordered) for right in ordered[position + 1:]}
    actual: set[tuple[Any, Any]] = set()
    for index, item in enumerate(pairs):
        left, right = item.get("left"), item.get("right")
        if not _id(left, f"model.collection.cross_pack_records[{index}].left", errors) or not _id(right, f"model.collection.cross_pack_records[{index}].right", errors):
            errors.append(f"model.collection.cross_pack_records[{index}] has invalid endpoints or result")
            continue
        pair = (left, right); actual.add(pair)
        if pair not in expected or not _choice(item.get("result"), COLLECTION_PAIR_RESULTS): errors.append(f"model.collection.cross_pack_records[{index}] has invalid endpoints or result")
        _refs(item.get("evidence_refs"), f"model.collection.cross_pack_records[{index}].evidence_refs", set(evidence_by_id), errors)
    if actual != expected or len(pairs) != len(expected): errors.append("model.collection.cross_pack_records must contain exactly one derived unordered pair per constituent tuple")
    try: canonical_json(model)
    except (ValueError, TypeError, RecursionError) as error: errors.append(f"model is outside the canonical JSON domain: {error}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument("model", type=Path); parser.add_argument("--binding", type=Path, required=True); parser.add_argument("--check-canonical", action="store_true"); args = parser.parse_args()
    try:
        model, model_bytes = load_json(args.model); binding, _ = load_json(args.binding)
        errors = validate_model(model, binding)
        if args.check_canonical and model_bytes != canonical_json(model): errors.append("model bytes are not V1 canonical JSON")
    except (OSError, ValueError, TypeError, RecursionError) as error:
        print(f"validate_evaluation_model.py: {error}", file=sys.stderr); return 2
    for error in errors: print(f"validate_evaluation_model.py: {error}", file=sys.stderr)
    if errors: return 1
    print(f"validated animation-pack evaluation model: {args.model}"); return 0


if __name__ == "__main__": raise SystemExit(main())
