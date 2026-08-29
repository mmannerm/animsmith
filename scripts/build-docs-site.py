#!/usr/bin/env python3
"""Stage and build the Pages book from the tracked repository Markdown.

The repository remains the documentation authority.  This script copies the
tracked tree into an ignored staging directory, then derives mdBook's SUMMARY
from the category column in docs/README.md.  It deliberately never writes into
the source tree and refuses symbolic links in the staged input.
"""

from __future__ import annotations

import argparse
import posixpath
import shutil
import subprocess
import sys
from pathlib import Path


INDEX = Path("docs/README.md")
PIN = Path(".mdbook-version")


def tracked_files(source: Path) -> list[Path]:
    result = subprocess.run(
        ["git", "-C", str(source), "ls-files", "-z"],
        check=True,
        stdout=subprocess.PIPE,
    )
    return [Path(item.decode("utf-8")) for item in result.stdout.split(b"\0") if item]


def index_rows(index: Path) -> list[tuple[str, str, str]]:
    lines = index.read_text(encoding="utf-8").splitlines()
    header = "| Document | Use it to… | Category |"
    try:
        start = lines.index(header) + 2
    except ValueError as error:
        raise ValueError(f"{index} must have the canonical three-column index table") from error

    rows = []
    for line in lines[start:]:
        if not line.startswith("|"):
            break
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 3:
            raise ValueError(f"{index}: malformed index row: {line}")
        document, _, category = cells
        if not document.startswith("[") or "](" not in document or not document.endswith(")"):
            raise ValueError(f"{index}: Document cell must contain one Markdown link: {document}")
        label, destination = document[1:-1].split("](", 1)
        if not label or not destination or not category:
            raise ValueError(f"{index}: document label, destination, and category are required")
        rows.append((category, label, destination))
    if not rows:
        raise ValueError(f"{index}: canonical index table has no rows")
    return rows


def write_book_files(stage: Path, rows: list[tuple[str, str, str]], site_url: str) -> None:
    source = stage / "src"
    summary = ["# Summary", "", "- [Documentation](docs/README.md)"]
    category = None
    for row_category, label, destination in rows:
        if row_category != category:
            category = row_category
            summary.extend(["", f"# {category}"])
        # Table destinations are relative to docs/README.md; SUMMARY.md lives
        # at the staged repository root, so retain the canonical destination's
        # meaning while changing only its presentation location.
        if "://" not in destination and not destination.startswith("#"):
            target, separator, fragment = destination.partition("#")
            destination = posixpath.normpath(posixpath.join("docs", target))
            if separator:
                destination = f"{destination}#{fragment}"
        summary.append(f"- [{label}]({destination})")
    (source / "SUMMARY.md").write_text("\n".join(summary) + "\n", encoding="utf-8")
    (stage / "book.toml").write_text(
        "[book]\n"
        "title = \"AnimSmith documentation\"\n"
        "authors = [\"AnimSmith contributors\"]\n"
        "language = \"en\"\n"
        "src = \"src\"\n\n"
        "[output.html]\n"
        f"site-url = \"{site_url}\"\n"
        "git-repository-url = \"https://github.com/mmannerm/animsmith\"\n"
        "edit-url-template = \"https://github.com/mmannerm/animsmith/edit/main/{path}\"\n",
        encoding="utf-8",
    )


def stage(source: Path, destination: Path, site_url: str) -> None:
    source = source.resolve()
    destination = destination.resolve()
    if destination == source:
        raise ValueError("staging directory must not replace the source checkout")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    staged_source = destination / "src"
    for relative in tracked_files(source):
        original = source / relative
        if original.is_symlink():
            raise ValueError(f"refusing symbolic link in Pages source: {relative}")
        if not original.is_file():
            continue
        target = staged_source / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(original, target)
    write_book_files(destination, index_rows(source / INDEX), site_url)


def required_mdbook(source: Path) -> None:
    expected = (source / PIN).read_text(encoding="utf-8").strip()
    result = subprocess.run(["mdbook", "--version"], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode or result.stdout.strip() != f"mdbook v{expected}":
        raise RuntimeError(
            f"mdBook {expected} is required (found {result.stdout.strip() or 'not installed'}); "
            "install the version in .mdbook-version"
        )


def build(source: Path, destination: Path) -> None:
    required_mdbook(source)
    subprocess.run(["mdbook", "build", "-d", "book"], cwd=destination, check=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=Path.cwd())
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--site-url", default="/animsmith/")
    parser.add_argument("--build", action="store_true")
    args = parser.parse_args()
    stage(args.source, args.stage, args.site_url)
    if args.build:
        build(args.source.resolve(), args.stage.resolve())


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        print(f"docs site: {error}", file=sys.stderr)
        sys.exit(1)
