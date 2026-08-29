#!/usr/bin/env python3
"""Stage and build the Pages book from the tracked repository Markdown.

The repository remains the documentation authority.  This script copies the
tracked tree into an ignored staging directory, then derives mdBook's SUMMARY
from the category column in docs/README.md.  It deliberately never writes into
the source tree and refuses symbolic links in the staged input.
"""

from __future__ import annotations

import argparse
import os
import posixpath
import shutil
import subprocess
import sys
import tempfile
import unicodedata
from pathlib import Path
from hashlib import sha256
from urllib.parse import urlsplit


INDEX = Path("docs/README.md")
PIN = Path(".mdbook-version")
GENERATED_EXTERNAL_DIR = Path("_generated/external")
MAX_EXTERNAL_URL_BYTES = 2048
MAX_LABEL_BYTES = 256


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
        validate_label(label)
        validate_category(category)
        rows.append((category, label, destination))
    if not rows:
        raise ValueError(f"{index}: canonical index table has no rows")
    return rows


def markdown_text(value: str) -> str:
    """Quote canonical index labels before placing them in generated Markdown."""
    return value.replace("\\", "\\\\").replace("[", "\\[").replace("]", "\\]")


def has_non_printable(value: str) -> bool:
    """Reject controls and formatting characters that alter generated Markdown."""
    return any(unicodedata.category(character).startswith("C") for character in value)


def validate_label(label: str) -> None:
    if has_non_printable(label):
        raise ValueError("index label contains control or non-printable characters")


def validate_category(category: str) -> None:
    if has_non_printable(category):
        raise ValueError("index category contains control or non-printable characters")


def external_proxy(source: Path, label: str, destination: str, proxies: dict[Path, str]) -> str:
    validate_label(label)
    if len(label.encode("utf-8")) > MAX_LABEL_BYTES:
        raise ValueError("external index label exceeds the generated Markdown bound")
    if len(destination.encode("utf-8")) > MAX_EXTERNAL_URL_BYTES:
        raise ValueError("external index destination exceeds the generated Markdown bound")
    if has_non_printable(destination) or any(character.isspace() for character in destination):
        raise ValueError(f"external index destination contains whitespace or control characters: {destination}")
    # Angle-bracket destinations are the only Markdown form that keeps query
    # delimiters literal, but raw angle brackets and backslashes would end or
    # escape that form.  Refuse them rather than publishing a changed URL.
    if any(character in "<>\\" for character in destination):
        raise ValueError(f"external index destination cannot be rendered exactly: {destination}")
    parsed = urlsplit(destination)
    if parsed.scheme != "https" or not parsed.netloc:
        raise ValueError(f"external index destination must be an https URL: {destination}")
    relative = GENERATED_EXTERNAL_DIR / f"{sha256(destination.encode('utf-8')).hexdigest()}.md"
    if relative in proxies:
        if proxies[relative] != destination:
            raise ValueError(f"external index destinations collide at {relative}")
        return relative.as_posix()
    proxies[relative] = destination
    if (source / relative).exists():
        raise ValueError(f"generated external-reference path is reserved: {relative}")
    proxy = source / relative
    proxy.parent.mkdir(parents=True, exist_ok=True)
    escaped_label = markdown_text(label)
    proxy.write_text(
        f"# {escaped_label}\n\n"
        "This reference is published outside the AnimSmith Pages site.\n\n"
        f"[Open {escaped_label}](<{destination}>)\n",
        encoding="utf-8",
    )
    return relative.as_posix()


def local_summary_destination(destination: str) -> str:
    """Translate a docs-index path to SUMMARY.md without crossing src/.

    Canonical index rows are relative URL paths from docs/README.md.  Refuse
    rooted and backslash paths on every host, then normalize lexically so a
    docs-to-root ../README.md remains valid while ../../ escapes do not.
    """
    target, separator, fragment = destination.partition("#")
    if (
        target.startswith(("/", "\\"))
        or "\\" in target
        or (len(target) >= 2 and target[0].isalpha() and target[1] == ":")
    ):
        raise ValueError(f"canonical index destination must be a relative URL path: {destination}")
    normalized = posixpath.normpath(posixpath.join("docs", target))
    if normalized == ".." or normalized.startswith("../"):
        raise ValueError(f"canonical index destination escapes staged source: {destination}")
    return f"{normalized}#{fragment}" if separator else normalized


def write_book_files(stage: Path, rows: list[tuple[str, str, str]], site_url: str) -> None:
    source = stage / "src"
    summary = ["# Summary", "", "- [Documentation](docs/README.md)"]
    category = None
    proxies: dict[Path, str] = {}
    for row_category, label, destination in rows:
        validate_label(label)
        validate_category(row_category)
        if row_category != category:
            category = row_category
            summary.extend(["", f"# {category}"])
        # Table destinations are relative to docs/README.md; SUMMARY.md lives
        # at the staged repository root, so retain the canonical destination's
        # meaning while changing only its presentation location.
        if urlsplit(destination).scheme:
            destination = external_proxy(source, label, destination, proxies)
        elif not destination.startswith("#"):
            destination = local_summary_destination(destination)
        summary.append(f"- [{markdown_text(label)}]({destination})")
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
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=f".{destination.name}.staging-", dir=destination.parent))
    try:
        staged_source = temporary / "src"
        for relative in tracked_files(source):
            original = source / relative
            if original.is_symlink():
                raise ValueError(f"refusing symbolic link in Pages source: {relative}")
            if not original.is_file():
                continue
            target = staged_source / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(original, target)
        write_book_files(temporary, index_rows(source / INDEX), site_url)
        if destination.exists():
            shutil.rmtree(destination)
        os.replace(temporary, destination)
    finally:
        if temporary.exists():
            shutil.rmtree(temporary)


def required_mdbook(source: Path) -> None:
    expected = (source / PIN).read_text(encoding="utf-8").strip()
    result = subprocess.run(["mdbook", "--version"], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode or result.stdout.strip() != f"mdbook v{expected}":
        raise RuntimeError(
            f"mdBook {expected} is required (found {result.stdout.strip() or 'not installed'}); "
            "install the version in .mdbook-version"
        )


def validate_artifact_paths(book: Path) -> None:
    invalid = [
        path.relative_to(book)
        for path in book.rglob("*")
        if any(character in '<>:"|?*' or ord(character) < 32 for character in path.name)
    ]
    if invalid:
        raise RuntimeError(f"rendered Pages artifact has invalid path characters: {invalid}")


def build(source: Path, destination: Path) -> None:
    required_mdbook(source)
    subprocess.run(["mdbook", "build", "-d", "book"], cwd=destination, check=True)
    validate_artifact_paths(destination / "book")


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
