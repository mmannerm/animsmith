#!/usr/bin/env python3
"""Strictly validate evaluation model V2 against exact collection-output V11."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

import jsonschema
from referencing import Registry, Resource

import evaluation_model_v1 as v1_contract
import evaluation_model_v2 as contract
import validate_evaluation_model as v1_validator


_SKILL_ROOT = Path(__file__).parents[1]
_SCHEMA_ROOT = Path(__file__).parents[4] / "docs" / "schemas"
_MODEL_SCHEMA_PATH = _SKILL_ROOT / "schemas" / "evaluation-model-v2.schema.json"
_MODEL_V1_SCHEMA_PATH = _SKILL_ROOT / "schemas" / "evaluation-model-v1.schema.json"
_VALIDATION_HANDSHAKE = (
    b"animsmith-internal collection-output-valid "
    b"urn:animsmith:schema:collection-output:11 11\n"
)


def _documents() -> list[dict[str, Any]]:
    paths = (
        _MODEL_SCHEMA_PATH,
        _MODEL_V1_SCHEMA_PATH,
        _SCHEMA_ROOT / "collection-output-v11.schema.json",
        _SCHEMA_ROOT / "output-v10.schema.json",
        _SCHEMA_ROOT / "output-v19.schema.json",
        _SCHEMA_ROOT / "measurements-v18.schema.json",
    )
    documents = [json.loads(path.read_text(encoding="utf-8")) for path in paths]
    expected_ids = (
        contract.SCHEMA,
        v1_contract.SCHEMA,
        contract.COLLECTION_OUTPUT_SCHEMA,
        "urn:animsmith:schema:output:10",
        contract.OUTPUT_SCHEMA,
        contract.MEASUREMENTS_SCHEMA,
    )
    actual_ids = tuple(document.get("$id") for document in documents)
    if actual_ids != expected_ids:
        raise ValueError(
            f"evaluation-model V2 offline schema registry is stale: {actual_ids!r}"
        )
    return documents


_DOCUMENTS = _documents()
_REGISTRY = Registry().with_resources(
    (document["$id"], Resource.from_contents(document)) for document in _DOCUMENTS
)
_MODEL_VALIDATOR = jsonschema.Draft202012Validator(_DOCUMENTS[0], registry=_REGISTRY)
_COLLECTION_VALIDATOR = jsonschema.Draft202012Validator(_DOCUMENTS[2], registry=_REGISTRY)


def _schema_errors(validator: jsonschema.Draft202012Validator, value: Any, root: str) -> list[str]:
    errors: list[str] = []
    for error in sorted(validator.iter_errors(value), key=lambda item: tuple(map(str, item.absolute_path))):
        path = root + "".join(
            f"[{part}]" if isinstance(part, int) else f".{part}"
            for part in error.absolute_path
        )
        message = error.message
        if error.validator == "additionalProperties":
            message = f"unknown fields: {message}"
        errors.append(f"{path} schema: {message}")
    return errors


def _constant(value: str) -> None:
    raise ValueError(f"non-finite JSON token: {value}")


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _depth(value: Any, depth: int = 0) -> None:
    if depth > contract.MAX_DEPTH:  # noqa: F405 - imported stable bound.
        raise ValueError(f"JSON nesting exceeds {contract.MAX_DEPTH}")
    if isinstance(value, dict):
        for child in value.values():
            _depth(child, depth + 1)
    elif isinstance(value, list):
        for child in value:
            _depth(child, depth + 1)


def _read_collection_output_bytes(path: Path) -> bytes:
    """Read one no-follow regular file descriptor into one bounded buffer."""
    try:
        before = os.lstat(path)
    except OSError as error:
        raise ValueError(f"collection output is unavailable: {path}") from error
    if not stat.S_ISREG(before.st_mode):
        raise ValueError(f"collection output must be a regular file: {path}")
    flags = os.O_RDONLY
    for name in ("O_BINARY", "O_CLOEXEC", "O_NOFOLLOW", "O_NONBLOCK"):
        flags |= getattr(os, name, 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ValueError(f"collection output cannot be opened without following links: {path}") from error
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode):
            raise ValueError(f"collection output must be a regular file: {path}")
        if (before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino):
            raise ValueError(f"collection output changed while it was opened: {path}")
        if opened.st_size > contract.MAX_COLLECTION_OUTPUT_BYTES:
            raise ValueError(
                f"{path} exceeds the {contract.MAX_COLLECTION_OUTPUT_BYTES}-byte limit"
            )
        chunks: list[bytes] = []
        remaining = contract.MAX_COLLECTION_OUTPUT_BYTES + 1
        while remaining:
            chunk = os.read(descriptor, min(64 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        raw = b"".join(chunks)
    finally:
        os.close(descriptor)
    if len(raw) > contract.MAX_COLLECTION_OUTPUT_BYTES:
        raise ValueError(f"{path} exceeds the {contract.MAX_COLLECTION_OUTPUT_BYTES}-byte limit")
    return raw


def _parse_collection_output(raw: bytes) -> Any:
    """Parse only the bytes already accepted by the authoritative reader."""
    try:
        value = json.loads(raw, object_pairs_hook=_pairs, parse_constant=_constant)
    except (json.JSONDecodeError, RecursionError, UnicodeDecodeError) as error:
        raise ValueError(f"invalid bounded JSON: {error}") from error
    _depth(value)
    return value


def validate_with_animsmith(animsmith: Path, binding_bytes: bytes) -> None:
    """Require the selected checkout binary to accept the exact V11 bytes."""
    try:
        executable = animsmith.resolve(strict=True)
    except OSError as error:
        raise ValueError(f"selected AnimSmith binary is unavailable: {animsmith}") from error
    if not executable.is_file():
        raise ValueError(f"selected AnimSmith binary is not a file: {executable}")
    try:
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            result = subprocess.run(
                [str(executable), "collection", "validate-output"],
                check=False,
                input=binding_bytes,
                stdout=stdout,
                stderr=stderr,
                timeout=30,
            )
            stdout.seek(0, os.SEEK_END)
            stdout_size = stdout.tell()
            stdout.seek(0)
            handshake = stdout.read(len(_VALIDATION_HANDSHAKE) + 1)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ValueError(
            "selected AnimSmith binary could not validate collection-output V11"
        ) from error
    if result.returncode != 0:
        raise ValueError(
            "selected AnimSmith binary rejected collection-output V11 "
            f"with exit code {result.returncode}"
        )
    if stdout_size != len(_VALIDATION_HANDSHAKE) or handshake != _VALIDATION_HANDSHAKE:
        raise ValueError(
            "selected AnimSmith binary did not return the exact internal "
            "collection-output V11 validation handshake"
        )


def load_authoritative_collection_output(
    animsmith: Path, binding: Path
) -> tuple[Any, bytes]:
    """Strict-read, Rust-validate, and parse one immutable in-memory buffer."""
    raw = _read_collection_output_bytes(binding)
    validate_with_animsmith(animsmith, raw)
    return _parse_collection_output(raw), raw


def _source_projection(source: dict[str, Any]) -> dict[str, Any]:
    config = {"state": source["config"]["state"]}
    if "input" in source["config"]:
        config["input"] = copy.deepcopy(source["config"]["input"])
    result = {"state": source["result"]["state"]}
    if "reason" in source["result"]:
        result["reason"] = source["result"]["reason"]
    return {
        "key": source["key"],
        "input": copy.deepcopy(source["input"]),
        "digest": copy.deepcopy(source["digest"]),
        "config": config,
        "loader": copy.deepcopy(source["loader"]),
        "dependency_closure": copy.deepcopy(source["dependency_closure"]),
        "take_inventory": source["take_inventory"],
        "result": result,
    }


def _v1_relation_projection(binding: dict[str, Any]) -> dict[str, Any]:
    """Build an in-memory V2 shape solely to reuse frozen V1 model relations.

    The public V1 validator never sees or accepts V11. V2 first validates and
    binds the complete V11 envelope; this synthetic unavailable-data shape only
    supplies the immutable clip/set identity interface to V1's relationship
    checker, so its evidence/reference/collection rules are not forked.
    """
    unavailable = {"state": "unavailable", "reason": "source_unavailable"}
    clips = [
        {
            "id": clip["id"], "source": clip["source"],
            "take_index": clip["take_index"], "take_name": clip["take_name"],
            "binding": unavailable,
        }
        for clip in binding["clips"]
    ]
    runtime_sets = []
    for runtime_set in binding["runtime_sets"]:
        members = [
            {
                "id": member["id"], "resolution": unavailable,
                "root_travel": {
                    "translation_availability": "unavailable",
                    "speed_mps_availability": "unavailable",
                },
                **(
                    {"gait_phase": {"availability": "unavailable"}}
                    if runtime_set["kind"] == "gait-group" else {}
                ),
            }
            for member in runtime_set["members"]
        ]
        evidence = {
            "root_travel": {"lifecycle": "incomplete", "members_measured": 0},
            **(
                {"gait_phase": {"lifecycle": "incomplete", "members_measured": 0}}
                if runtime_set["kind"] == "gait-group" else {}
            ),
        }
        runtime_sets.append({
            "id": runtime_set["id"], "kind": runtime_set["kind"],
            "members": members, "lifecycle": "incomplete",
            "decision": "not_evaluated", "gaps": ["source_unavailable"],
            "evidence": evidence,
        })
    sources = [
        {
            "key": source["key"], "locator": source["key"],
            "input": {"state": "unavailable", "reason": "missing", "inspected_bytes": 0},
            "digest": {"state": "unpinned"}, "config": {"state": "default"},
            "loader": unavailable, "take_inventory": "unavailable",
            "observed_takes": [], "result": unavailable,
        }
        for source in binding["sources"]
    ]
    return {
        "schema": v1_contract.COLLECTION_OUTPUT_SCHEMA, "schema_version": 2,
        "tool": binding["tool"], "command": "collection lint",
        "manifest": binding["manifest"], "budget": binding["budget"],
        "summary": binding["summary"], "work": binding["work"],
        "sources": sources, "clips": clips, "runtime_sets": runtime_sets,
    }


def _v1_relation_ids(
    model: dict[str, Any], binding: dict[str, Any]
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Losslessly alias slash-separated manifest IDs for frozen V1 relations."""
    aliases = {
        identifier: "v2:" + hashlib.sha256(identifier.encode()).hexdigest()
        for identifier in (
            [clip["id"] for clip in binding["clips"]]
            + [runtime_set["id"] for runtime_set in binding["runtime_sets"]]
        )
    }
    model = copy.deepcopy(model)
    binding = copy.deepcopy(binding)
    for clip in model["clips"]:
        clip["id"] = aliases[clip["id"]]
    for clip in binding["clips"]:
        clip["id"] = aliases[clip["id"]]
    for runtime_set in model["runtime_sets"]:
        runtime_set["id"] = aliases[runtime_set["id"]]
        for member in runtime_set["members"]:
            member["clip_id"] = aliases[member["clip_id"]]
    for runtime_set in binding["runtime_sets"]:
        runtime_set["id"] = aliases[runtime_set["id"]]
        for member in runtime_set["members"]:
            member["id"] = aliases[member["id"]]
    for narrative in model["narratives"]:
        narrative["fact_refs"] = sorted(
            aliases.get(reference, reference) for reference in narrative["fact_refs"]
        )
    for constituent in model["collection"]["constituents"]:
        constituent["clip_ids"] = sorted(
            aliases.get(identifier, identifier) for identifier in constituent["clip_ids"]
        )
        constituent["runtime_set_ids"] = sorted(
            aliases.get(identifier, identifier)
            for identifier in constituent["runtime_set_ids"]
        )
    return model, binding


