#!/usr/bin/env python3
"""Validate animation-pack report pairs from a parsed CommonMark/GFM AST."""

from __future__ import annotations

import argparse
import functools
import json
import math
import os
import re
import subprocess
import sys
from datetime import date
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

from evaluation_contract_v1 import (
    ACTIVATION_BASES,
    COVERAGE_STATES,
    CONFIDENCE,
    EVALUATION_COMPLETENESS,
    ISSUE_SEVERITIES,
    PIPELINE_STAGE_ROWS,
    PRIMARY_OWNERS,
    PRIMARY_ROLES,
    PROFILE_ROWS,
    SCHEMA,
    SET_TYPES,
    TECHNICAL_VERDICTS,
    VARIANTS,
)


PRIMARY_HEADINGS = (
    "Technical decision", "Capability coverage", "Runtime sets and authored motion",
    "Integration recipe", "Technical issue register", "Engine status",
    "Fit and limitations", "Evidence status", "Sources",
)
CAPABILITY_HEADINGS = ("Complete core", "Partial supporting gameplay", "Absent")
V1_CAPABILITY_HEADINGS = CAPABILITY_HEADINGS + ("Not evaluated", "Not applicable")
APPENDIX_HEADINGS = (
    "Evaluation scope and provenance", "Evaluation manifest and taxonomy",
    "Pack inventory and content evidence", "Mechanical baseline",
    "AnimSmith remediation evidence", "Engine procedures and evidence",
    "Rig, masking, and compatibility evidence", "Limitations and unknowns",
    "Reproduction", "Sources",
)
APPENDIX_MANIFEST_HEADINGS = (
    "Canonical clip-role inventory", "Runtime-set inventory",
    "Pipeline-stage coverage", "Readiness evidence by clip set",
    "Validation-profile status",
)
PIPELINE_STAGE_LABELS = tuple(label for _identifier, label in PIPELINE_STAGE_ROWS)
PROFILE_LABELS = tuple(label for _identifier, label in PROFILE_ROWS)
ENGINE_LABELS = ("Unity", "Unreal Engine", "Godot", "Bevy")
MAX_PRIMARY_WORDS = 2000
REPORT_FORMAT_VERSION = "1"
REPORT_TITLE_PREFIX = "Animation pack evaluation: "
APPENDIX_TITLE_PREFIX = "Animation pack evidence appendix: "
RUNTIME_SET_HEADER = (
    "Set/profile", "Role or coordinate", "Exact members", "Variant/type",
    "Timing or motion", "Runtime contract",
)
RECIPE_LABELS = (
    "Members/topology", "Timing/synchronization", "State ownership",
    "Composition constraints", "Acceptance gate",
)
RECIPE_KEYS = (
    ("topology",), ("timing", "loop", "sync", "transition", "contact"),
    ("owner",), ("composition",), ("gate",),
)
ISSUE_HEADER = (
    "ID", "Severity", "Problem and impact", "Primary owner", "Current action",
    "Future AnimSmith potential", "Evidence/status",
)
ENGINE_HEADER = ("Runtime", "Evidence level", "Technical result", "Remaining gate")
ROLE_HEADER = (
    "Canonical primary role", "Logical motions", "Delivered files", "Evidence boundary",
)
V1_ROLE_HEADER = (
    "Canonical primary role", "Logical motions", "Unique source files used by role", "Evidence boundary",
)
PIPELINE_HEADER = ("Stage", "Coverage state", "Evidence / remaining gate")
PROFILE_HEADER = ("Validation profile", "Selection", "Result / next evidence")
APPENDIX_RUNTIME_HEADER = (
    "Runtime set", "Type", "Members/variants", "Grouping evidence", "Validation status",
)
REPORT_VARIANTS = set(VARIANTS) | {"paired-ip-rm"}
TIMING_UNITS = {
    "duration": "s", "rm_speed": "m/s", "sample_rate": "Hz",
    "frames": "frames", "threshold": None,
}
CONTRACT_KEYS = {
    "loop", "loop_ip", "loop_rm", "sync", "transition", "mask", "additive",
    "contact", "interaction", "movement", "state", "database", "playback",
}
LOOP_VALUES = {"true", "false", "unknown", "not-applicable"}
GUIDANCE_PREFIX = "../game-ready-clips.md#"
CANONICAL_LADDER = "../game-ready-clips.md#the-readiness-ladder"
VENDOR_ID_MARKER = "<!-- vendor-id -->"
VENDOR_ID_MARKER_RE = re.compile(r"(?:^|[^\w`\\])`[^`\r\n]+`<!-- vendor-id -->")


def _parse_report_markdown(text: str) -> dict[str, Any]:
    """Parse reports while admitting only exact inline vendor-id comments."""
    document = parse_markdown(text)
    if (
        document["has_raw_html"]
        and not document["has_raw_html_block"]
        and document["inline_html"]
        and len(document["vendor_id_markers"]) == len(document["inline_html"])
        and all(value == VENDOR_ID_MARKER for value in document["inline_html"])
    ):
        document["has_raw_html"] = False
    return document


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Check a report and evidence appendix.")
    parser.add_argument("report", type=Path)
    parser.add_argument("--appendix", type=Path)
    parser.add_argument("--evaluation-model-v1", action="store_true", help="validate the fixed V1 renderer projection")
    return parser.parse_args()


