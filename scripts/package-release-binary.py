#!/usr/bin/env python3
"""Stage and archive a release binary, writing a matching .sha256 sidecar.

This is the single packaging recipe shared by the release-binaries.yml
workflow and the local `check-release-packaging.sh` contract test, so the
archive layout and checksum format have one source of truth instead of an
inline heredoc that only ever runs in CI.

It stages the binary plus any extra files (README, licenses, notices) under
a `<stem>/` directory, packs that directory into `<stem>.<ext>` with the
same top-level layout for both `.tar.gz` and `.zip`, and writes
`<stem>.<ext>.sha256` in `sha256sum` format (`<digest>  <archive name>`).
"""

from __future__ import annotations

import argparse
import hashlib
import shutil
import tarfile
import zipfile
from pathlib import Path

SUPPORTED_EXTS = ("tar.gz", "zip")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path, help="Path to the built release binary.")
    parser.add_argument("--stem", required=True, help="Archive stem, e.g. animsmith-v1.2.3-x86_64-unknown-linux-gnu.")
    parser.add_argument("--ext", required=True, choices=SUPPORTED_EXTS, help="Archive extension.")
    parser.add_argument("--out-dir", default="dist", type=Path, help="Directory for the staging tree and archive (default: dist).")
    parser.add_argument("extras", nargs="*", type=Path, help="Extra files to include alongside the binary.")
    return parser.parse_args()


def stage(binary: Path, extras: list[Path], staging: Path) -> None:
    if staging.exists():
        shutil.rmtree(staging)
    staging.mkdir(parents=True)
    shutil.copy2(binary, staging / binary.name)
    for extra in extras:
        shutil.copy2(extra, staging / extra.name)


def archive_tree(staging: Path, stem: str, ext: str, archive: Path) -> None:
    files = sorted(path for path in staging.rglob("*") if path.is_file())
    if ext == "tar.gz":
        with tarfile.open(archive, "w:gz") as tar:
            for path in files:
                tar.add(path, arcname=Path(stem) / path.relative_to(staging))
    elif ext == "zip":
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as zf:
            for path in files:
                zf.write(path, Path(stem) / path.relative_to(staging))
    else:  # pragma: no cover - argparse choices already constrain this
        raise SystemExit(f"unsupported archive extension: {ext}")


def write_checksum(archive: Path) -> None:
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    Path(f"{archive}.sha256").write_text(f"{digest}  {archive.name}\n", encoding="ascii")


def main() -> None:
    args = parse_args()
    staging = args.out_dir / args.stem
    archive = args.out_dir / f"{args.stem}.{args.ext}"
    stage(args.binary, args.extras, staging)
    archive_tree(staging, args.stem, args.ext, archive)
    write_checksum(archive)
    print(f"packaged {archive} (+ {archive.name}.sha256)")


if __name__ == "__main__":
    main()