def validate_model(model: Any, binding: Any, binding_bytes: bytes) -> list[str]:
    errors = _schema_errors(_MODEL_VALIDATOR, model, "model")
    errors.extend(_schema_errors(_COLLECTION_VALIDATOR, binding, "binding"))
    if errors:
        return errors
    declared = model["binding"]
    manifest = binding["manifest"]
    identity = manifest["input"]
    actual_binding = hashlib.sha256(binding_bytes).hexdigest(), len(binding_bytes)
    expected_identity = (
        declared["collection_id"], declared["manifest_sha256"],
        declared["manifest_bytes"], declared["collection_output_sha256"],
        declared["collection_output_bytes"],
    )
    actual_identity = (
        manifest["collection_id"], identity["sha256"], identity["bytes"],
        actual_binding[0], actual_binding[1],
    )
    if expected_identity != actual_identity:
        errors.append("model.binding must identify the exact collection-output:11 bytes and manifest")

    projected_sources = [_source_projection(source) for source in binding["sources"]]
    if declared["sources"] != projected_sources:
        errors.append("model.binding.sources must retain every typed V11 source state in canonical order")
    if binding["summary"]["incomplete"] and model["presentation"]["completeness"] == "complete":
        errors.append("model.presentation.completeness cannot be complete for incomplete V11 evidence")

    bound_clips = {clip["id"]: clip for clip in binding["clips"]}
    model_clips = {clip["id"]: clip for clip in model["clips"]}
    if set(bound_clips) != set(model_clips):
        errors.append("model.clips must contain exactly every V11 clip row")
    for clip_id in sorted(set(bound_clips) & set(model_clips)):
        source = bound_clips[clip_id]
        record = model_clips[clip_id]
        if tuple(record[key] for key in ("source", "take_index", "take_name")) != tuple(
            source[key] for key in ("source", "take_index", "take_name")
        ):
            errors.append(f"model.clips[{clip_id}] has a stale V11 source/take witness")
        state = source["binding"]
        if state["state"] == "unavailable":
            if record["assessment"] != "not-evaluated" or record["coverage"] not in {
                "not-evaluated", "unsupported-input", "unavailable-evidence"
            }:
                errors.append(f"model.clips[{clip_id}] must retain unavailable V11 evidence as a typed soft failure")
            if record["duration_s"] != {"state": "unavailable"} or record["root_motion_speed_mps"] != {"state": "unavailable"}:
                errors.append(f"model.clips[{clip_id}] must preserve unavailable V11 measurement availability")
        else:
            if (
                state["observed_source_take_index"], state["observed_take_name"]
            ) != (source["take_index"], source["take_name"]):
                errors.append(f"binding.clips[{clip_id}] has a contradictory observed take witness")
            measurements = state["measurements"]
            if record["duration_s"] != {"state": "available", "value": measurements["duration_s"]}:
                errors.append(f"model.clips[{clip_id}].duration_s must equal V11")
            speed_availability = measurements["speed_mps_availability"]
            expected_speed = (
                {"state": "available", "value": measurements["speed_mps"]}
                if speed_availability == "measured"
                else {
                    "unavailable": {"state": "unavailable"},
                    "not_applicable": {"state": "not-applicable"},
                }[speed_availability]
            )
            if record["root_motion_speed_mps"] != expected_speed:
                errors.append(f"model.clips[{clip_id}].root_motion_speed_mps must preserve exact V11 value and availability")

    bound_sets = {runtime_set["id"]: runtime_set for runtime_set in binding["runtime_sets"]}
    model_sets = {runtime_set["id"]: runtime_set for runtime_set in model["runtime_sets"]}
    if set(bound_sets) != set(model_sets):
        errors.append("model.runtime_sets must contain exactly every V11 runtime-set row")
    for set_id in sorted(set(bound_sets) & set(model_sets)):
        source = bound_sets[set_id]
        record = model_sets[set_id]
        if record["kind"] != source["kind"] or [member["clip_id"] for member in record["members"]] != [member["id"] for member in source["members"]]:
            errors.append(f"model.runtime_sets[{set_id}] must preserve V11 kind and member order")
            continue
        for index, member in enumerate(source["members"]):
            if member["resolution"]["state"] == "unavailable" and record["members"][index]["eligibility"] == "complete":
                errors.append(f"model.runtime_sets[{set_id}].members[{index}] cannot be complete when V11 is unavailable")
        if source["lifecycle"] == "incomplete" and (
            record["assessment"] != "not-evaluated"
            or record["coverage"] not in {"partially-evaluated", "not-evaluated", "unavailable-evidence"}
        ):
            errors.append(f"model.runtime_sets[{set_id}] must retain incomplete V11 evidence as a typed soft failure")

    # Reuse the frozen V1 relationship closure only after V2/V11 identity and
    # completeness have been checked independently above.
    relation_model = copy.deepcopy(model)
    relation_model["schema"] = v1_contract.SCHEMA
    relation_model["schema_version"] = v1_contract.SCHEMA_VERSION
    relation_model["binding"] = {
        "collection_id": declared["collection_id"],
        "manifest_sha256": declared["manifest_sha256"],
        "manifest_bytes": declared["manifest_bytes"],
    }
    for runtime_set in relation_model["runtime_sets"]:
        if runtime_set["kind"] == "gait-group":
            runtime_set["kind"] = "sync-group"
    relation_binding = _v1_relation_projection(binding)
    for runtime_set in relation_binding["runtime_sets"]:
        if runtime_set["kind"] == "gait-group":
            runtime_set["kind"] = "sync-group"
    relation_model, relation_binding = _v1_relation_ids(
        relation_model, relation_binding
    )
    errors.extend(
        error.replace("model must identify urn:animsmith:skill:animation-pack-evaluation:1 version 1", "model relation projection is invalid")
        for error in v1_validator.validate_model(relation_model, relation_binding)
    )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path)
    parser.add_argument("--binding", type=Path, required=True)
    parser.add_argument("--animsmith", type=Path, required=True)
    parser.add_argument("--check-canonical", action="store_true")
    args = parser.parse_args()
    try:
        model, model_bytes = v1_validator.load_json(args.model)
        binding, binding_bytes = load_authoritative_collection_output(
            args.animsmith, args.binding
        )
        errors = validate_model(model, binding, binding_bytes)
        if args.check_canonical and model_bytes != contract.canonical_json(model):
            errors.append("model bytes are not V2 canonical JSON")
    except (OSError, ValueError, TypeError, RecursionError) as error:
        print(f"validate_evaluation_model_v2.py: {error}", file=sys.stderr)
        return 2
    for error in errors:
        print(f"validate_evaluation_model_v2.py: {error}", file=sys.stderr)
    if errors:
        return 1
    print(f"validated animation-pack evaluation model V2: {args.model}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
