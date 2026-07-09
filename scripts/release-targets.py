#!/usr/bin/env python3
"""Render canonical release target metadata for workflows and docs."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = REPO_ROOT / "release-targets.json"
DEFAULT_DOCS_CLI = REPO_ROOT / "docs" / "cli.md"
START_MARKER = "<!-- release-targets:start -->"
END_MARKER = "<!-- release-targets:end -->"
SUPPORTED_ARCHIVE_EXTENSIONS = {"tar.gz", "zip"}


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

    required = {"platform", "os", "target", "binary", "archive_extension", "python"}
    seen_targets: set[str] = set()
    targets: list[dict[str, str]] = []
    for index, raw in enumerate(raw_targets, start=1):
        if not isinstance(raw, dict):
            raise SystemExit(f"{manifest}: release_targets[{index}] must be an object")
        missing = sorted(required - raw.keys())
        if missing:
            raise SystemExit(f"{manifest}: release_targets[{index}] missing {', '.join(missing)}")

        target: dict[str, str] = {}
        for key in sorted(required):
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


def github_matrix(targets: list[dict[str, str]]) -> str:
    return json.dumps({"include": targets}, separators=(",", ":"))


def markdown_table(targets: list[dict[str, str]]) -> str:
    lines = [
        START_MARKER,
        "| Platform | Archive |",
        "|---|---|",
    ]
    for target in targets:
        lines.append(f"| {target['platform']} | `{archive_name(target)}` |")
    lines.append(END_MARKER)
    return "\n".join(lines)


def replace_block(text: str, replacement: str) -> str:
    start = text.find(START_MARKER)
    if start == -1:
        raise SystemExit(f"missing {START_MARKER}")
    end = text.find(END_MARKER, start)
    if end == -1:
        raise SystemExit(f"missing {END_MARKER}")
    end += len(END_MARKER)
    return f"{text[:start]}{replacement}{text[end:]}"


def render_docs(targets: list[dict[str, str]], docs_path: Path, *, check: bool) -> None:
    original = docs_path.read_text(encoding="utf-8")
    updated = replace_block(original, markdown_table(targets))
    if check:
        if updated != original:
            try:
                docs_label = docs_path.relative_to(REPO_ROOT)
            except ValueError:
                docs_label = docs_path
            raise SystemExit(
                f"{docs_label} release target table is stale; "
                "run scripts/release-targets.py write-docs"
            )
    else:
        docs_path.write_text(updated, encoding="utf-8")


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
    subcommands = parser.add_subparsers(dest="command", required=True)
    subcommands.add_parser("github-matrix", help="Print a GitHub Actions matrix as compact JSON.")
    subcommands.add_parser("check-docs", help="Verify docs/cli.md matches the manifest.")
    subcommands.add_parser("write-docs", help="Update docs/cli.md from the manifest.")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    targets = load_targets(args.manifest)

    if args.command == "github-matrix":
        print(github_matrix(targets))
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
