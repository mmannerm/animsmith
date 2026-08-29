#!/usr/bin/env python3
"""Hermetic contract tests for generated Pages external-reference proxies."""

from __future__ import annotations

import hashlib
import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BUILDER = ROOT / "scripts/build-docs-site.py"
SPEC = importlib.util.spec_from_file_location("build_docs_site", BUILDER)
assert SPEC and SPEC.loader
BUILD_DOCS_SITE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = BUILD_DOCS_SITE
SPEC.loader.exec_module(BUILD_DOCS_SITE)


class ExternalProxyContractTests(unittest.TestCase):
    def fixture_source(
        self, root: Path, label: str, destination: str, reserve_proxy: bool = False, category: str = "Reference"
    ) -> None:
        (root / "docs").mkdir(parents=True)
        (root / "README.md").write_text("# fixture\n", encoding="utf-8")
        (root / ".mdbook-version").write_text("0.4.52\n", encoding="utf-8")
        (root / "docs/README.md").write_text(
            "# Documentation\n\n"
            "| Document | Use it to… | Category |\n"
            "|---|---|---|\n"
            f"| [{label}]({destination}) | External fixture. | {category} |\n",
            encoding="utf-8",
        )
        if reserve_proxy:
            digest = hashlib.sha256(destination.encode("utf-8")).hexdigest()
            reserved = root / "_generated/external" / f"{digest}.md"
            reserved.parent.mkdir(parents=True)
            reserved.write_text("reserved\n", encoding="utf-8")
        subprocess.run(["git", "init", "--quiet", str(root)], check=True)
        subprocess.run(["git", "-C", str(root), "add", "."], check=True)

    def assert_rejected_without_publication(
        self,
        label: str,
        destination: str,
        expected_error: str,
        reserve_proxy: bool = False,
        category: str = "Reference",
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            stage = root / "published"
            self.fixture_source(source, label, destination, reserve_proxy, category)
            stage.mkdir()
            sentinel = stage / "previous-publication.txt"
            sentinel.write_text("keep\n", encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(BUILDER), "--source", str(source), "--stage", str(stage)],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertNotEqual(result.returncode, 0, result.stderr)
            self.assertIn(expected_error, result.stderr)
            self.assertTrue(sentinel.is_file(), "failed staging preserves the prior publication")
            self.assertFalse((stage / "src").exists(), "failed staging publishes no partial source tree")

    def test_external_proxy_rejects_each_public_guard_without_partial_publication(self) -> None:
        cases = [
            ("label", "http://example.test/reference", "must be an https URL", False),
            ("label", "https://example.test/has space", "whitespace or control", False),
            ("label", "https://example.test/?q=>evil", "cannot be rendered exactly", False),
            ("label", "https://example.test/?q=<evil", "cannot be rendered exactly", False),
            ("label", "https://example.test/a\\b", "cannot be rendered exactly", False),
            ("x" * 257, "https://example.test/reference", "label exceeds", False),
            ("label", "https://example.test/" + "x" * 2049, "destination exceeds", False),
            ("bad\tlabel", "https://example.test/reference", "control or non-printable", False),
            ("bad\nlabel", "https://example.test/reference", "malformed index row", False),
            ("label", "https://example.test/reserved", "path is reserved", True),
        ]
        for label, destination, expected_error, reserve_proxy in cases:
            with self.subTest(expected_error=expected_error):
                self.assert_rejected_without_publication(label, destination, expected_error, reserve_proxy)
        self.assert_rejected_without_publication(
            "label",
            "https://example.test/reference",
            "category contains control or non-printable",
            category="bad\tcategory",
        )

    def test_staging_refuses_all_source_destination_overlaps_without_mutating_source(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            container = Path(temporary) / "container"
            source = container / "source"
            self.fixture_source(source, "Guide", "guide.md")
            expected_readme = source.joinpath("README.md").read_text(encoding="utf-8")
            for stage in [source / "stage", container]:
                with self.subTest(stage=stage):
                    result = subprocess.run(
                        [sys.executable, str(BUILDER), "--source", str(source), "--stage", str(stage)],
                        text=True,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                    )
                    self.assertNotEqual(result.returncode, 0, result.stderr)
                    self.assertIn("must not overlap the source checkout", result.stderr)
                    self.assertEqual(
                        source.joinpath("README.md").read_text(encoding="utf-8"),
                        expected_readme,
                        "overlap refusal leaves the canonical source untouched",
                    )
                    self.assertTrue(source.joinpath("docs/README.md").is_file())
            self.assertFalse((source / "stage").exists(), "inside-source stage was never created")

    @unittest.skipIf(os.name == "nt", "Windows symlink creation requires host-specific privileges")
    def test_staging_refuses_tracked_symlinks_without_replacing_a_prior_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            stage = root / "published"
            self.fixture_source(source, "Guide", "guide.md")
            link = source / "docs/linked-root.md"
            link.symlink_to(source / "README.md")
            subprocess.run(["git", "-C", str(source), "add", str(link.relative_to(source))], check=True)
            stage.mkdir()
            sentinel = stage / "previous-publication.txt"
            sentinel.write_text("keep\n", encoding="utf-8")

            result = subprocess.run(
                [sys.executable, str(BUILDER), "--source", str(source), "--stage", str(stage)],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

            self.assertNotEqual(result.returncode, 0, result.stderr)
            self.assertIn("refusing symbolic link in Pages source", result.stderr)
            self.assertTrue(link.is_symlink(), "refusal never deletes the canonical symlink")
            self.assertTrue(sentinel.is_file(), "refusal preserves the prior publication")
            self.assertFalse((stage / "src").exists(), "refusal publishes no partial tree")

    def test_label_controls_are_rejected_before_markdown_generation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(ValueError, "control or non-printable"):
                BUILD_DOCS_SITE.external_proxy(
                    Path(temporary), "bad\nlabel", "https://example.test/reference", {}
                )

    def test_canonical_local_destinations_cannot_escape_staged_source(self) -> None:
        self.assert_rejected_without_publication(
            "escape",
            "../../book.toml",
            "canonical index destination escapes staged source",
        )
        self.assert_rejected_without_publication(
            "rooted",
            "/book.toml",
            "canonical index destination must be a relative URL path",
        )
        self.assert_rejected_without_publication(
            "rooted",
            r"\book.toml",
            "canonical index destination must be a relative URL path",
        )
        self.assert_rejected_without_publication(
            "drive",
            r"C:\book.toml",
            "canonical index destination must be a relative URL path",
        )

    def test_external_proxy_accepts_exact_documented_byte_boundaries(self) -> None:
        label = "l" * BUILD_DOCS_SITE.MAX_LABEL_BYTES
        prefix = "https://example.test/"
        destination = prefix + "u" * (BUILD_DOCS_SITE.MAX_EXTERNAL_URL_BYTES - len(prefix.encode("utf-8")))
        self.assertEqual(len(label.encode("utf-8")), BUILD_DOCS_SITE.MAX_LABEL_BYTES)
        self.assertEqual(len(destination.encode("utf-8")), BUILD_DOCS_SITE.MAX_EXTERNAL_URL_BYTES)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            stage = root / "stage"
            self.fixture_source(source, label, destination)
            result = subprocess.run(
                [sys.executable, str(BUILDER), "--source", str(source), "--stage", str(stage)],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            proxy = next((stage / "src/_generated/external").glob("*.md"))
            self.assertIn(f"](<{destination}>)", proxy.read_text(encoding="utf-8"))

    def test_docs_serve_uses_the_same_external_stage_without_starting_a_server(self) -> None:
        result = subprocess.run(
            ["just", "--dry-run", "docs-serve"],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        stage = f"{ROOT}-docs-site"
        rendered = result.stdout + result.stderr
        self.assertIn(f'python3 scripts/build-docs-site.py --stage "{stage}"', rendered)
        self.assertIn(f'cd "{stage}" && mdbook serve -d book', rendered)

    def test_safe_brackets_and_backslash_are_escaped_without_changing_the_url(self) -> None:
        label = r"safe [ ] and \ label"
        destination = "https://example.test/reference?query=exact"
        self.assertEqual(BUILD_DOCS_SITE.markdown_text(label), r"safe \[ \] and \\ label")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            stage = root / "stage"
            self.fixture_source(source, label, destination)
            result = subprocess.run(
                [sys.executable, str(BUILDER), "--source", str(source), "--stage", str(stage)],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            summary = (stage / "src/SUMMARY.md").read_text(encoding="utf-8")
            self.assertIn(r"[safe \[ \] and \\ label](_generated/external/", summary)
            proxy = next((stage / "src/_generated/external").glob("*.md"))
            self.assertEqual(
                proxy.read_text(encoding="utf-8"),
                "# safe \\[ \\] and \\\\ label\n\n"
                "This reference is published outside the AnimSmith Pages site.\n\n"
                f"[Open safe \\[ \\] and \\\\ label](<{destination}>)\n",
            )

    def test_post_build_artifact_validator_rejects_invalid_path_component(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            book = Path(temporary) / "book"
            book.mkdir()
            (book / "bad:name").write_text("invalid on Pages artifact upload\n", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "invalid path characters"):
                BUILD_DOCS_SITE.validate_artifact_paths(book)


if __name__ == "__main__":
    unittest.main()