def _root() -> Path:
    return Path(__file__).resolve().parents[4]


@functools.lru_cache(maxsize=1)
def _parser_binary() -> Path:
    root = _root()
    environment = os.environ.copy()
    environment["RUSTC_WRAPPER"] = ""
    result = subprocess.run(
        [
            "cargo", "build", "-p", "animsmith", "--example",
            "animation_pack_markdown_ast", "--no-default-features",
            "--message-format=json-render-diagnostics",
        ],
        cwd=root,
        env=environment,
        text=True,
        capture_output=True,
        check=True,
    )
    for line in result.stdout.splitlines():
        message = json.loads(line)
        target = message.get("target", {})
        executable = message.get("executable")
        if (
            message.get("reason") == "compiler-artifact"
            and target.get("name") == "animation_pack_markdown_ast"
            and executable
        ):
            return Path(executable)
    raise RuntimeError("Cargo did not report the Markdown parser executable")


@functools.lru_cache(maxsize=1024)
def parse_markdown(text: str) -> dict[str, Any]:
    """Parse Markdown with the repository's pinned pulldown-cmark helper."""
    result = subprocess.run(
        [_parser_binary()], input=text, text=True, capture_output=True, check=True
    )
    document = json.loads(result.stdout)
    if not isinstance(document, dict):
        raise RuntimeError("Markdown parser returned a non-object document")
    return document


def rendered_word_count(text: str) -> int:
    return int(parse_markdown(text)["word_count"])


def _heading_order(
    document: dict[str, Any], required: tuple[str, ...], *, level: int,
    section: str | None = None,
) -> list[str]:
    headings = [
        heading["text"] for heading in document["headings"]
        if heading["level"] == level
        and not heading["blockquote"]
        and heading["list_depth"] == 0
        and (section is None or heading["section"] == section)
    ]
    errors: list[str] = []
    cursor = -1
    marker = "## " if level == 2 else "### "
    for label in required:
        positions = [index for index, heading in enumerate(headings) if heading == label]
        if not positions:
            errors.append(f"missing required heading: {marker}{label}")
            continue
        if len(positions) > 1:
            errors.append(f"required heading appears multiple times: {marker}{label}")
        position = positions[0]
        if position <= cursor:
            errors.append(f"required heading is out of order: {marker}{label}")
        cursor = max(cursor, position)
    return errors


def _document_identity(
    document: dict[str, Any], prefix: str
) -> str | None:
    headings = [heading for heading in document["headings"] if heading["level"] == 1]
    if (
        len(headings) != 1
        or headings[0]["blockquote"]
        or headings[0]["list_depth"] != 0
        or not headings[0]["text"].startswith(prefix)
    ):
        return None
    identity = headings[0]["text"][len(prefix):].strip()
    return identity or None


def _paragraphs(document: dict[str, Any], section: str) -> list[dict[str, Any]]:
    return [p for p in document["paragraphs"] if p["section"] == section]


def _top_level_paragraphs(
    document: dict[str, Any], section: str
) -> list[dict[str, Any]]:
    return [
        paragraph
        for paragraph in document["paragraphs"]
        if paragraph["section"] == section
        and paragraph["list_depth"] == 0
        and not paragraph["blockquote"]
    ]


def _subsection_paragraphs(
    document: dict[str, Any], section: str, subsection: str
) -> list[dict[str, Any]]:
    return [
        paragraph
        for paragraph in document["paragraphs"]
        if paragraph["section"] == section
        and paragraph["subsection"] == subsection
        and paragraph["list_depth"] == 0
        and not paragraph["blockquote"]
    ]


def _tables(document: dict[str, Any], section: str) -> list[dict[str, Any]]:
    return [
        table
        for table in document["tables"]
        if table["section"] == section
        and not table["blockquote"]
        and table["list_depth"] == 0
    ]


def _section_body_count(
    document: dict[str, Any], section: str, subsection: str | None = None
) -> int:
    def belongs(block: dict[str, Any]) -> bool:
        return block["section"] == section and (
            subsection is None or block["subsection"] == subsection
        )

    paragraphs = sum(belongs(block) for block in document["paragraphs"])
    tables = sum(belongs(block) for block in document["tables"])
    code_blocks = sum(belongs(block) for block in document["code_blocks"])
    rules = sum(belongs(block) for block in document["rules"])
    headings = sum(
        heading["section"] == section
        and (subsection is None or heading["subsection"] == subsection)
        and (
            heading["level"] > (2 if subsection is None else 3)
            or heading["list_depth"] > 0
            or heading["blockquote"]
        )
        for heading in document["headings"]
    )
    return paragraphs + tables + code_blocks + rules + headings


