#!/usr/bin/env python3
"""Stage and build the Pages book from the tracked repository Markdown.

The repository remains the documentation authority.  This script copies the
tracked tree into an ignored staging directory, derives mdBook's navigation
from the Category column of docs/README.md, and nests the canonical
animation-report pairs from docs/reports/README.md as sub-chapters.  A
Category cell is either a part name or "Part <separator> Group"; parts become
mdBook part titles and groups become generated chapter pages that collect
their member rows.  Tracked docs/site files are staged as mdBook's theme
override directory rather than as publishable source.  The script
deliberately never writes into the source tree, refuses symbolic links in the
staged input, and validates that every rendered local link resolves inside the
built artifact.
"""

from __future__ import annotations

import argparse
import os
import posixpath
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unicodedata
from html import escape
from html.parser import HTMLParser
from pathlib import Path
from hashlib import sha256
from typing import NamedTuple
from urllib.parse import quote, unquote, urlsplit


INDEX = Path("docs/README.md")
REPORT_INDEX = Path("docs/reports/README.md")
REPORT_INDEX_ROW = REPORT_INDEX.relative_to(INDEX.parent).as_posix()
SITE = Path("docs/site")
REDIRECTS = SITE / "redirects.toml"
THEME_CSS = "animsmith.css"
PIN = Path(".mdbook-version")
GENERATED_EXTERNAL_DIR = Path("_generated/external")
GENERATED_GROUP_DIR = Path("_generated/groups")
GROUP_SEPARATOR = " › "
INLINE_LINK = re.compile(r"\]\((?P<destination>[^()\s]*)\)")
MAX_EXTERNAL_URL_BYTES = 2048
MAX_LABEL_BYTES = 256
MAX_SOURCE_REF_BYTES = 255
REPO_URL = "https://github.com/mmannerm/animsmith"


class Row(NamedTuple):
    """One canonical docs/README.md index row."""

    category: str
    label: str
    destination: str
    description: str


class Group(NamedTuple):
    """Consecutive rows sharing one group, or bare chapters when unnamed."""

    name: str
    rows: list[Row]


class Part(NamedTuple):
    """One mdBook part title and the groups published under it."""

    title: str
    groups: list[Group]


def tracked_files(source: Path) -> list[Path]:
    result = subprocess.run(
        ["git", "-C", str(source), "ls-files", "-z"],
        check=True,
        stdout=subprocess.PIPE,
    )
    return [Path(item.decode("utf-8")) for item in result.stdout.split(b"\0") if item]


def index_rows(index: Path) -> list[Row]:
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
        document, description, category = cells
        label, destination = markdown_link(document, f"{index}: Document cell")
        if not category:
            raise ValueError(f"{index}: document category is required")
        validate_text(category, "category")
        validate_text(description, "description")
        rows.append(Row(category, label, destination, description))
    if not rows:
        raise ValueError(f"{index}: canonical index table has no rows")
    return rows


def markdown_link(cell: str, context: str) -> tuple[str, str]:
    if not cell.startswith("[") or "](" not in cell or not cell.endswith(")"):
        raise ValueError(f"{context} must contain one Markdown link: {cell}")
    label, destination = cell[1:-1].split("](", 1)
    if not label or not destination:
        raise ValueError(f"{context} link label and destination are required")
    if destination.startswith("#"):
        raise ValueError(f"{context} destination must be a page, not a bare fragment: {destination}")
    validate_text(label, "label")
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


def validate_text(value: str, what: str) -> None:
    if has_non_printable(value):
        raise ValueError(f"index {what} contains control or non-printable characters")


def is_plain_text(value: str, forbidden: str = "") -> bool:
    """Whether text embeds verbatim: no controls, no whitespace, nothing forbidden."""
    return (
        not has_non_printable(value)
        and not any(character.isspace() for character in value)
        and not any(character in forbidden for character in value)
    )


def validate_source_ref(source_ref: str) -> None:
    if not source_ref:
        raise ValueError("source ref is required")
    if len(source_ref.encode("utf-8")) > MAX_SOURCE_REF_BYTES:
        raise ValueError("source ref exceeds the GitHub URL bound")
    if not is_plain_text(source_ref):
        raise ValueError("source ref contains whitespace or control characters")


def github_source_url(kind: str, source_ref: str, relative: Path) -> str:
    validate_source_ref(source_ref)
    encoded_ref = quote(source_ref, safe="")
    encoded_path = quote(relative.as_posix(), safe="/")
    return f"{REPO_URL}/{kind}/{encoded_ref}/{encoded_path}"


def is_external(destination: str) -> bool:
    """Report a URL scheme.  A drive prefix is a local path on every host."""
    if len(destination) >= 2 and destination[0].isalpha() and destination[1] == ":":
        return False
    return bool(urlsplit(destination).scheme)


