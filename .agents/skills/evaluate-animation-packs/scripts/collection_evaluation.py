"""Shared bounded authority and relationship validation for model V2/V3.

Each revision pins its own immutable schemas and Rust validation handshake.
The capture, relationship checks and canonical writer are shared, not reimplemented.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import re
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
import evaluation_model_v3 as v3_contract
import validate_evaluation_model as v1_validator

_SKILL_ROOT = Path(__file__).parents[1]
_SCHEMA_ROOT = Path(__file__).parents[4] / "docs" / "schemas"

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
    """Build an in-memory collection-output V2 shape for frozen V1 model relations.

    The public V1 validator never sees or accepts V11/V12. The selected validator
    first validates and binds the complete collection envelope; this synthetic unavailable-data shape only
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
            }
            for member in runtime_set["members"]
        ]
        evidence = {
            "root_travel": {"lifecycle": "incomplete", "members_measured": 0},
        }
        runtime_sets.append({
            "id": runtime_set["id"],
            "kind": "sync-group" if runtime_set["kind"] == "gait-group" else runtime_set["kind"],
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
) -> tuple[dict[str, Any], dict[str, Any], dict[str, str]]:
    """Alias the complete relation namespace for frozen V1 without losing identity."""
    model = copy.deepcopy(model)
    binding = copy.deepcopy(binding)
    identifiers = [
        (record, "id")
        for owner, fields in ((model, ("clips", "runtime_sets", "evidence")),
                              (binding, ("clips", "runtime_sets")))
        for field in fields for record in owner[field]
    ]
    for owner, member_key in ((model, "clip_id"), (binding, "id")):
        identifiers.extend(
            (member, member_key)
            for runtime_set in owner["runtime_sets"]
            for member in runtime_set["members"]
        )
    # These are the schema's evidence/clip/set reference positions. Unknown
    # original references must receive distinct aliases too: passing them
    # through could capture a generated alias and fabricate a known fact.
    references = [
        record["evidence_refs"]
        for field in ("runs", "clips", "runtime_sets", "profiles", "pipeline_stages",
                      "readiness", "capabilities", "integration_steps", "issues",
                      "engine_evidence", "limitations", "sources")
        for record in model[field]
    ]
    references.extend(
        record["evidence_refs"]
        for field in ("exclusions", "cross_pack_records")
        for record in model["collection"][field]
    )
    references.extend(
        record[field] for record in model["remediations"]
        for field in ("input_evidence_refs", "refusal_evidence_refs")
    )
    references.extend(record["fact_refs"] for record in model["narratives"])
    references.extend(
        record[field] for record in model["collection"]["constituents"]
        for field in ("clip_ids", "runtime_set_ids")
    )
    originals = {record[key] for record, key in identifiers}
    originals.update(reference for items in references for reference in items)
    aliases = {
        original: f"v2:{index:016x}"
        for index, original in enumerate(sorted(originals))
    }
    # A bijection preserves equality, duplicates and lexical order, including
    # authored IDs which themselves look like private aliases. Never sort the
    # projected rows or reference arrays: malformed input must remain malformed.
    for record, key in identifiers:
        record[key] = aliases[record[key]]
    for items in references:
        items[:] = [aliases[reference] for reference in items]
    return model, binding, {alias: original for original, alias in aliases.items()}



class CollectionEvaluationValidator:
    """One authority pipeline with the two explicitly supported version pins."""

    def __init__(self, version: int) -> None:
        if version == 2:
            self.contract = contract
            self.collection_version = 11
        elif version == 3:
            self.contract = v3_contract
            self.collection_version = 12
        else:
            raise ValueError("unsupported evaluation-model revision")
        self.version = version
        self.handshake = (
            f"animsmith-internal collection-output-valid "
            f"{self.contract.COLLECTION_OUTPUT_SCHEMA} {self.collection_version}\n"
        ).encode("ascii")
        paths = (
            _SKILL_ROOT / "schemas" / f"evaluation-model-v{version}.schema.json",
            _SKILL_ROOT / "schemas" / "evaluation-model-v1.schema.json",
            _SCHEMA_ROOT / f"collection-output-v{self.collection_version}.schema.json",
            _SCHEMA_ROOT / "output-v10.schema.json",
            _SCHEMA_ROOT / "output-v19.schema.json",
            _SCHEMA_ROOT / "measurements-v18.schema.json",
        )
        self.documents = [json.loads(path.read_text(encoding="utf-8")) for path in paths]
        expected_ids = (
            self.contract.SCHEMA, v1_contract.SCHEMA,
            self.contract.COLLECTION_OUTPUT_SCHEMA, "urn:animsmith:schema:output:10",
            self.contract.OUTPUT_SCHEMA, self.contract.MEASUREMENTS_SCHEMA,
        )
        actual_ids = tuple(document.get("$id") for document in self.documents)
        if actual_ids != expected_ids:
            raise ValueError(f"evaluation-model V{version} offline schema registry is stale: {actual_ids!r}")
        registry = Registry().with_resources(
            (document["$id"], Resource.from_contents(document)) for document in self.documents
        )
        self.model_validator = jsonschema.Draft202012Validator(self.documents[0], registry=registry)
        self.collection_validator = jsonschema.Draft202012Validator(self.documents[2], registry=registry)

    def validate_with_animsmith(self, animsmith: Path, binding_bytes: bytes) -> None:
        """Require the selected checkout binary to accept the exact bytes for this revision."""
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
                handshake = stdout.read(len(self.handshake) + 1)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise ValueError(
                f"selected AnimSmith binary could not validate collection-output V{self.collection_version}"
            ) from error
        if result.returncode != 0:
            raise ValueError(
                f"selected AnimSmith binary rejected collection-output V{self.collection_version} "
                f"with exit code {result.returncode}"
            )
        if stdout_size != len(self.handshake) or handshake != self.handshake:
            raise ValueError(
                "selected AnimSmith binary did not return the exact internal "
                f"collection-output V{self.collection_version} validation handshake"
            )

    def load_authoritative_collection_output(
        self, animsmith: Path, binding: Path
    ) -> tuple[Any, bytes]:
        """Strict-read, Rust-validate, and parse one immutable in-memory buffer."""
        raw = _read_collection_output_bytes(binding)
        self.validate_with_animsmith(animsmith, raw)
        return _parse_collection_output(raw), raw

    def validate_model(self, model: Any, binding: Any, binding_bytes: bytes) -> list[str]:
        errors = _schema_errors(self.model_validator, model, "model")
        errors.extend(_schema_errors(self.collection_validator, binding, "binding"))
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
            errors.append(f"model.binding must identify the exact collection-output:{self.collection_version} bytes and manifest")

        projected_sources = [_source_projection(source) for source in binding["sources"]]
        if declared["sources"] != projected_sources:
            errors.append(f"model.binding.sources must retain every typed V{self.collection_version} source state in canonical order")
        if binding["summary"]["incomplete"] and model["presentation"]["completeness"] == "complete":
            errors.append(f"model.presentation.completeness cannot be complete for incomplete V{self.collection_version} evidence")

        bound_clips = {clip["id"]: clip for clip in binding["clips"]}
        model_clips = {clip["id"]: clip for clip in model["clips"]}
        if set(bound_clips) != set(model_clips):
            errors.append(f"model.clips must contain exactly every V{self.collection_version} clip row")
        for clip_id in sorted(set(bound_clips) & set(model_clips)):
            source = bound_clips[clip_id]
            record = model_clips[clip_id]
            if tuple(record[key] for key in ("source", "take_index", "take_name")) != tuple(
                source[key] for key in ("source", "take_index", "take_name")
            ):
                errors.append(f"model.clips[{clip_id}] has a stale V{self.collection_version} source/take witness")
            state = source["binding"]
            if state["state"] == "unavailable":
                if record["assessment"] != "not-evaluated" or record["coverage"] not in {
                    "not-evaluated", "unsupported-input", "unavailable-evidence"
                }:
                    errors.append(f"model.clips[{clip_id}] must retain unavailable V{self.collection_version} evidence as a typed soft failure")
                if record["duration_s"] != {"state": "unavailable"} or record["root_motion_speed_mps"] != {"state": "unavailable"}:
                    errors.append(f"model.clips[{clip_id}] must preserve unavailable V{self.collection_version} measurement availability")
            else:
                if (
                    state["observed_source_take_index"], state["observed_take_name"]
                ) != (source["take_index"], source["take_name"]):
                    errors.append(f"binding.clips[{clip_id}] has a contradictory observed take witness")
                measurements = state["measurements"]
                if record["duration_s"] != {"state": "available", "value": measurements["duration_s"]}:
                    errors.append(f"model.clips[{clip_id}].duration_s must equal V{self.collection_version}")
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
                    errors.append(f"model.clips[{clip_id}].root_motion_speed_mps must preserve exact V{self.collection_version} value and availability")

        bound_sets = {runtime_set["id"]: runtime_set for runtime_set in binding["runtime_sets"]}
        model_sets = {runtime_set["id"]: runtime_set for runtime_set in model["runtime_sets"]}
        if set(bound_sets) != set(model_sets):
            errors.append(f"model.runtime_sets must contain exactly every V{self.collection_version} runtime-set row")
        for set_id in sorted(set(bound_sets) & set(model_sets)):
            source = bound_sets[set_id]
            record = model_sets[set_id]
            if record["kind"] != source["kind"] or [member["clip_id"] for member in record["members"]] != [member["id"] for member in source["members"]]:
                errors.append(f"model.runtime_sets[{set_id}] must preserve V{self.collection_version} kind and member order")
                continue
            for index, member in enumerate(source["members"]):
                if member["resolution"]["state"] == "unavailable" and record["members"][index]["eligibility"] == "complete":
                    errors.append(f"model.runtime_sets[{set_id}].members[{index}] cannot be complete when V{self.collection_version} is unavailable")
            if source["lifecycle"] == "incomplete" and (
                record["assessment"] != "not-evaluated"
                or record["coverage"] not in {"partially-evaluated", "not-evaluated", "unavailable-evidence"}
            ):
                errors.append(f"model.runtime_sets[{set_id}] must retain incomplete V{self.collection_version} evidence as a typed soft failure")

        # Reuse the frozen V1 relationship closure only after version-pinned identity and
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
        relation_model, relation_binding, original_ids = _v1_relation_ids(
            relation_model, relation_binding
        )
        errors.extend(
            re.sub(r"v2:[0-9a-f]{16}", lambda match: original_ids.get(match[0], match[0]), error)
            .replace("model must identify urn:animsmith:skill:animation-pack-evaluation:1 version 1", "model relation projection is invalid")
            for error in v1_validator.validate_model(relation_model, relation_binding)
        )
        return errors

    def main(self) -> int:
        parser = argparse.ArgumentParser(description=f"Validate evaluation model V{self.version} against collection-output V{self.collection_version}.")
        parser.add_argument("model", type=Path)
        parser.add_argument("--binding", type=Path, required=True)
        parser.add_argument("--animsmith", type=Path, required=True)
        parser.add_argument("--check-canonical", action="store_true")
        args = parser.parse_args()
        try:
            model, model_bytes = v1_validator.load_json(args.model)
            binding, binding_bytes = self.load_authoritative_collection_output(
                args.animsmith, args.binding
            )
            errors = self.validate_model(model, binding, binding_bytes)
            if args.check_canonical and model_bytes != self.contract.canonical_json(model):
                errors.append(f"model bytes are not V{self.version} canonical JSON")
        except (OSError, ValueError, TypeError, RecursionError) as error:
            print(f"validate_evaluation_model_v{self.version}.py: {error}", file=sys.stderr)
            return 2
        for error in errors:
            print(f"validate_evaluation_model_v{self.version}.py: {error}", file=sys.stderr)
        if errors:
            return 1
        print(f"validated animation-pack evaluation model V{self.version}: {args.model}")
        return 0
