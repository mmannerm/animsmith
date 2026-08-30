#!/usr/bin/env python3
"""Stage and build the Pages book from the tracked repository Markdown.

The repository remains the documentation authority.  This script copies the
tracked tree into an ignored staging directory, derives mdBook's top-level
SUMMARY from docs/README.md, and adds the canonical animation-report pairs from
docs/reports/README.md as nested chapters.  It deliberately never writes into
the source tree, refuses symbolic links in the staged input, and validates that
every rendered local link resolves inside the built artifact.
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
from html import escape
from html.parser import HTMLParser
from pathlib import Path
from hashlib import sha256
from urllib.parse import quote, unquote, urlsplit


INDEX = Path("docs/README.md")
REPORT_INDEX = Path("docs/reports/README.md")
PIN = Path(".mdbook-version")
GENERATED_EXTERNAL_DIR = Path("_generated/external")
MAX_EXTERNAL_URL_BYTES = 2048
MAX_LABEL_BYTES = 256
MAX_SOURCE_REF_BYTES = 255
REPO_URL = "https://github.com/mmannerm/animsmith"


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
        label, destination = markdown_link(document, f"{index}: Document cell")
        if not category:
            raise ValueError(f"{index}: document category is required")
        validate_category(category)
        rows.append((category, label, destination))
    if not rows:
        raise ValueError(f"{index}: canonical index table has no rows")
    return rows


def markdown_link(cell: str, context: str) -> tuple[str, str]:
    if not cell.startswith("[") or "](" not in cell or not cell.endswith(")"):
        raise ValueError(f"{context} must contain one Markdown link: {cell}")
    label, destination = cell[1:-1].split("](", 1)
    if not label or not destination:
        raise ValueError(f"{context} link label and destination are required")
    validate_label(label)
    return label, destination


def report_rows(index: Path) -> list[tuple[str, str, str]]:
    """Read the canonical technical-report/evidence pairs for nested navigation."""
    lines = index.read_text(encoding="utf-8").splitlines()
    header = "| Technical report | Evidence appendix | Scope | Evaluation status |"
    try:
        start = lines.index(header) + 2
    except ValueError as error:
        raise ValueError(f"{index} must have the canonical current-reports table") from error

    rows = []
    for line in lines[start:]:
        if not line.startswith("|"):
            break
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 4:
            raise ValueError(f"{index}: malformed current-reports row: {line}")
        technical_label, technical_destination = markdown_link(
            cells[0], f"{index}: Technical report cell"
        )
        _, evidence_destination = markdown_link(cells[1], f"{index}: Evidence appendix cell")
        rows.append((technical_label, technical_destination, evidence_destination))
    if not rows:
        raise ValueError(f"{index}: canonical current-reports table has no rows")
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


def validate_source_ref(source_ref: str) -> None:
    if not source_ref:
        raise ValueError("source ref is required")
    if len(source_ref.encode("utf-8")) > MAX_SOURCE_REF_BYTES:
        raise ValueError("source ref exceeds the GitHub URL bound")
    if has_non_printable(source_ref) or any(character.isspace() for character in source_ref):
        raise ValueError("source ref contains whitespace or control characters")


def github_source_url(kind: str, source_ref: str, relative: Path) -> str:
    validate_source_ref(source_ref)
    encoded_ref = quote(source_ref, safe="")
    encoded_path = quote(relative.as_posix(), safe="/")
    return f"{REPO_URL}/{kind}/{encoded_ref}/{encoded_path}"


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


def local_summary_destination(destination: str, base: str = "docs") -> str:
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
    normalized = posixpath.normpath(posixpath.join(base, target))
    if normalized == ".." or normalized.startswith("../"):
        raise ValueError(f"canonical index destination escapes staged source: {destination}")
    return f"{normalized}#{fragment}" if separator else normalized


def write_book_files(
    stage: Path,
    rows: list[tuple[str, str, str]],
    reports: list[tuple[str, str, str]],
    site_url: str,
) -> None:
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
        # A drive-prefixed path is not a URL scheme.  Route it through the
        # canonical local-path guard so every host refuses it consistently.
        if len(destination) >= 2 and destination[0].isalpha() and destination[1] == ":":
            destination = local_summary_destination(destination)
        elif urlsplit(destination).scheme:
            destination = external_proxy(source, label, destination, proxies)
        elif not destination.startswith("#"):
            destination = local_summary_destination(destination)
        summary.append(f"- [{markdown_text(label)}]({destination})")
        if destination == REPORT_INDEX.as_posix():
            for report_label, report_destination, evidence_destination in reports:
                report_destination = local_summary_destination(
                    report_destination, REPORT_INDEX.parent.as_posix()
                )
                evidence_destination = local_summary_destination(
                    evidence_destination, REPORT_INDEX.parent.as_posix()
                )
                summary.append(
                    f"  - [{markdown_text(report_label)}]({report_destination})"
                )
                summary.append(
                    f"    - [{markdown_text(report_label)} evidence]({evidence_destination})"
                )
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
    if destination.is_relative_to(source) or source.is_relative_to(destination):
        raise ValueError("staging directory must not overlap the source checkout")
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
        rows = index_rows(source / INDEX)
        reports = (
            report_rows(source / REPORT_INDEX)
            if any(destination == "reports/README.md" for _, _, destination in rows)
            else []
        )
        write_book_files(temporary, rows, reports, site_url)
        if destination.exists():
            shutil.rmtree(destination)
        os.replace(temporary, destination)
    finally:
        if temporary.exists():
            shutil.rmtree(temporary)


def required_mdbook(source: Path, mdbook: str) -> None:
    expected = (source / PIN).read_text(encoding="utf-8").strip()
    result = subprocess.run([mdbook, "--version"], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
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


def publish_readme_aliases(staged_source: Path, book: Path) -> None:
    """Keep mdBook's rewritten README.html links valid for index chapters."""
    for readme in sorted(staged_source.rglob("README.md")):
        relative_directory = readme.relative_to(staged_source).parent
        index = book / relative_directory / "index.html"
        if not index.is_file():
            continue
        alias = book / relative_directory / "README.html"
        if alias.exists():
            raise RuntimeError(f"README compatibility alias already exists: {alias.relative_to(book)}")
        shutil.copy2(index, alias)


class LocalAnchorParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.destinations: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag != "a":
            return
        for name, value in attrs:
            if name == "href" and value is not None:
                self.destinations.append(value)


def local_artifact_target(book: Path, page: Path, destination: str, site_path: str) -> Path:
    parsed = urlsplit(destination)
    local = unquote(parsed.path)
    if local.startswith("/"):
        if not local.startswith(site_path):
            raise ValueError("root-relative link escapes site URL")
        local = local[len(site_path) :]
        normalized = posixpath.normpath(local)
    else:
        normalized = posixpath.normpath(posixpath.join(page.parent.as_posix(), local))
    if normalized.startswith("/") or normalized == ".." or normalized.startswith("../"):
        raise ValueError("local link escapes artifact")
    target = book / normalized
    if local.endswith("/") or target.is_dir():
        target /= "index.html"
    return target


def rendered_local_links(book: Path, site_url: str) -> list[tuple[Path, str, Path | None, str | None]]:
    site_path = urlsplit(site_url).path
    if not site_path.startswith("/") or not site_path.endswith("/"):
        raise ValueError(f"site URL must be a root-relative directory: {site_url}")

    links = []
    for page in sorted(book.rglob("*.html")):
        parser = LocalAnchorParser()
        parser.feed(page.read_text(encoding="utf-8"))
        relative_page = page.relative_to(book)
        for destination in parser.destinations:
            parsed = urlsplit(destination)
            if parsed.scheme or parsed.netloc:
                continue
            if not parsed.path:
                continue
            try:
                target = local_artifact_target(book, relative_page, destination, site_path)
            except ValueError as error:
                links.append((relative_page, destination, None, str(error)))
            else:
                links.append((relative_page, destination, target, None))
    return links