def _required_body_errors(
    document: dict[str, Any], sections: tuple[str, ...],
    subsections: tuple[tuple[str, str], ...] = (),
) -> list[str]:
    def content_count(section: str, subsection: str | None = None) -> int:
        def belongs(block: dict[str, Any]) -> bool:
            return block["section"] == section and (
                subsection is None or block["subsection"] == subsection
            )

        paragraphs = sum(
            belongs(block) and bool(block["text"].strip())
            for block in document["paragraphs"]
        )
        tables = sum(
            belongs(table)
            and any(
                cell["text"].strip()
                for row in table["rows"]
                for cell in row
            )
            for table in document["tables"]
        )
        code_blocks = sum(
            belongs(block) and bool(block["text"].strip())
            for block in document["code_blocks"]
        )
        return paragraphs + tables + code_blocks

    errors = [
        f"required section is empty: ## {section}"
        for section in sections
        if content_count(section) == 0
    ]
    errors.extend(
        f"required subsection is empty: ### {subsection}"
        for section, subsection in subsections
        if content_count(section, subsection) == 0
    )
    return errors


def _header(table: dict[str, Any]) -> tuple[str, ...]:
    return tuple(cell["text"] for cell in table["header"])


def _table(
    document: dict[str, Any], section: str, header: tuple[str, ...],
    subsection: str | None = None,
) -> dict[str, Any] | None:
    matching = [
        table
        for table in _tables(document, section)
        if _header(table) == header
        and (subsection is None or table["subsection"] == subsection)
    ]
    return matching[0] if len(matching) == 1 else None


def _missing_table(header: tuple[str, ...]) -> str:
    return "missing required table header: " + " | ".join(header)


def _ascii_slug(value: str) -> bool:
    return bool(value) and value[0].isascii() and value[0].isalnum() and all(
        character.isascii()
        and (character.islower() or character.isdigit() or character in "._/-")
        for character in value
    )


def _ascii_decimal(value: str) -> bool:
    whole, separator, fraction = value.partition(".")
    if not whole or not all("0" <= c <= "9" for c in whole):
        return False
    if len(whole) > 1 and whole.startswith("0"):
        return False
    return not separator or bool(fraction) and all("0" <= c <= "9" for c in fraction)


def _public_number(value: str, *, positive: bool) -> bool:
    if not _ascii_decimal(value):
        return False
    try:
        decimal, binary = Decimal(value), float(value)
    except (InvalidOperation, OverflowError, ValueError):
        return False
    if not decimal.is_finite() or not math.isfinite(binary):
        return False
    return decimal > 0 and binary > 0 if positive else decimal >= 0


def _ascii_count(value: str) -> bool:
    return 1 <= len(value) <= 18 and all("0" <= c <= "9" for c in value)


def _key_values(value: str) -> tuple[dict[str, str], bool]:
    result: dict[str, str] = {}
    malformed = False
    for raw in value.split(";"):
        key, separator, item = raw.strip().partition("=")
        if not separator or not key or not _ascii_slug(item) or key in result:
            malformed = True
        else:
            result[key] = item
    return result, malformed


def _timing_values(value: str) -> tuple[dict[str, str], bool]:
    if value == "N/A":
        return {}, False
    # V1's fixed renderer may summarize all member witnesses as an available
    # count plus a deterministic single value/range.  The full exact values
    # remain in its authority ledger; this report cell must not impersonate one
    # member as the set metric.
    if value.startswith("duration available="):
        return {}, False
    result: dict[str, str] = {}
    malformed = False
    for raw in value.split(";"):
        key, separator, number_unit = raw.strip().partition("=")
        if not separator or key not in TIMING_UNITS or key in result:
            malformed = True
            continue
        unit = TIMING_UNITS[key]
        if unit is None:
            number = number_unit
        else:
            suffix = " " + unit
            if not number_unit.endswith(suffix):
                malformed = True
                continue
            number = number_unit[: -len(suffix)]
        if not _public_number(number, positive=key in {"duration", "sample_rate", "frames"}):
            malformed = True
        else:
            result[key] = number
    return result, malformed


def _metadata(document: dict[str, Any], label: str) -> dict[str, Any] | None:
    prefix = label + ": "
    matching = [
        paragraph
        for paragraph in document["paragraphs"]
        if paragraph["blockquote"]
        and paragraph["list_depth"] == 0
        and paragraph["section"] == ""
        and paragraph["subsection"] == ""
        and paragraph["text"].startswith(prefix)
    ]
    return matching[0] if len(matching) == 1 else None


def _bold_metadata_value(
    document: dict[str, Any], label: str, *, allow_boundary: bool = False
) -> str | None:
    paragraph = _metadata(document, label)
    if paragraph is None or len(paragraph["strong"]) != 1:
        return None
    value = paragraph["strong"][0]
    prefix = f"{label}: {value}"
    if paragraph["text"] == prefix:
        return value
    if allow_boundary and paragraph["text"].startswith(prefix + " — "):
        return value
    return None


