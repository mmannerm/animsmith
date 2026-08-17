#!/usr/bin/env python3
"""Validate the required structure of an animation-pack Markdown report."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from evaluation_contract_v1 import (
    PIPELINE_STAGE_ROWS,
    PRIMARY_OWNERS,
    PRIMARY_ROLES,
    PROFILE_ROWS,
    SCHEMA,
)


REQUIRED_HEADINGS = [
    "## Executive decision",
    "## Evaluation scope and evidence",
    "## Pack inventory and content coverage",
    "## Out-of-the-box results",
    "## AnimSmith results",
    "## Engine integration",
    "## Blending, masking, and gameplay caveats",
    "## Compatibility",
    "## Issue and remediation register",
    "## Acquisition and adoption guidance",
    "## Limitations and unknowns",
    "## Reproduction appendix",
    "## Sources",
]
REQUIRED_EXECUTIVE_HEADINGS = [
    "### Decision",
    "### Canonical clip-role inventory",
    "### Runtime-set inventory",
    "### Pipeline-stage coverage",
    "### Readiness ladder by clip set",
    "#### File-ready and clip-ready",
    "#### Set-ready and rig/use",
    "### Tooling frontier",
    "### Validation-profile status",
    "### Common-engine status",
    "### Best fit",
    "### Poor fit or material caveats",
    "### Adoption conditions",
]
PIPELINE_STAGE_LABELS = tuple(label for _identifier, label in PIPELINE_STAGE_ROWS)
PROFILE_LABELS = tuple(label for _identifier, label in PROFILE_ROWS)
MANIFEST_SCHEMA = SCHEMA
PLACEHOLDER = re.compile(r"\{\{[^{}]+\}\}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check required section order and unresolved template placeholders."
    )
    parser.add_argument("report", type=Path, help="completed Markdown report")
    return parser.parse_args()


def heading_position(text: str, heading: str) -> int:
    match = re.search(rf"^{re.escape(heading)}\s*$", text, re.MULTILINE)
    return match.start() if match else -1


def validate(text: str) -> list[str]:
    errors: list[str] = []
    if not text.startswith("# Animation pack evaluation:"):
        errors.append("report must start with '# Animation pack evaluation:'")

    cursor = -1
    for heading in REQUIRED_HEADINGS:
        position = heading_position(text, heading)
        if position < 0:
            errors.append(f"missing required heading: {heading}")
        elif position <= cursor:
            errors.append(f"required heading is out of order: {heading}")
        else:
            cursor = position

    executive_start = heading_position(text, "## Executive decision")
    executive_end = heading_position(text, "## Evaluation scope and evidence")
    if 0 <= executive_start < executive_end:
        executive = text[executive_start:executive_end]
        cursor = -1
        for heading in REQUIRED_EXECUTIVE_HEADINGS:
            position = heading_position(executive, heading)
            if position < 0:
                errors.append(f"missing required executive heading: {heading}")
            elif position <= cursor:
                errors.append(f"required executive heading is out of order: {heading}")
            else:
                cursor = position

        role_start = heading_position(executive, "### Canonical clip-role inventory")
        role_end = heading_position(executive, "### Runtime-set inventory")
        if 0 <= role_start < role_end:
            role_section = executive[role_start:role_end]
            for role in PRIMARY_ROLES:
                if f"`{role}`" not in role_section:
                    errors.append(f"canonical role inventory is missing: {role}")

        pipeline_start = heading_position(executive, "### Pipeline-stage coverage")
        pipeline_end = heading_position(executive, "### Readiness ladder by clip set")
        if 0 <= pipeline_start < pipeline_end:
            pipeline_section = executive[pipeline_start:pipeline_end]
            for label in PIPELINE_STAGE_LABELS:
                if not re.search(rf"^\|\s*{re.escape(label)}\s*\|", pipeline_section, re.MULTILINE):
                    errors.append(f"pipeline-stage coverage is missing: {label}")

        profile_start = heading_position(executive, "### Validation-profile status")
        profile_end = heading_position(executive, "### Common-engine status")
        if 0 <= profile_start < profile_end:
            profile_section = executive[profile_start:profile_end]
            for label in PROFILE_LABELS:
                if not re.search(rf"^\|\s*{re.escape(label)}\s*\|", profile_section, re.MULTILINE):
                    errors.append(f"validation-profile status is missing: {label}")

    if MANIFEST_SCHEMA not in text:
        errors.append(f"report must identify evaluation manifest schema: {MANIFEST_SCHEMA}")

    issue_start = heading_position(text, "## Issue and remediation register")
    issue_end = heading_position(text, "## Acquisition and adoption guidance")
    if 0 <= issue_start < issue_end:
        issue_section = text[issue_start:issue_end]
        for line in issue_section.splitlines():
            if not line.startswith("|"):
                continue
            cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
            if len(cells) != 7 or cells[0] in {"ID", "---"}:
                continue
            owner = cells[3].strip("`")
            if owner not in PRIMARY_OWNERS:
                errors.append(
                    f"issue {cells[0]} has unknown or composite primary owner: {owner!r}"
                )

    placeholders = sorted(set(PLACEHOLDER.findall(text)))
    if placeholders:
        preview = ", ".join(placeholders[:5])
        remainder = len(placeholders) - 5
        suffix = f" (and {remainder} more)" if remainder > 0 else ""
        errors.append(f"unresolved template placeholders: {preview}{suffix}")

    return errors


def main() -> int:
    args = parse_args()
    try:
        text = args.report.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        print(f"validate_report.py: {error}", file=sys.stderr)
        return 2

    errors = validate(text)
    if errors:
        for error in errors:
            print(f"validate_report.py: {error}", file=sys.stderr)
        return 1
    print(f"validated animation-pack report: {args.report}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
