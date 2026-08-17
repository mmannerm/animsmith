#!/usr/bin/env python3
"""Create a deterministic, non-following inventory of an animation pack."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import sys
from collections import Counter, defaultdict
from pathlib import Path, PurePosixPath
from typing import Any


ANIMSMITH_CANDIDATES = {".fbx", ".glb", ".gltf"}
ANIMATION_SOURCES = {
    ".anim",
    ".bvh",
    ".c3d",
    ".dae",
    ".ma",
    ".mb",
    ".blend",
}
ENGINE_PACKAGES = {".uasset", ".unitypackage", ".godot", ".tscn", ".tres"}
ARCHIVES = {".7z", ".bz2", ".gz", ".rar", ".tar", ".tgz", ".xz", ".zip"}
DOC_EXTENSIONS = {".html", ".md", ".pdf", ".rtf", ".txt"}
LICENSE_WORDS = {"copying", "copyright", "eula", "license", "licence", "terms"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Emit a stable JSON inventory without following symlinks. "
            "Regular files are SHA-256 hashed unless --no-hash is used."
        )
    )
    parser.add_argument("root", type=Path, help="unmodified pack directory")
    parser.add_argument(
        "--label", help="report label; defaults to the directory name"
    )
    parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        metavar="RELATIVE_PATH",
        help="exclude this relative path and its descendants; repeatable",
    )
    parser.add_argument(
        "--no-hash", action="store_true", help="record sizes without SHA-256"
    )
    return parser.parse_args()


def normalize_exclusion(raw: str) -> PurePosixPath:
    candidate = PurePosixPath(raw.replace("\\", "/"))
    if candidate.is_absolute() or not candidate.parts:
        raise ValueError(f"exclude path must be non-empty and relative: {raw!r}")
    if any(part in {"", ".", ".."} for part in candidate.parts):
        raise ValueError(f"exclude path must be normalized and stay below root: {raw!r}")
    return candidate


def is_excluded(relative: PurePosixPath, exclusions: set[PurePosixPath]) -> bool:
    return any(relative == item or item in relative.parents for item in exclusions)


def classify(relative: PurePosixPath) -> str:
    suffix = Path(relative.name).suffix.lower()
    words = {
        word
        for word in Path(relative.name).stem.lower().replace("-", "_").split("_")
        if word
    }
    if suffix in ANIMSMITH_CANDIDATES:
        return "animsmith-input-candidate"
    if suffix in ANIMATION_SOURCES:
        return "animation-source"
    if suffix in ENGINE_PACKAGES:
        return "engine-native"
    if suffix in ARCHIVES:
        return "archive"
    if words & LICENSE_WORDS:
        return "license-or-terms"
    if suffix in DOC_EXTENSIONS:
        return "documentation"
    return "other"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def inventory(
    root: Path, label: str, exclusions: set[PurePosixPath], hash_files: bool
) -> dict[str, Any]:
    records: list[dict[str, Any]] = []
    pending: list[tuple[Path, PurePosixPath]] = [(root, PurePosixPath())]

    while pending:
        directory, relative_dir = pending.pop()
        with os.scandir(directory) as entries:
            directory_entries = list(entries)
        for entry in directory_entries:
            relative = relative_dir / entry.name
            if is_excluded(relative, exclusions):
                continue
            entry_path = Path(entry.path)
            entry_stat = entry.stat(follow_symlinks=False)
            if stat.S_ISLNK(entry_stat.st_mode):
                records.append({"path": relative.as_posix(), "type": "symlink"})
            elif stat.S_ISDIR(entry_stat.st_mode):
                pending.append((entry_path, relative))
            elif stat.S_ISREG(entry_stat.st_mode):
                record: dict[str, Any] = {
                    "path": relative.as_posix(),
                    "type": "file",
                    "kind": classify(relative),
                    "size_bytes": entry_stat.st_size,
                }
                if hash_files:
                    record["sha256"] = sha256_file(entry_path)
                records.append(record)
            else:
                records.append({"path": relative.as_posix(), "type": "special"})

    records.sort(key=lambda item: os.fsencode(item["path"]))
    regular = [record for record in records if record["type"] == "file"]
    kinds = Counter(record["kind"] for record in regular)
    extensions = Counter(
        Path(record["path"]).suffix.lower() or "<none>" for record in regular
    )
    duplicate_paths: defaultdict[str, list[str]] = defaultdict(list)
    if hash_files:
        for record in regular:
            duplicate_paths[record["sha256"]].append(record["path"])

    duplicates = [
        {"sha256": digest, "paths": sorted(paths, key=os.fsencode)}
        for digest, paths in duplicate_paths.items()
        if len(paths) > 1
    ]
    duplicates.sort(key=lambda item: item["sha256"])

    return {
        "schema": "urn:animsmith:skill:animation-pack-inventory:1",
        "pack_label": label,
        "hash_algorithm": "sha256" if hash_files else None,
        "excluded_paths": sorted(
            (item.as_posix() for item in exclusions), key=os.fsencode
        ),
        "summary": {
            "regular_file_count": len(regular),
            "symlink_count": sum(record["type"] == "symlink" for record in records),
            "special_file_count": sum(
                record["type"] == "special" for record in records
            ),
            "total_regular_bytes": sum(record["size_bytes"] for record in regular),
            "by_kind": dict(sorted(kinds.items())),
            "by_extension": dict(sorted(extensions.items())),
        },
        "duplicate_file_groups": duplicates,
        "files": records,
    }


def main() -> int:
    args = parse_args()
    root = args.root
    if root.is_symlink():
        print(f"inventory_pack.py: root must not be a symlink: {root}", file=sys.stderr)
        return 2
    if not root.is_dir():
        print(f"inventory_pack.py: root is not a directory: {root}", file=sys.stderr)
        return 2
    try:
        exclusions = {normalize_exclusion(raw) for raw in args.exclude}
        result = inventory(
            root=root,
            label=args.label or root.name or root.resolve().name,
            exclusions=exclusions,
            hash_files=not args.no_hash,
        )
    except (OSError, ValueError) as error:
        print(f"inventory_pack.py: {error}", file=sys.stderr)
        return 1
    json.dump(result, sys.stdout, indent=2, sort_keys=True)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
