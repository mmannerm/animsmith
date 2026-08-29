#!/usr/bin/env python3
"""Build the released Pages root and current-development `/dev/` subtree."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path


def build(builder: Path, source: Path, stage: Path, site_url: str, mdbook: Path) -> None:
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


def compose(
    builder: Path,
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
    release_source = release_source.resolve()
    main_source = main_source.resolve()
    if release_source == main_source:
        raise ValueError("release and development sources must be distinct checkouts")
    if not release_tag:
        raise ValueError("release tag is required")
    build(builder, release_source, release_stage, "/animsmith/", release_mdbook)
    build(builder, main_source, development_stage, "/animsmith/dev/", development_mdbook)
    if output.exists():
        shutil.rmtree(output)
    copy_tree(release_stage / "book", output)
    copy_tree(development_stage / "book", output / "dev")
    (output / "BUILD-INFO.txt").write_text(
        f"Release root: {release_tag}\nDevelopment subtree: main\n",
        encoding="utf-8",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--builder", type=Path, default=Path(__file__).with_name("build-docs-site.py"))
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
        args.builder,
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