def external_proxy(source: Path, label: str, destination: str, proxies: dict[Path, str]) -> str:
    validate_text(label, "label")
    if len(label.encode("utf-8")) > MAX_LABEL_BYTES:
        raise ValueError("external index label exceeds the generated Markdown bound")
    if len(destination.encode("utf-8")) > MAX_EXTERNAL_URL_BYTES:
        raise ValueError("external index destination exceeds the generated Markdown bound")
    if not is_plain_text(destination):
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
    escaped_label = markdown_text(label)
    write_generated_page(
        source,
        relative,
        f"# {escaped_label}\n\n"
        "This reference is published outside the AnimSmith Pages site.\n\n"
        f"[Open {escaped_label}](<{destination}>)\n",
        "external-reference",
    )
    return relative.as_posix()


def write_generated_page(source: Path, relative: Path, text: str, kind: str) -> None:
    """Publish a generated chapter without overwriting a canonical page."""
    page = source / relative
    if page.exists():
        raise ValueError(f"generated {kind} path is reserved: {relative}")
    page.parent.mkdir(parents=True, exist_ok=True)
    page.write_text(text, encoding="utf-8", newline="\n")


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
    if target.endswith("/"):
        normalized += "/"
    return f"{normalized}#{fragment}" if separator else normalized


def page_relative(destination: str, page: Path) -> str:
    """Present a staged-root destination from a generated page's directory."""
    target, separator, fragment = destination.partition("#")
    if target:
        relative = posixpath.relpath(target, page.parent.as_posix())
        target = f"{relative}/" if target.endswith("/") else relative
    return f"{target}#{fragment}" if separator else target


def relocate_links(description: str, page: Path) -> str:
    """Keep a canonical description's local links valid on a generated page."""

    def relocate(match: re.Match[str]) -> str:
        destination = match["destination"]
        if not destination or is_external(destination):
            return match[0]
        if destination.startswith("#"):
            # A description is written on the index page, so its same-page
            # anchors point back at that page from a generated one.
            destination = INDEX.name + destination
        return f"]({page_relative(local_summary_destination(destination), page)})"

    return INLINE_LINK.sub(relocate, description)


def split_category(category: str) -> tuple[str, str]:
    """Split a canonical Category cell into its part title and optional group."""
    title, separator, group = category.partition(GROUP_SEPARATOR)
    title, group = title.strip(), group.strip()
    # The separator character is reserved, so a stray one is a malformed cell
    # rather than a part or group name.
    if not title or (separator and not group) or any("›" in name for name in (title, group)):
        raise ValueError(
            f"index category must be 'Part' or 'Part{GROUP_SEPARATOR}Group': {category}"
        )
    return title, group


def group_slug(group: str) -> str:
    """Derive a generated group page name from its canonical title."""
    words = "".join(
        character if character.isascii() and character.isalnum() else " "
        for character in group.lower()
    ).split()
    if not words:
        raise ValueError(f"index group has no slug characters: {group}")
    return "-".join(words)


def navigation(rows: list[Row]) -> list[Part]:
    """Order canonical rows into contiguous parts and contiguous group chapters."""
    parts: list[Part] = []
    seen_parts: set[str] = set()
    group_parts: dict[str, str] = {}
    slugs: dict[str, str] = {}
    for row in rows:
        title, group = split_category(row.category)
        if not parts or parts[-1].title != title:
            if title in seen_parts:
                raise ValueError(f"index part is not contiguous: {title}")
            seen_parts.add(title)
            parts.append(Part(title, []))
        groups = parts[-1].groups
        if not groups or groups[-1].name != group:
            if group:
                if group_parts.setdefault(group, title) != title:
                    raise ValueError(f"index group appears in two parts: {group}")
                if any(published.name == group for published in groups):
                    raise ValueError(f"index group is not contiguous: {group}")
                slug = group_slug(group)
                if slugs.setdefault(slug, group) != group:
                    raise ValueError(
                        f"index groups collide at {slug}.md: {slugs[slug]} and {group}"
                    )
            groups.append(Group(group, []))
        groups[-1].rows.append(row)
    return parts


def chapter_destination(source: Path, row: Row, proxies: dict[Path, str]) -> str:
    """Resolve a canonical index destination to a staged SUMMARY chapter.

    Table destinations are relative to docs/README.md; SUMMARY.md lives at the
    staged repository root, so retain the canonical destination's meaning while
    changing only its presentation location.
    """
    if is_external(row.destination):
        return external_proxy(source, row.label, row.destination, proxies)
    return local_summary_destination(row.destination)


def chapter(depth: int, label: str, destination: str) -> str:
    return f"{'  ' * depth}- [{markdown_text(label)}]({destination})"


def report_chapters(depth: int, destination: str, reports: list[tuple[str, str, str]]) -> list[str]:
    """Nest the canonical report/evidence pairs under the reports index chapter."""
    if destination != REPORT_INDEX.as_posix():
        return []
    base = REPORT_INDEX.parent.as_posix()
    lines = []
    for label, report, evidence in reports:
        lines.append(chapter(depth, label, local_summary_destination(report, base)))
        lines.append(
            chapter(depth + 1, f"{label} evidence", local_summary_destination(evidence, base))
        )
    return lines