def _evaluation_date(document: dict[str, Any]) -> str | None:
    value = _bold_metadata_value(document, "Evaluation date")
    if value is None:
        return None
    try:
        parsed = date.fromisoformat(value)
    except ValueError:
        return None
    return value if parsed.isoformat() == value else None


def _each_member_has_one_code_span(cell: dict[str, Any]) -> bool:
    boundaries = [-1]
    boundaries.extend(
        index for index, character in enumerate(cell["text"]) if character == ";"
    )
    boundaries.append(len(cell["text"]))
    for start_marker, end in zip(boundaries, boundaries[1:]):
        start = start_marker + 1
        spans = [
            span
            for span in cell["code_spans"]
            if span["start"] >= start and span["end"] <= end
        ]
        if len(spans) != 1 or not spans[0]["text"].strip():
            return False
    return True


def _has_link(cell: dict[str, Any], prefix: str) -> bool:
    return any(link["destination"].startswith(prefix) for link in cell["links"])


def _placeholder_errors(document: dict[str, Any]) -> list[str]:
    placeholders = document["placeholders"]
    if not placeholders:
        return []
    preview = ", ".join(placeholders[:5])
    remainder = len(placeholders) - 5
    suffix = f" (and {remainder} more)" if remainder > 0 else ""
    return [f"unresolved template placeholders: {preview}{suffix}"]


def _validate_runtime_sets(document: dict[str, Any]) -> list[str]:
    section = "Runtime sets and authored motion"
    no_sets = "No important runtime sets were identified."
    paragraphs = _top_level_paragraphs(document, section)
    absence = [p for p in paragraphs if p["text"] == no_sets]
    table = _table(document, section, RUNTIME_SET_HEADER)
    errors: list[str] = []
    if absence and table is not None:
        errors.append("runtime-set inventory contradicts the explicit no-set result")
    if table is None:
        if (
            len(paragraphs) == 1
            and paragraphs[0]["text"] == no_sets
            and _section_body_count(document, section) == 1
        ):
            return errors
        return errors + ["runtime-set inventory must use the required member/contract table"]
    if not table["rows"]:
        return errors + ["runtime-set inventory must contain at least one member row"]
    for index, row in enumerate(table["rows"], start=1):
        if len(row) != len(RUNTIME_SET_HEADER):
            errors.append(f"runtime-set member row {index} must contain exactly six cells")
            continue
        missing = [RUNTIME_SET_HEADER[i] for i, cell in enumerate(row) if not cell["text"]]
        if missing:
            errors.append(f"runtime-set member row {index} has empty required cells: " + ", ".join(missing))
            continue
        members = [item.strip() for item in row[2]["text"].split(";")]
        if not _each_member_has_one_code_span(row[2]):
            errors.append(f"runtime-set member row {index} must name every exact member in code")

        variants, malformed_variant = _key_values(row[3]["text"])
        if len(variants) != 1 or set(variants) - {"variant", "set_type"}:
            malformed_variant = True
        if "variant" in variants and variants["variant"] not in REPORT_VARIANTS:
            malformed_variant = True
        if "set_type" in variants and variants["set_type"] not in SET_TYPES:
            malformed_variant = True
        if malformed_variant:
            errors.append(f"runtime-set member row {index} has malformed variant or set type")
        variant = variants.get("variant", "")
        ip_members = sum(item.startswith("IP ") for item in members)
        rm_members = sum(item.startswith("RM ") for item in members)
        if variant == "paired-ip-rm" and (len(members) != 2 or ip_members != 1 or rm_members != 1):
            errors.append(f"runtime-set member row {index} must name exactly one IP and one RM member")
        code_members = [span["text"] for span in row[2]["code_spans"]]
        if len(code_members) != len(set(code_members)):
            errors.append(f"runtime-set member row {index} must name distinct exact members")
        if ip_members and variant not in {"in-place", "paired-ip-rm"}:
            errors.append(f"runtime-set member row {index} has IP members without an in-place variant")
        if rm_members and variant not in {"root-motion", "rotation-only-root", "paired-ip-rm"}:
            errors.append(f"runtime-set member row {index} has RM members without a root-motion variant")

        timing, malformed_timing = _timing_values(row[4]["text"])
        contract, malformed_contract = _key_values(row[5]["text"])
        if row[5]["text"] == "N/A" or set(contract) - CONTRACT_KEYS:
            malformed_contract = True
        if any(key.startswith("loop") and value not in LOOP_VALUES for key, value in contract.items()):
            malformed_contract = True
        requires_duration = (
            variant in {"in-place", "root-motion", "paired-ip-rm"} or ip_members or rm_members
            or bool({"loop", "loop_ip", "loop_rm", "sync"} & set(contract))
        )
        requires_speed = variant in {"root-motion", "paired-ip-rm"} or bool(rm_members)
        if malformed_timing or requires_duration and "duration" not in timing or requires_speed and "rm_speed" not in timing:
            errors.append(f"runtime-set member row {index} has malformed timing or motion evidence")
        if variant == "paired-ip-rm" and not {"loop_ip", "loop_rm", "sync"}.issubset(contract):
            malformed_contract = True
        if malformed_contract:
            errors.append(f"runtime-set member row {index} has malformed runtime contract")
    return errors


