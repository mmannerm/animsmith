#!/usr/bin/env python3
"""Render canonical release target metadata for workflows and docs."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from release_archives import SUPPORTED_ARCHIVE_EXTENSIONS

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = REPO_ROOT / "release-targets.json"
DEFAULT_DOCS_CLI = REPO_ROOT / "docs" / "cli.md"
DEFAULT_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "release-binaries.yml"
DOCS_START_MARKER = "<!-- release-targets:start -->"
DOCS_END_MARKER = "<!-- release-targets:end -->"
WORKFLOW_START_MARKER = "# release-targets:start"
WORKFLOW_END_MARKER = "# release-targets:end"
WORKFLOW_MATRIX_INDENT = "          "
REQUIRED_FIELDS = ("platform", "os", "target", "binary", "archive_extension", "python")
WORKFLOW_FIELDS = ("os", "target", "binary", "archive_extension", "python")


def load_targets(manifest: Path) -> list[dict[str, str]]:
    try:
        data = json.loads(manifest.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise SystemExit(f"{manifest} is missing") from exc
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{manifest} is not valid JSON: {exc}") from exc

    raw_targets = data.get("release_targets")
    if not isinstance(raw_targets, list) or not raw_targets:
        raise SystemExit(f"{manifest} must contain a non-empty release_targets array")

    required = set(REQUIRED_FIELDS)
    seen_targets: set[str] = set()
    targets: list[dict[str, str]] = []
    for index, raw in enumerate(raw_targets, start=1):
        if not isinstance(raw, dict):
            raise SystemExit(f"{manifest}: release_targets[{index}] must be an object")
        missing = sorted(required - raw.keys())
        if missing:
            raise SystemExit(f"{manifest}: release_targets[{index}] missing {', '.join(missing)}")

        target: dict[str, str] = {}
        for key in REQUIRED_FIELDS:
            value: Any = raw[key]
            if not isinstance(value, str) or not value:
                raise SystemExit(f"{manifest}: release_targets[{index}].{key} must be a non-empty string")
            target[key] = value

        triple = target["target"]
        if triple in seen_targets:
            raise SystemExit(f"{manifest}: duplicate release target {triple}")
        seen_targets.add(triple)

        archive_extension = target["archive_extension"]
        if archive_extension not in SUPPORTED_ARCHIVE_EXTENSIONS:
            supported = ", ".join(sorted(SUPPORTED_ARCHIVE_EXTENSIONS))
            raise SystemExit(
                f"{manifest}: release target {triple} uses unsupported archive_extension "
                f"{archive_extension!r}; expected one of {supported}"
            )

        targets.append(target)

    return targets


def archive_name(target: dict[str, str]) -> str:
    return f"animsmith-vX.Y.Z-{target['target']}.{target['archive_extension']}"


def markdown_table(targets: list[dict[str, str]]) -> str:
    lines = [
        DOCS_START_MARKER,
        "| Platform | Archive |",
        "|---|---|",
    ]
    for target in targets:
        lines.append(f"| {target['platform']} | `{archive_name(target)}` |")
    lines.append(DOCS_END_MARKER)
    return "\n".join(lines)


def workflow_matrix(targets: list[dict[str, str]]) -> str:
    lines = [f"{WORKFLOW_MATRIX_INDENT}{WORKFLOW_START_MARKER}"]
    for target in targets:
        first_key, *remaining_keys = WORKFLOW_FIELDS
        lines.append(
            f"{WORKFLOW_MATRIX_INDENT}- {first_key}: "
            f"{json.dumps(target[first_key])}"
        )
        for key in remaining_keys:
            lines.append(
                f"{WORKFLOW_MATRIX_INDENT}  {key}: "
                f"{json.dumps(target[key])}"
            )
    lines.append(f"{WORKFLOW_MATRIX_INDENT}{WORKFLOW_END_MARKER}")
    return "\n".join(lines)


def replace_block(text: str, replacement: str, start_marker: str, end_marker: str) -> str:
    start = text.find(start_marker)
    if start == -1:
        raise SystemExit(f"missing {start_marker}; run scripts/release-targets.py write")
    end = text.find(end_marker, start)
    if end == -1:
        raise SystemExit(f"missing {end_marker}; run scripts/release-targets.py write")

    line_start = text.rfind("\n", 0, start) + 1
    line_end = text.find("\n", end)
    if line_end == -1:
        line_end = len(text)
        trailing_newline = ""
    else:
        line_end += 1
        trailing_newline = "\n"
    return f"{text[:line_start]}{replacement}{trailing_newline}{text[line_end:]}"


def render_file(path: Path, replacement: str, start_marker: str, end_marker: str, *, check: bool, label: str) -> None:
    original = path.read_text(encoding="utf-8")
    updated = replace_block(original, replacement, start_marker, end_marker)
    if check:
        if updated != original:
            try:
                path_label = path.relative_to(REPO_ROOT)
            except ValueError:
                path_label = path
            raise SystemExit(
                f"{path_label} release target {label} is stale; "
                "run scripts/release-targets.py write"
            )
    else:
        path.write_text(updated, encoding="utf-8")


def render_docs(targets: list[dict[str, str]], docs_path: Path, *, check: bool) -> None:
    render_file(
        docs_path,
        markdown_table(targets),
        DOCS_START_MARKER,
        DOCS_END_MARKER,
        check=check,
        label="table",
    )


def render_workflow(targets: list[dict[str, str]], workflow_path: Path, *, check: bool) -> None:
    render_file(
        workflow_path,
        workflow_matrix(targets),
        WORKFLOW_START_MARKER,
        WORKFLOW_END_MARKER,
        check=check,
        label="matrix",
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        default=DEFAULT_MANIFEST,
        type=Path,
        help="Release target manifest (default: release-targets.json).",
    )
    parser.add_argument(
        "--docs",
        default=DEFAULT_DOCS_CLI,
        type=Path,
        help="CLI docs file to check or update (default: docs/cli.md).",
    )
    parser.add_argument(
        "--workflow",
        default=DEFAULT_WORKFLOW,
        type=Path,
        help="Release binary workflow to check or update (default: .github/workflows/release-binaries.yml).",
    )
    subcommands = parser.add_subparsers(dest="command", required=True)
    subcommands.add_parser("check", help="Verify generated workflow and docs blocks match the manifest.")
    subcommands.add_parser("write", help="Update generated workflow and docs blocks from the manifest.")
    subcommands.add_parser("check-workflow", help="Verify the workflow matrix matches the manifest.")
    subcommands.add_parser("write-workflow", help="Update the workflow matrix from the manifest.")
    subcommands.add_parser("check-docs", help="Verify docs/cli.md matches the manifest.")
    subcommands.add_parser("write-docs", help="Update docs/cli.md from the manifest.")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    targets = load_targets(args.manifest)

    if args.command == "check":
        render_workflow(targets, args.workflow, check=True)
        render_docs(targets, args.docs, check=True)
    elif args.command == "write":
        render_workflow(targets, args.workflow, check=False)
        render_docs(targets, args.docs, check=False)
    elif args.command == "check-workflow":
        render_workflow(targets, args.workflow, check=True)
    elif args.command == "write-workflow":
        render_workflow(targets, args.workflow, check=False)
    elif args.command == "check-docs":
        render_docs(targets, args.docs, check=True)
    elif args.command == "write-docs":
        render_docs(targets, args.docs, check=False)
    else:  # pragma: no cover - argparse constrains this
        raise SystemExit(f"unknown command: {args.command}")


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        sys.exit(1)