def write_group_page(source: Path, group: Group, destinations: list[str]) -> str:
    """Publish a generated group chapter derived from its member rows."""
    relative = GENERATED_GROUP_DIR / f"{group_slug(group.name)}.md"
    members = [
        f"- [{markdown_text(row.label)}]({page_relative(destination, relative)})"
        f" — {relocate_links(row.description, relative)}"
        for row, destination in zip(group.rows, destinations)
    ]
    write_generated_page(
        source,
        relative,
        f"# {markdown_text(group.name)}\n\n" + "\n".join(members) + "\n",
        "group",
    )
    return relative.as_posix()


def summary_markdown(
    source: Path, parts: list[Part], reports: list[tuple[str, str, str]]
) -> str:
    """Render SUMMARY.md, publishing every page the navigation generates."""
    proxies: dict[Path, str] = {}
    summary = ["# Summary", "", "- [Documentation](docs/README.md)"]
    for part in parts:
        summary.extend(["", f"# {part.title}"])
        for group in part.groups:
            destinations = [chapter_destination(source, row, proxies) for row in group.rows]
            depth = 0
            if group.name:
                page = write_group_page(source, group, destinations)
                summary.append(chapter(0, group.name, page))
                depth = 1
            for row, destination in zip(group.rows, destinations):
                summary.append(chapter(depth, row.label, destination))
                summary.extend(report_chapters(depth + 1, destination, reports))
    return "\n".join(summary) + "\n"


def redirect_entries(path: Path) -> dict[str, str]:
    """Read the tracked Pages redirect map, validated at the configuration boundary."""
    if not path.is_file():
        return {}
    try:
        entries = tomllib.loads(path.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as error:
        raise ValueError(f"{path} is not a valid redirect map: {error}") from error
    for route, target in entries.items():
        if not isinstance(target, str):
            raise ValueError(f"{path}: redirect target must be a string: {route}")
        for value in (route, target):
            if not is_plain_text(value, '":\\'):
                raise ValueError(f"{path}: redirect entry is not a plain site path: {route}")
        if not route.startswith("/") or not route.endswith(".html"):
            raise ValueError(
                f"{path}: redirect route must be a site-root path ending in .html: {route}"
            )
        if not target or target.startswith("/"):
            raise ValueError(f"{path}: redirect target must be a relative path: {target}")
    return entries


def book_toml(site_url: str, redirects: dict[str, str]) -> str:
    """Render book.toml for the staged book."""
    lines = [
        "[book]",
        'title = "AnimSmith documentation"',
        'authors = ["AnimSmith contributors"]',
        'language = "en"',
        'src = "src"',
        "",
        "[output.html]",
        f'site-url = "{site_url}"',
        f'git-repository-url = "{REPO_URL}"',
        f'edit-url-template = "{REPO_URL}/edit/main/{{path}}"',
        'default-theme = "light"',
        'preferred-dark-theme = "navy"',
        "no-section-label = true",
        f'additional-css = ["theme/{THEME_CSS}"]',
        "",
        "[output.html.fold]",
        "enable = true",
        "level = 0",
    ]
    if redirects:
        lines.extend(["", "[output.html.redirect]"])
        lines.extend(f'"{route}" = "{target}"' for route, target in redirects.items())
    return "\n".join(lines) + "\n"


def stage(source: Path, destination: Path, site_url: str) -> None:
    source = source.resolve()
    destination = destination.resolve()
    if destination.is_relative_to(source) or source.is_relative_to(destination):
        raise ValueError("staging directory must not overlap the source checkout")
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=f".{destination.name}.staging-", dir=destination.parent))
    try:
        staged_source = temporary / "src"
        theme = temporary / "theme"
        for relative in tracked_files(source):
            original = source / relative
            if original.is_symlink():
                raise ValueError(f"refusing symbolic link in Pages source: {relative}")
            if not original.is_file():
                continue
            # docs/site holds mdBook's theme override rather than publishable
            # source, and its redirect map is configuration rather than an asset.
            if relative == REDIRECTS:
                continue
            target = (
                theme / relative.relative_to(SITE)
                if relative.is_relative_to(SITE)
                else staged_source / relative
            )
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(original, target)
        if not (theme / THEME_CSS).is_file():
            raise ValueError(f"{(SITE / THEME_CSS).as_posix()} must be tracked to style the book")
        rows = index_rows(source / INDEX)
        reports = (
            report_rows(source / REPORT_INDEX)
            if any(row.destination == REPORT_INDEX_ROW for row in rows)
            else []
        )
        (staged_source / "SUMMARY.md").write_text(
            summary_markdown(staged_source, navigation(rows), reports), encoding="utf-8", newline="\n"
        )
        (temporary / "book.toml").write_text(
            book_toml(site_url, redirect_entries(source / REDIRECTS)),
            encoding="utf-8", newline="\n",
        )
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
            encoding="utf-8", newline="\n",
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
    # mdBook renders each configured redirect as a page linking its target, so
    # the rendered-link inventory also proves every redirect resolves.
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
