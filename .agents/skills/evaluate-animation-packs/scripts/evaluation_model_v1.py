"""V1 evaluation-model vocabulary and bounded canonical JSON authority."""

from __future__ import annotations

import hashlib
import json
import math
import re
from typing import Any

from evaluation_contract_v1 import (
    ACTIVATION_BASES, CLASSIFICATION_BASES, CONFIDENCE, COVERAGE_STATES,
    EVALUATION_COMPLETENESS, ISSUE_SEVERITIES, PIPELINE_STAGES,
    PRIMARY_OWNERS, PRIMARY_ROLES, PROFILE_IDS, PROFILE_STATUSES, SET_TYPES,
    TECHNICAL_VERDICTS, VARIANTS,
)

SCHEMA = "urn:animsmith:skill:animation-pack-evaluation:1"
SCHEMA_VERSION = 1
COLLECTION_OUTPUT_SCHEMA = "urn:animsmith:schema:collection-output:2"
IDENTIFIER = re.compile(r"^[a-z0-9][a-z0-9._:-]*$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
MAX_MODEL_BYTES = 8 * 1024 * 1024
MAX_RECORDS = 4096
# n choose 2 pair records must fit the global repeated-record bound: 90 yields
# 4,005 pairs; 91 yields 4,095, and 92 would exceed the 4,096-record bound.
MAX_COLLECTION_CONSTITUENTS = 90
MAX_TEXT_BYTES = 32 * 1024
MAX_DEPTH = 64
CANONICAL_NUMBER_ALGORITHM = "python-binary64-shortest-v1"

# Imported names above remain the canonical skill taxonomies. These are only
# V1 relationship states that the legacy manifest did not need to express.
EVIDENCE_KINDS = frozenset({
    "user-stated", "observed-file", "observed-animsmith", "observed-report",
    "observed-engine", "vendor-stated", "documentation-stated", "inferred",
    "not-evaluated",
})
ASSESSMENT_STATES = frozenset({"pass", "finding", "not-evaluated", "not-applicable"})
LOOP_STATES = frozenset({"loop", "not-loop", "unknown", "not-applicable"})
AVAILABILITY = frozenset({"available", "unavailable", "not-applicable"})
MEMBER_ELIGIBILITY = frozenset({"complete", "incomplete", "quarantined"})
RUN_STATES = frozenset({"current", "historical"})
REMEDIATION_STATES = frozenset({"produced", "refused", "not-run"})
ENGINE_LEVELS = frozenset({"documentation-only", "prototype-observed", "not-evaluated", "deferred"})
READINESS_STATES = frozenset({"ready", "conditional", "poor-fit", "not-applicable", "unknown"})
RECIPE_ACTIONS = frozenset({"topology", "timing", "ownership", "composition", "acceptance-gate"})
COLLECTION_PAIR_RESULTS = frozenset({"direct", "engine-config", "animsmith-current", "artist-required", "incompatible", "unknown"})
NARRATIVE_SLOTS = frozenset({"technical-decision", "fit-and-limitations", "evidence-status", "pack-inventory", "mechanical-baseline", "reproduction"})
READINESS_LANES = frozenset({
    "delivery-completeness", "animsmith-readable-format", "untouched-mechanical-health",
    "declared-clip-semantics", "set-sync-locomotion", "rig-rest-bind-retarget",
    "root-motion-in-place", "target-engine-import-playback", "masks-additive-ik-attachments",
    "performance-runtime-footprint", "game-content-artistic-fit",
    "cross-pack-compatibility", "maintainability-reproducibility",
})


class CanonicalJsonError(ValueError):
    """A value lies outside V1's finite, bounded JSON canonicalization domain."""


def canonical_number(value: int | float) -> str:
    """Encode finite built-in integers/floats in V1's shortest spelling."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise CanonicalJsonError("number must be a built-in integer or finite float")
    if isinstance(value, int):
        return str(value)
    if not math.isfinite(value):
        raise CanonicalJsonError("number must be finite")
    if value == 0:
        return "0"
    mantissa, marker, exponent = repr(value).lower().partition("e")
    if mantissa.endswith(".0"):
        mantissa = mantissa[:-2]
    if not marker:
        return mantissa
    negative = exponent.startswith("-")
    exponent = exponent.lstrip("+-").lstrip("0") or "0"
    return f"{mantissa}e{'-' if negative else ''}{exponent}"


def canonical_json(value: Any) -> bytes:
    """Return bounded, sorted, finite canonical UTF-8 JSON bytes."""
    def encode(item: Any, depth: int) -> str:
        if depth > MAX_DEPTH:
            raise CanonicalJsonError(f"JSON nesting exceeds {MAX_DEPTH}")
        if item is None:
            return "null"
        if item is True:
            return "true"
        if item is False:
            return "false"
        if isinstance(item, str):
            return json.dumps(item, ensure_ascii=False, separators=(",", ":"))
        if isinstance(item, (int, float)) and not isinstance(item, bool):
            return canonical_number(item)
        if isinstance(item, list):
            return "[" + ",".join(encode(child, depth + 1) for child in item) + "]"
        if isinstance(item, dict):
            if not all(isinstance(key, str) for key in item):
                raise CanonicalJsonError("object keys must be strings")
            return "{" + ",".join(
                json.dumps(key, ensure_ascii=False) + ":" + encode(item[key], depth + 1)
                for key in sorted(item)
            ) + "}"
        raise CanonicalJsonError(f"unsupported JSON value: {type(item).__name__}")
    try:
        return encode(value, 0).encode("utf-8")
    except RecursionError as error:
        raise CanonicalJsonError("JSON nesting exceeds implementation limit") from error


def canonical_digest(value: Any) -> str:
    """Hash the canonical model bytes with SHA-256."""
    return hashlib.sha256(canonical_json(value)).hexdigest()