def source_redirect(
    staged_source: Path, book: Path, target: Path, source_ref: str
) -> tuple[Path, str] | None:
    relative = target.relative_to(book)
    if relative.name == "index.html":
        source_relative = relative.parent
        source = staged_source / source_relative
        if not source.is_dir():
            return None
        if source_relative.parts[:1] == ("docs",) and (source / "README.md").is_file():
            return None
        return source_relative, github_source_url("tree", source_ref, source_relative)
    if relative.suffix != ".html":
        return None
    source_relative = relative.with_suffix(".md")
    source = staged_source / source_relative
    if not source.is_file() or source_relative.parts[:1] == ("docs",):
        return None
    return source_relative, github_source_url("blob", source_ref, source_relative)


def publish_source_redirects(
    staged_source: Path,
    book: Path,
    links: list[tuple[Path, str, Path | None, str | None]],
    source_ref: str,
) -> None:
    """Keep links to non-site source references useful without adding chapters."""
    for _, _, target, error in links:
        if error is not None or target is None or target.exists():
            continue
        redirect = source_redirect(staged_source, book, target, source_ref)
        if redirect is None:
            continue
        source_relative, url = redirect
        target.parent.mkdir(parents=True, exist_ok=True)
        escaped_url = escape(url, quote=True)
        escaped_source = escape(source_relative.as_posix())
        target.write_text(
            "<!doctype html>\n"
            '<meta charset="utf-8">\n'
            f'<meta http-equiv="refresh" content="0; url={escaped_url}">\n'
            f"<title>AnimSmith source reference: {escaped_source}</title>\n"
            f'<p>This reference is maintained in the source tree. '
            f'<a href="{escaped_url}">Open {escaped_source} on GitHub</a>.</p>\n',
            encoding="utf-8",
        )


def validate_rendered_link_targets(
    links: list[tuple[Path, str, Path | None, str | None]],
) -> None:
    """Refuse a rendered link inventory whose local destinations would return 404."""
    errors = []
    for page, destination, target, error in links:
        if error is not None:
            errors.append(f"{page}: {error}: {destination}")
        elif target is not None and not target.is_file():
            errors.append(f"{page}: rendered link has no published target: {destination}")
    if errors:
        raise RuntimeError("rendered Pages links are broken:\n" + "\n".join(errors))


def validate_rendered_local_links(book: Path, site_url: str) -> None:
    """Parse and validate every rendered local link in an existing artifact."""
    validate_rendered_link_targets(rendered_local_links(book, site_url))


def build(
    source: Path, destination: Path, mdbook: str, site_url: str, source_ref: str
) -> None:
    validate_source_ref(source_ref)
    required_mdbook(source, mdbook)
    subprocess.run([mdbook, "build", "-d", "book"], cwd=destination, check=True)
    publish_readme_aliases(destination / "src", destination / "book")
    links = rendered_local_links(destination / "book", site_url)
    publish_source_redirects(destination / "src", destination / "book", links, source_ref)
    validate_artifact_paths(destination / "book")
    validate_rendered_link_targets(links)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=Path.cwd())
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--site-url", default="/animsmith/")
    parser.add_argument("--source-ref", default="main")
    parser.add_argument("--mdbook", default="mdbook")
    parser.add_argument("--build", action="store_true")
    args = parser.parse_args()
    if args.build:
        validate_source_ref(args.source_ref)
    stage(args.source, args.stage, args.site_url)
    if args.build:
        build(
            args.source.resolve(),
            args.stage.resolve(),
            args.mdbook,
            args.site_url,
            args.source_ref,
        )


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        print(f"docs site: {error}", file=sys.stderr)
        sys.exit(1)