def _validate_recipe(document: dict[str, Any]) -> list[str]:
    paragraphs = _paragraphs(document, "Integration recipe")
    errors: list[str] = []
    positions: list[int] = []
    for index, (label, keys) in enumerate(zip(RECIPE_LABELS, RECIPE_KEYS), start=1):
        matching = [
            (position, paragraph)
            for position, paragraph in enumerate(paragraphs)
            if paragraph["list_item"] == index
            and paragraph["list_depth"] == 1
            and not paragraph["blockquote"]
            and f"{label}:" in paragraph["strong"]
        ]
        if len(matching) != 1:
            errors.append(f"integration recipe is missing step {index}: {label}")
            continue
        position, paragraph = matching[0]
        positions.append(position)
        if not any(
            any(value.startswith(key + "=") and _ascii_slug(value.split("=", 1)[1]) for key in keys)
            for value in paragraph["code"]
        ):
            errors.append(f"integration recipe step {index} lacks an implementable {label} decision")
    if len(positions) == len(RECIPE_LABELS) and positions != sorted(positions):
        errors.append("integration recipe steps must appear in order 1 through 5")
    return errors


def validate(text: str, *, evaluation_schema: str = SCHEMA) -> list[str]:
    document = _parse_report_markdown(text)
    errors: list[str] = []
    if document["has_raw_html"]:
        errors.append("report must not contain raw HTML")
    if (
        document["first_block"] != "heading:1"
        or _document_identity(document, REPORT_TITLE_PREFIX) is None
    ):
        errors.append("report must start with '# Animation pack evaluation:'")
    errors.extend(_heading_order(document, PRIMARY_HEADINGS, level=2))
    capability_headings = V1_CAPABILITY_HEADINGS if evaluation_schema != SCHEMA else CAPABILITY_HEADINGS
    errors.extend(_heading_order(document, capability_headings, level=3, section="Capability coverage"))
    errors.extend(_required_body_errors(
        document,
        PRIMARY_HEADINGS,
        tuple(("Capability coverage", heading) for heading in capability_headings),
    ))
    errors.extend(_validate_runtime_sets(document))
    errors.extend(_validate_recipe(document))

    verdict = _metadata(document, "Technical verdict")
    if verdict is None or len(verdict["strong"]) != 1:
        errors.append("report must declare a bold Technical verdict")
    else:
        value = verdict["strong"][0]
        if verdict["text"] != f"Technical verdict: {value}":
            errors.append("report must declare a bold Technical verdict")
        elif value not in TECHNICAL_VERDICTS:
            errors.append(f"report has unknown Technical verdict: {value}")
    completeness = _bold_metadata_value(
        document, "Evaluation completeness", allow_boundary=True
    )
    if completeness is None:
        errors.append("report must declare bold Evaluation completeness")
    elif completeness not in EVALUATION_COMPLETENESS:
        errors.append(f"report has unknown Evaluation completeness: {completeness}")
    confidence = _bold_metadata_value(document, "Confidence")
    if confidence not in CONFIDENCE:
        errors.append("report must declare one canonical bold Confidence")
    if _evaluation_date(document) is None:
        errors.append("report must declare a bold YYYY-MM-DD Evaluation date")
    report_format = _metadata(document, "Report format")
    if report_format is None or report_format["text"] != "Report format: 1" or report_format["strong"] != ["1"]:
        errors.append("report must declare Report format 1")
    word_count = int(document["word_count"])
    if word_count > MAX_PRIMARY_WORDS:
        errors.append(f"primary report has {word_count} words; maximum is {MAX_PRIMARY_WORDS}")

    engine_table = _table(document, "Engine status", ENGINE_HEADER)
    engine_rows = [] if engine_table is None else engine_table["rows"]
    if engine_table is None:
        errors.append(_missing_table(ENGINE_HEADER))
    for engine in ENGINE_LABELS:
        matching = [row for row in engine_rows if row and (row[0]["text"] == engine or row[0]["text"].startswith(engine + " "))]
        if not matching:
            errors.append(f"engine status is missing: {engine}")
        elif len(matching) != 1 or len(matching[0]) != len(ENGINE_HEADER) or any(not cell["text"] for cell in matching[0]):
            errors.append(f"engine status row is malformed: {engine}")
    if not any(link["destination"] == CANONICAL_LADDER for link in document["links"]):
        errors.append("report must link the canonical readiness ladder")

    issue_table = _table(document, "Technical issue register", ISSUE_HEADER)
    issue_paragraphs = _top_level_paragraphs(document, "Technical issue register")
    clean = "No material technical issues were found at the stated scope."
    clean_paragraphs = [p for p in issue_paragraphs if p["text"] == clean]
    if issue_table is not None and clean_paragraphs:
        errors.append("technical issue register contradicts the explicit clean result")
    issue_rows = [] if issue_table is None else issue_table["rows"]
    if issue_table is None and (
        len(issue_paragraphs) != 1
        or not clean_paragraphs
        or _section_body_count(document, "Technical issue register") != 1
    ):
        errors.append("technical issue register must contain an issue table or the explicit clean result")
    issue_ids: list[str] = []
    for row in issue_rows:
        issue_id = row[0]["text"] if row else "<unknown>"
        if len(row) != len(ISSUE_HEADER):
            errors.append(f"issue {issue_id or '<unknown>'} row must contain exactly seven cells")
            continue
        missing = [ISSUE_HEADER[i] for i, cell in enumerate(row) if not cell["text"]]
        if missing:
            errors.append(f"issue {issue_id or '<unknown>'} has empty required cells: " + ", ".join(missing))
            continue
        issue_ids.append(issue_id)
        owner, severity = row[3]["text"], row[1]["text"]
        if owner not in PRIMARY_OWNERS:
            errors.append(f"issue {issue_id} has unknown or composite primary owner: {owner!r}")
        if severity not in ISSUE_SEVERITIES:
            errors.append(f"issue {issue_id} has unknown severity: {severity!r}")
        opt_out = row[2]["text"].endswith("Guidance: not applicable.") and "Guidance: not applicable." not in row[2]["code"]
        if not _has_link(row[2], GUIDANCE_PREFIX) and not opt_out:
            errors.append(f"issue {issue_id} must link applicable docs/game-ready-clips.md guidance or mark it not applicable")
    duplicates = sorted({value for value in issue_ids if issue_ids.count(value) > 1})
    if duplicates:
        errors.append("technical issue register contains duplicate IDs: " + ", ".join(duplicates))
    if any(link["destination"].startswith("https://github.com/") and "/docs/game-ready-clips.md" in link["destination"] for link in document["links"]):
        errors.append("report must use repository-relative links for local AnimSmith docs")
    errors.extend(_placeholder_errors(document))
    return errors


