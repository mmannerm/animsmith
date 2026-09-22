#!/usr/bin/env python3
"""Build the released Pages root and current-development `/dev/` subtree.

Each snapshot is built by its own checkout's tooling: the release root uses the
build script and mdBook pinned at the release tag, so publishing a new site
shape preserves released chapter content. Composition adds only version
navigation notices to released report pages and a stable latest-report route.
"""

from __future__ import annotations

import argparse
import posixpath
from html.parser import HTMLParser
import shutil
import subprocess
import sys
from pathlib import Path


RELATIVE_BUILDER = Path("scripts/build-docs-site.py")


def build(
    builder: Path,
    source: Path,
    stage: Path,
    site_url: str,
    source_ref: str,
    mdbook: Path,
) -> None:
    subprocess.run(
        [
            sys.executable,
            str(builder),
            "--source",
            str(source),
            "--stage",
            str(stage),
            "--site-url",
            site_url,
            "--source-ref",
            source_ref,
            "--mdbook",
            str(mdbook),
            "--build",
        ],
        check=True,
    )


def copy_tree(source: Path, destination: Path) -> None:
    if not source.is_dir():
        raise ValueError(f"built book is missing: {source}")
    shutil.copytree(source, destination)


def paths_overlap(left: Path, right: Path) -> bool:
    """Whether two resolved paths are equal or contain one another."""
    return left == right or left.is_relative_to(right) or right.is_relative_to(left)


def preflight_paths(
    release_source: Path,
    main_source: Path,
    release_stage: Path,
    development_stage: Path,
    output: Path,
) -> tuple[Path, Path, Path, Path, Path]:
    """Resolve and reject all source/mutable-tree aliases before doing work."""
    resolved = {
        "release-source": release_source.resolve(),
        "main-source": main_source.resolve(),
        "release-stage": release_stage.resolve(),
        "development-stage": development_stage.resolve(),
        "output": output.resolve(),
    }

    def reject_if_overlapping(left: str, right: str) -> None:
        if paths_overlap(resolved[left], resolved[right]):
            raise ValueError(
                "Pages composition path conflict: "
                f"{left} ({resolved[left]}) overlaps {right} ({resolved[right]})"
            )

    reject_if_overlapping("release-source", "main-source")
    for mutable in ("release-stage", "development-stage", "output"):
        for source in ("release-source", "main-source"):
            reject_if_overlapping(mutable, source)
    reject_if_overlapping("release-stage", "development-stage")
    reject_if_overlapping("release-stage", "output")
    reject_if_overlapping("development-stage", "output")
    return tuple(resolved[role] for role in (
        "release-source",
        "main-source",
        "release-stage",
        "development-stage",
        "output",
    ))


class MainStart(HTMLParser):
    """Locate the first main element without rewriting released HTML content."""

    def __init__(self, text: str) -> None:
        super().__init__()
        self.lines = text.splitlines(keepends=True)
        self.insertion_offset: int | None = None
        self.feed(text)

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "main" and self.insertion_offset is None:
            line, column = self.getpos()
            self.insertion_offset = sum(len(value) for value in self.lines[:line - 1]) + column + len(self.get_starttag_text())


def link_latest_evaluations(output: Path) -> None:
    """Add navigation chrome to old report snapshots and a stable latest route."""
    target = Path("dev/docs/reports/index.html")
    if not (output / target).is_file():
        raise ValueError("latest evaluations require the development report index")
    for page in sorted((output / "docs/reports").glob("*.html")):
        text = page.read_text(encoding="utf-8")
        offset = MainStart(text).insertion_offset
        if offset is None:  # Redirect aliases contain no chapter content.
            continue
        href = posixpath.relpath(target.as_posix(), page.relative_to(output).parent.as_posix())
        banner = ('\n<aside class="warning" aria-label="Report version">'
                  'This is a release snapshot. '
                  f'<a href="{href}">Read the latest pack evaluations</a> '
                  '(current main).</aside>\n')
        page.write_text(text[:offset] + banner + text[offset:], encoding="utf-8")
    latest = output / "evaluations"
    latest.mkdir()
    (latest / "index.html").write_text(
        '<!doctype html><html lang="en"><head><meta charset="utf-8">'
        '<title>Latest animation-pack evaluations</title>'
        '<meta http-equiv="refresh" content="0; url=../dev/docs/reports/index.html">'
        '</head><body><a href="../dev/docs/reports/index.html">'
        'Read the latest animation-pack evaluations</a></body></html>\n',
        encoding="utf-8",
    )


def compose(
    release_builder: Path | None,
    development_builder: Path,
    release_source: Path,
    main_source: Path,
    release_stage: Path,
    development_stage: Path,
    output: Path,
    release_tag: str,
    release_mdbook: Path,
    development_mdbook: Path,
) -> None:
    """Put the selected release at `/` and current main at `/dev/`."""
    (
        release_source,
        main_source,
        release_stage,
        development_stage,
        output,
    ) = preflight_paths(
        release_source,
        main_source,
        release_stage,
        development_stage,
        output,
    )
    if not release_tag:
        raise ValueError("release tag is required")
    build(
        release_builder or release_source / RELATIVE_BUILDER,
        release_source,
        release_stage,
        "/animsmith/",
        release_tag,
        release_mdbook,
    )
    build(
        development_builder,
        main_source,
        development_stage,
        "/animsmith/dev/",
        "main",
        development_mdbook,
    )
    if output.exists():
        shutil.rmtree(output)
    copy_tree(release_stage / "book", output)
    copy_tree(development_stage / "book", output / "dev")
    link_latest_evaluations(output)
    (output / "BUILD-INFO.txt").write_text(
        f"Release root: {release_tag}\nDevelopment subtree: main\n",
        encoding="utf-8", newline="\n",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--release-builder", type=Path)
    parser.add_argument(
        "--development-builder", type=Path, default=Path(__file__).with_name("build-docs-site.py")
    )
    parser.add_argument("--release-source", type=Path, required=True)
    parser.add_argument("--main-source", type=Path, required=True)
    parser.add_argument("--release-stage", type=Path, required=True)
    parser.add_argument("--development-stage", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--release-tag", required=True)
    parser.add_argument("--release-mdbook", type=Path, required=True)
    parser.add_argument("--development-mdbook", type=Path, required=True)
    args = parser.parse_args()
    compose(
        args.release_builder,
        args.development_builder,
        args.release_source,
        args.main_source,
        args.release_stage,
        args.development_stage,
        args.output,
        args.release_tag,
        args.release_mdbook,
        args.development_mdbook,
    )


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Pages composition: {error}", file=sys.stderr)
        sys.exit(1)
