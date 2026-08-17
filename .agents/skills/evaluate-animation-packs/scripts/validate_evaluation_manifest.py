#!/usr/bin/env python3
"""Validate a versioned animation-pack evaluation manifest."""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path
from typing import Any


SCHEMA = "urn:animsmith:skill:animation-pack-evaluation-manifest:1"
TAXONOMY_VERSION = "1"
PROFILE_SET_VERSION = "1"

PRIMARY_ROLES = (
    "idle-pose",
    "continuous-locomotion",
    "locomotion-transition",
    "airborne",
    "traversal",
    "action-interaction",
    "reaction-death",
    "emote-cinematic",
    "other-unknown",
)
VARIANTS = {"in-place", "root-motion", "rotation-only-root", "single", "unknown"}
SET_TYPES = {
    "directional-blend",
    "speed-blend",
    "sync-group",
    "transition-chain",
    "mask-composition",
    "retarget-group",
    "paired-interaction",
    "motion-database",
    "other",
}
CLASSIFICATION_BASES = {"user-required", "vendor-stated", "observed-file", "inferred"}
CONFIDENCE = {"high", "medium", "low"}
PROFILE_IDS = (
    "marketplace-intake",
    "blended-locomotion",
    "root-motion-controller",
    "state-machine-transitions",
    "layered-upper-body-weapons",
    "traversal-environment",
    "contact-actions-interactions",
    "retargeted-customizable-characters",
    "motion-matching-search",
    "networked-movement",
    "runtime-performance",
)
PROFILE_STATUSES = {"selected", "not-selected", "not-applicable"}
ACTIVATION_BASES = {
    "user-required",
    "vendor-intended",
    "observed-pack-capability",
    "evaluator-selected-generic-scenario",
}
PIPELINE_STAGES = (
    "acquire",
    "preserve-raw",
    "inspect",
    "segment",
    "root-motion",
    "conform",
    "validate",
    "optimize",
    "export",
    "gate-report",
)
COVERAGE_STATES = {
    "evaluated-clean",
    "evaluated-finding",
    "partially-evaluated",
    "not-applicable",
    "not-evaluated",
    "unsupported-input",
    "unavailable-evidence",
}
IDENTIFIER = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
TAG = re.compile(r"^[a-z][a-z0-9-]*:[a-z0-9][a-z0-9-]*$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path, help="evaluation manifest JSON")
    return parser.parse_args()


def require_string(value: Any, path: str, errors: list[str]) -> bool:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{path} must be a non-empty string")
        return False
    return True


def require_identifier(value: Any, path: str, errors: list[str]) -> bool:
    if not require_string(value, path, errors):
        return False
    if not IDENTIFIER.fullmatch(value):
        errors.append(f"{path} must be a lowercase hyphenated identifier")
        return False
    return True


def unknown_choices(values: list[Any], allowed: set[str]) -> list[str]:
    return sorted(
        {
            repr(value)
            for value in values
            if not isinstance(value, str) or value not in allowed
        }
    )


def require_complete_id_list(
    entries: Any,
    field: str,
    expected: tuple[str, ...],
    path: str,
    errors: list[str],
) -> dict[str, dict[str, Any]]:
    if not isinstance(entries, list):
        errors.append(f"{path} must be an array")
        return {}
    indexed: dict[str, dict[str, Any]] = {}
    for index, entry in enumerate(entries):
        item_path = f"{path}[{index}]"
        if not isinstance(entry, dict):
            errors.append(f"{item_path} must be an object")
            continue
        identifier = entry.get(field)
        if not isinstance(identifier, str):
            errors.append(f"{item_path}.{field} must be a string")
            continue
        if identifier in indexed:
            errors.append(f"duplicate {field}: {identifier}")
            continue
        indexed[identifier] = entry
    missing = sorted(set(expected) - set(indexed))
    unknown = sorted(set(indexed) - set(expected))
    if missing:
        errors.append(f"{path} is missing: {', '.join(missing)}")
    if unknown:
        errors.append(f"{path} contains unknown identifiers: {', '.join(unknown)}")
    return indexed


def validate_manifest(data: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(data, dict):
        return ["manifest root must be an object"]

    if data.get("schema") != SCHEMA:
        errors.append(f"schema must be {SCHEMA!r}")
    if data.get("taxonomy_version") != TAXONOMY_VERSION:
        errors.append(f"taxonomy_version must be {TAXONOMY_VERSION!r}")
    if data.get("validation_profile_set_version") != PROFILE_SET_VERSION:
        errors.append(f"validation_profile_set_version must be {PROFILE_SET_VERSION!r}")

    evaluator = data.get("evaluator")
    if not isinstance(evaluator, dict):
        errors.append("evaluator must be an object")
    else:
        require_string(evaluator.get("version"), "evaluator.version", errors)
        require_string(evaluator.get("revision"), "evaluator.revision", errors)

    motions = data.get("motions")
    if not isinstance(motions, list):
        errors.append("motions must be an array")
        motions = []

    motion_by_id: dict[str, dict[str, Any]] = {}
    file_to_motion: dict[str, str] = {}
    role_motion_counts: Counter[str] = Counter()
    role_file_counts: Counter[str] = Counter()

    for index, motion in enumerate(motions):
        path = f"motions[{index}]"
        if not isinstance(motion, dict):
            errors.append(f"{path} must be an object")
            continue
        motion_id = motion.get("id")
        if require_identifier(motion_id, f"{path}.id", errors):
            if motion_id in motion_by_id:
                errors.append(f"duplicate motion id: {motion_id}")
            else:
                motion_by_id[motion_id] = motion
        role = motion.get("primary_role")
        if role not in PRIMARY_ROLES:
            errors.append(f"{path}.primary_role has unknown value: {role!r}")
        else:
            role_motion_counts[role] += 1
        require_string(motion.get("vendor_label"), f"{path}.vendor_label", errors)

        tags = motion.get("tags")
        if not isinstance(tags, list):
            errors.append(f"{path}.tags must be an array")
        else:
            string_tags = [tag for tag in tags if isinstance(tag, str)]
            if len(string_tags) != len(set(string_tags)):
                errors.append(f"{path}.tags contains duplicates")
            for tag in tags:
                if not isinstance(tag, str) or not TAG.fullmatch(tag):
                    errors.append(f"{path}.tags contains invalid tag: {tag!r}")

        bases = motion.get("classification_basis")
        if not isinstance(bases, list) or not bases:
            errors.append(f"{path}.classification_basis must be a non-empty array")
        else:
            unknown_bases = unknown_choices(bases, CLASSIFICATION_BASES)
            if unknown_bases:
                errors.append(
                    f"{path}.classification_basis contains unknown values: "
                    + ", ".join(unknown_bases)
                )

        files = motion.get("files")
        if not isinstance(files, list) or not files:
            errors.append(f"{path}.files must be a non-empty array")
            continue
        for file_index, file_entry in enumerate(files):
            file_path = f"{path}.files[{file_index}]"
            if not isinstance(file_entry, dict):
                errors.append(f"{file_path} must be an object")
                continue
            delivered_path = file_entry.get("path")
            if require_string(delivered_path, f"{file_path}.path", errors):
                if delivered_path in file_to_motion:
                    errors.append(
                        f"physical file {delivered_path!r} belongs to more than one motion"
                    )
                elif isinstance(motion_id, str):
                    file_to_motion[delivered_path] = motion_id
            variant = file_entry.get("variant")
            if not isinstance(variant, str) or variant not in VARIANTS:
                errors.append(f"{file_path}.variant has unknown value: {variant!r}")
            if role in PRIMARY_ROLES:
                role_file_counts[role] += 1

    runtime_sets = data.get("runtime_sets")
    if not isinstance(runtime_sets, list):
        errors.append("runtime_sets must be an array")
        runtime_sets = []
    set_ids: set[str] = set()
    for index, runtime_set in enumerate(runtime_sets):
        path = f"runtime_sets[{index}]"
        if not isinstance(runtime_set, dict):
            errors.append(f"{path} must be an object")
            continue
        set_id = runtime_set.get("id")
        if require_identifier(set_id, f"{path}.id", errors):
            if set_id in set_ids:
                errors.append(f"duplicate runtime set id: {set_id}")
            set_ids.add(set_id)
        set_type = runtime_set.get("set_type")
        if not isinstance(set_type, str) or set_type not in SET_TYPES:
            errors.append(f"{path}.set_type has unknown value: {set_type!r}")
        confidence = runtime_set.get("confidence")
        if not isinstance(confidence, str) or confidence not in CONFIDENCE:
            errors.append(f"{path}.confidence has unknown value: {confidence!r}")
        bases = runtime_set.get("classification_basis")
        if not isinstance(bases, list) or not bases:
            errors.append(f"{path}.classification_basis must be a non-empty array")
        else:
            unknown_bases = unknown_choices(bases, CLASSIFICATION_BASES)
            if unknown_bases:
                errors.append(
                    f"{path}.classification_basis contains unknown values: "
                    + ", ".join(unknown_bases)
                )
        members = runtime_set.get("members")
        if not isinstance(members, list) or len(members) < 2:
            errors.append(f"{path}.members must contain at least two members")
            continue
        seen_members: set[tuple[str, str | None]] = set()
        for member_index, member in enumerate(members):
            member_path = f"{path}.members[{member_index}]"
            if not isinstance(member, dict):
                errors.append(f"{member_path} must be an object")
                continue
            member_motion = member.get("motion_id")
            if not isinstance(member_motion, str) or member_motion not in motion_by_id:
                errors.append(f"{member_path}.motion_id references unknown motion")
                continue
            member_file = member.get("file")
            member_file_valid = member_file is None or isinstance(member_file, str)
            if member_file is not None:
                if not isinstance(member_file, str):
                    errors.append(f"{member_path}.file must be a string when present")
                elif file_to_motion.get(member_file) != member_motion:
                    errors.append(
                        f"{member_path}.file is not a delivered file of {member_motion!r}"
                    )
            if not member_file_valid:
                continue
            key = (member_motion, member_file)
            if key in seen_members:
                errors.append(f"{member_path} duplicates another member")
            seen_members.add(key)

    profiles = require_complete_id_list(
        data.get("profiles"), "profile_id", PROFILE_IDS, "profiles", errors
    )
    for profile_id, profile in profiles.items():
        path = f"profiles[{profile_id}]"
        status = profile.get("status")
        if not isinstance(status, str) or status not in PROFILE_STATUSES:
            errors.append(f"{path}.status has unknown value: {status!r}")
        require_string(profile.get("rationale"), f"{path}.rationale", errors)
        activation_basis = profile.get("activation_basis")
        if status == "selected" and (
            not isinstance(activation_basis, str)
            or activation_basis not in ACTIVATION_BASES
        ):
            errors.append(f"{path}.activation_basis is required for selected profiles")
        if status != "selected" and activation_basis is not None:
            errors.append(f"{path}.activation_basis must be omitted unless selected")
    marketplace = profiles.get("marketplace-intake")
    if marketplace is not None and marketplace.get("status") != "selected":
        errors.append("marketplace-intake profile must be selected")

    stages = require_complete_id_list(
        data.get("pipeline_stages"),
        "stage_id",
        PIPELINE_STAGES,
        "pipeline_stages",
        errors,
    )
    for stage_id, stage in stages.items():
        path = f"pipeline_stages[{stage_id}]"
        status = stage.get("status")
        if not isinstance(status, str) or status not in COVERAGE_STATES:
            errors.append(f"{path}.status has unknown value: {status!r}")
        require_string(stage.get("evidence"), f"{path}.evidence", errors)

    expected_role_totals = {
        role: {
            "logical_motions": role_motion_counts[role],
            "delivered_files": role_file_counts[role],
        }
        for role in PRIMARY_ROLES
    }
    if data.get("role_totals") != expected_role_totals:
        errors.append("role_totals does not match totals derived from motions")

    expected_totals = {
        "logical_motions": len(motions),
        "delivered_files": len(file_to_motion),
        "runtime_sets": len(runtime_sets),
    }
    if data.get("totals") != expected_totals:
        errors.append("totals does not match motions, physical files, and runtime sets")

    return errors


def main() -> int:
    args = parse_args()
    try:
        data = json.loads(args.manifest.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        print(f"validate_evaluation_manifest.py: {error}", file=sys.stderr)
        return 2
    errors = validate_manifest(data)
    if errors:
        for error in errors:
            print(f"validate_evaluation_manifest.py: {error}", file=sys.stderr)
        return 1
    print(f"validated animation-pack evaluation manifest: {args.manifest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