def _appendix_table(
    document: dict[str, Any], subsection: str, header: tuple[str, ...]
) -> tuple[dict[str, Any] | None, list[str]]:
    table = _table(
        document, "Evaluation manifest and taxonomy", header, subsection
    )
    return table, [] if table is not None else [_missing_table(header)]


def validate_appendix(text: str, *, evaluation_schema: str = SCHEMA) -> list[str]:
    document = _parse_report_markdown(text)
    errors: list[str] = []
    if document["has_raw_html"]:
        errors.append("appendix must not contain raw HTML")
    if (
        document["first_block"] != "heading:1"
        or _document_identity(document, APPENDIX_TITLE_PREFIX) is None
    ):
        errors.append("appendix must start with '# Animation pack evidence appendix:'")
    errors.extend(_heading_order(document, APPENDIX_HEADINGS, level=2))
    errors.extend(_heading_order(
        document, APPENDIX_MANIFEST_HEADINGS, level=3,
        section="Evaluation manifest and taxonomy",
    ))
    errors.extend(_required_body_errors(
        document,
        APPENDIX_HEADINGS,
        tuple(
            ("Evaluation manifest and taxonomy", heading)
            for heading in APPENDIX_MANIFEST_HEADINGS
        ),
    ))
    report_format = _metadata(document, "Report format")
    if report_format is None or report_format["text"] != "Report format: 1" or report_format["strong"] != ["1"]:
        errors.append("appendix must declare Report format 1")
    evidence_status = _bold_metadata_value(
        document, "Evidence status", allow_boundary=True
    )
    if evidence_status not in EVALUATION_COMPLETENESS:
        errors.append("appendix must declare one canonical bold Evidence status")
    if _evaluation_date(document) is None:
        errors.append("appendix must declare a bold YYYY-MM-DD Evaluation date")
    if not any(evaluation_schema in paragraph["code"] for paragraph in document["paragraphs"]):
        errors.append(f"appendix must identify evaluation manifest schema: {evaluation_schema}")
    if not any(link["destination"] == CANONICAL_LADDER for link in document["links"]):
        errors.append("appendix must link the canonical readiness ladder")
    if (
        any(
            heading["text"]
            in {"Technical issue register", "Issue and remediation register"}
            for heading in document["headings"]
        )
        or any(_header(table) == ISSUE_HEADER for table in document["tables"])
    ):
        errors.append("appendix must not duplicate the technical issue register")

    role_header = V1_ROLE_HEADER if evaluation_schema != SCHEMA else ROLE_HEADER
    role_table, table_errors = _appendix_table(
        document, "Canonical clip-role inventory", role_header
    )
    errors.extend(table_errors)
    role_rows = [] if role_table is None else role_table["rows"]
    role_totals: list[tuple[int, int]] = []
    for role in PRIMARY_ROLES:
        matching = [row for row in role_rows if row and row[0]["text"] == role and row[0]["code"] == [role]]
        if not matching:
            errors.append(f"canonical role inventory is missing: {role}")
        elif (
            len(matching) != 1 or len(matching[0]) != len(role_header)
            or not _ascii_count(matching[0][1]["text"])
            or not _ascii_count(matching[0][2]["text"])
            or not matching[0][3]["text"]
        ):
            errors.append(f"canonical role inventory row is malformed: {role}")
        else:
            role_totals.append((int(matching[0][1]["text"]), int(matching[0][2]["text"])))
    allowed_roles = set(PRIMARY_ROLES) | {"Total"}
    if any(row and row[0]["text"] not in allowed_roles for row in role_rows):
        errors.append("canonical role inventory contains an unknown role row")
    total_rows = [row for row in role_rows if row and row[0]["text"] == "Total"]
    if len(total_rows) != 1 or len(total_rows[0]) != len(role_header):
        errors.append("canonical role inventory requires one Total row")
    else:
        total_logical, total_files = total_rows[0][1]["text"], total_rows[0][2]["text"]
        if not _ascii_count(total_logical) or not _ascii_count(total_files):
            errors.append("canonical role inventory Total row is malformed")
        elif len(role_totals) == len(PRIMARY_ROLES):
            logical_total, file_total = int(total_logical), int(total_files)
            if sum(logical for logical, _files in role_totals) != logical_total:
                errors.append("canonical role inventory totals do not reconcile")
            elif evaluation_schema == SCHEMA and sum(files for _logical, files in role_totals) != file_total:
                errors.append("canonical role inventory totals do not reconcile")
            elif evaluation_schema != SCHEMA and any(files > file_total for _logical, files in role_totals):
                errors.append("canonical role inventory V1 source counts exceed the unique binding total")

    no_sets = "No runtime sets were identified."
    runtime_paragraphs = _subsection_paragraphs(
        document, "Evaluation manifest and taxonomy", "Runtime-set inventory"
    )
    absence = [paragraph for paragraph in runtime_paragraphs if paragraph["text"] == no_sets]
    runtime_table = _table(
        document,
        "Evaluation manifest and taxonomy",
        APPENDIX_RUNTIME_HEADER,
        "Runtime-set inventory",
    )
    if absence and (
        runtime_table is not None
        or len(runtime_paragraphs) != 1
        or _section_body_count(
            document, "Evaluation manifest and taxonomy", "Runtime-set inventory"
        ) != 1
    ):
        errors.append("runtime-set appendix inventory contradicts the explicit no-set result")
    if runtime_table is None and not absence:
        errors.append(_missing_table(APPENDIX_RUNTIME_HEADER))
        runtime_rows: list[list[dict[str, Any]]] = []
    elif runtime_table is None:
        runtime_rows = []
    else:
        runtime_rows = runtime_table["rows"]
        if not runtime_rows:
            errors.append("runtime-set appendix inventory requires at least one row")
    runtime_names = [row[0]["text"] for row in runtime_rows if row]
    duplicates = sorted({name for name in runtime_names if runtime_names.count(name) > 1})
    if duplicates:
        errors.append("runtime-set appendix inventory contains duplicate sets: " + ", ".join(duplicates))
    for index, row in enumerate(runtime_rows, start=1):
        if len(row) != len(APPENDIX_RUNTIME_HEADER) or any(not cell["text"] for cell in row):
            errors.append(f"runtime-set appendix row {index} is malformed")
        elif row[1]["text"] not in SET_TYPES:
            errors.append(f"runtime-set appendix row {index} has unknown type: {row[1]['text']}")

    pipeline_table, table_errors = _appendix_table(
        document, "Pipeline-stage coverage", PIPELINE_HEADER
    )
    errors.extend(table_errors)
    pipeline_rows = [] if pipeline_table is None else pipeline_table["rows"]
    if any(row and row[0]["text"] not in PIPELINE_STAGE_LABELS for row in pipeline_rows):
        errors.append("pipeline-stage coverage contains an unknown stage row")
    for label in PIPELINE_STAGE_LABELS:
        matching = [row for row in pipeline_rows if row and row[0]["text"] == label]
        if not matching:
            errors.append(f"pipeline-stage coverage is missing: {label}")
        elif (
            len(matching) != 1 or len(matching[0]) != len(PIPELINE_HEADER)
            or len(matching[0][1]["code"]) != 1
            or matching[0][1]["code"][0] not in COVERAGE_STATES
            or matching[0][1]["text"] != matching[0][1]["code"][0]
            or not matching[0][2]["text"]
        ):
            errors.append(f"pipeline-stage coverage row is malformed: {label}")

    profile_table, table_errors = _appendix_table(
        document, "Validation-profile status", PROFILE_HEADER
    )
    errors.extend(table_errors)
    profile_rows = [] if profile_table is None else profile_table["rows"]
    if any(row and row[0]["text"] not in PROFILE_LABELS for row in profile_rows):
        errors.append("validation-profile status contains an unknown profile row")
    for label in PROFILE_LABELS:
        matching = [row for row in profile_rows if row and row[0]["text"] == label]
        if not matching:
            errors.append(f"validation-profile status is missing: {label}")
            continue
        if len(matching) != 1 or len(matching[0]) != len(PROFILE_HEADER):
            errors.append(f"validation-profile status row is malformed: {label}")
            continue
        selection = matching[0][1]
        code = selection["code"]
        selection_valid = (
            code in [["not-selected"], ["not-applicable"]] and selection["text"] == code[0]
        ) or (
            len(code) == 2 and code[0] == "selected" and code[1] in ACTIVATION_BASES
            and selection["text"] == f"selected — {code[1]}"
        )
        if not selection_valid or not matching[0][2]["text"]:
            errors.append(f"validation-profile status row is malformed: {label}")
    if any(link["destination"].startswith("https://github.com/") and "/docs/game-ready-clips.md" in link["destination"] for link in document["links"]):
        errors.append("appendix must use repository-relative links for local AnimSmith docs")
    errors.extend(_placeholder_errors(document))
    return errors


def _runtime_sets(document: dict[str, Any], header: tuple[str, ...]) -> tuple[bool, set[str]]:
    if header == RUNTIME_SET_HEADER:
        table = _table(document, "Runtime sets and authored motion", header)
    else:
        table = _table(
            document,
            "Evaluation manifest and taxonomy",
            header,
            "Runtime-set inventory",
        )
    if table is not None:
        return True, {row[0]["text"] for row in table["rows"] if row}
    return False, set()


def validate_pair(
    report_text: str, appendix_text: str, report_name: str, appendix_name: str,
) -> list[str]:
    report, appendix = parse_markdown(report_text), parse_markdown(appendix_text)
    errors: list[str] = []
    report_path, appendix_path = Path(report_name), Path(appendix_name)
    expected_appendix = f"{report_path.stem}-evidence.md"
    expected_report = (
        f"{appendix_path.stem.removesuffix('-evidence')}.md"
        if appendix_path.stem.endswith("-evidence") else ""
    )
    if appendix_path.name != expected_appendix:
        errors.append(f"appendix filename must be {expected_appendix}, got {appendix_path.name}")
    if expected_report and report_path.name != expected_report:
        errors.append(f"report filename must be {expected_report}, got {report_path.name}")
    if report_path.resolve().parent != appendix_path.resolve().parent:
        errors.append("report and evidence appendix must share a directory")
    report_identity = _document_identity(report, REPORT_TITLE_PREFIX)
    appendix_identity = _document_identity(appendix, APPENDIX_TITLE_PREFIX)
    if report_identity is None or report_identity != appendix_identity:
        errors.append("report and appendix must declare the same pack identity")
    report_date = _evaluation_date(report)
    appendix_date = _evaluation_date(appendix)
    if report_date is None or report_date != appendix_date:
        errors.append("report and appendix must declare the same evaluation date")
    report_has_sets, report_sets = _runtime_sets(report, RUNTIME_SET_HEADER)
    appendix_has_sets, appendix_sets = _runtime_sets(appendix, APPENDIX_RUNTIME_HEADER)
    if report_has_sets and not appendix_has_sets:
        errors.append("report and appendix disagree on runtime-set presence")
    elif report_has_sets:
        missing = sorted(report_sets - appendix_sets)
        if missing:
            errors.append("appendix runtime-set inventory is missing primary sets: " + ", ".join(missing))
    report_link = _metadata(report, "Detailed evidence")
    if report_link is None or not any(link["destination"] == appendix_path.name for link in report_link["links"]):
        errors.append(f"report must link companion appendix: {appendix_path.name}")
    appendix_link = _metadata(appendix, "Companion report")
    if appendix_link is None or not any(link["destination"] == report_path.name for link in appendix_link["links"]):
        errors.append(f"appendix must link companion report: {report_path.name}")
    return errors


def main() -> int:
    args = parse_args()
    appendix = args.appendix or args.report.with_name(f"{args.report.stem}-evidence.md")
    try:
        report_text = args.report.read_text(encoding="utf-8")
        appendix_text = appendix.read_text(encoding="utf-8")
        evaluation_schema = "urn:animsmith:skill:animation-pack-evaluation:1" if args.evaluation_model_v1 else SCHEMA
        errors = validate(report_text, evaluation_schema=evaluation_schema)
        errors.extend(validate_appendix(appendix_text, evaluation_schema=evaluation_schema))
        errors.extend(validate_pair(report_text, appendix_text, str(args.report), str(appendix)))
    except (OSError, UnicodeError, subprocess.SubprocessError, ValueError) as error:
        print(f"validate_report.py: {error}", file=sys.stderr)
        return 2
    if errors:
        for error in errors:
            print(f"validate_report.py: {error}", file=sys.stderr)
        return 1
    print(f"validated animation-pack report pair: {args.report} + {appendix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
