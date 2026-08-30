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
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
BUILDER = ROOT / "scripts/build-docs-site.py"
COMPOSER = ROOT / "scripts/compose-pages-site.py"
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

    def fixture_pages_source(self, root: Path) -> Path:
        source = root / "source"
        (source / "docs/reports").mkdir(parents=True)
        (source / "examples/assets").mkdir(parents=True)
        (source / "schemas").mkdir()
        (source / ".mdbook-version").write_text("0.4.52\n", encoding="utf-8")
        (source / "README.md").write_text("# Root\n", encoding="utf-8")
        (source / "CONTRIBUTING.md").write_text("# Contributing\n", encoding="utf-8")
        (source / "examples/assets/README.md").write_text("# Assets\n", encoding="utf-8")
        (source / "schemas/example.json").write_text("{}\n", encoding="utf-8")
        (source / "docs/README.md").write_text(
            "# Documentation\n\n"
            "| Document | Use it to… | Category |\n"
            "|---|---|---|\n"
            "| [Reports](reports/README.md) | Read reports. | Guides |\n"
            "| [Contributing](../CONTRIBUTING.md) | Contribute. | Reference |\n"
            "| [Assets](../examples/assets/README.md) | Inspect fixtures. | Reference |\n"
            "| [Schemas](../schemas/) | Inspect schemas. | Reference |\n",
            encoding="utf-8",
        )
        (source / "docs/reports/README.md").write_text(
            "# Reports\n\n"
            "| Technical report | Evidence appendix | Scope | Evaluation status |\n"
            "|---|---|---|---|\n"
            "| [One](one.md) | [Evidence](one-evidence.md) | Fixture | Current |\n",
            encoding="utf-8",
        )
        (source / "docs/reports/one.md").write_text("# One\n", encoding="utf-8")
        (source / "docs/reports/one-evidence.md").write_text(
            "# One evidence\n", encoding="utf-8"
        )
        subprocess.run(["git", "init", "--quiet", str(source)], check=True)
        subprocess.run(["git", "-C", str(source), "add", "."], check=True)
        return source

    def invoke_fixture_build(
        self, source: Path, stage: Path, include_missing_report: bool = False
    ) -> None:
        real_run = subprocess.run

        def run(command: list[str], *args: object, **kwargs: object) -> subprocess.CompletedProcess[str]:
            if command[0] != "fixture-mdbook":
                return real_run(command, *args, **kwargs)
            if command[1:] == ["--version"]:
                return subprocess.CompletedProcess(
                    command, 0, stdout="mdbook v0.4.52\n", stderr=""
                )
            self.assertEqual(command[1:], ["build", "-d", "book"])
            book = Path(str(kwargs["cwd"])) / "book"
            reports = book / "docs/reports"
            reports.mkdir(parents=True)
            (book / "index.html").write_text("<h1>Root</h1>\n", encoding="utf-8")
            missing = (
                '<a href="reports/missing.html">missing report</a>\n'
                if include_missing_report
                else ""
            )
            (book / "docs/index.html").write_text(
                '<a href="reports/README.html">reports</a>\n'
                '<a href="../CONTRIBUTING.html">contributing</a>\n'
                '<a href="../examples/assets/README.html">assets</a>\n'
                '<a href="../schemas/">schemas</a>\n'
                '<a href="/animsmith/dev/docs/reports/one.html">root-relative report</a>\n'
                + missing,
                encoding="utf-8",
            )
            (reports / "index.html").write_text(
                '<a href="one.html">report</a>\n'
                '<a href="one-evidence.html">evidence</a>\n'
                + (
                    '<a href="../../missing-root.html">missing root page</a>\n'
                    if include_missing_report
                    else ""
                ),
                encoding="utf-8",
            )
            (reports / "one.html").write_text("<h1>One</h1>\n", encoding="utf-8")
            (reports / "one-evidence.html").write_text(
                "<h1>Evidence</h1>\n", encoding="utf-8"
            )
            return subprocess.CompletedProcess(command, 0)

        arguments = [
            str(BUILDER),
            "--source",
            str(source),
            "--stage",
            str(stage),
            "--site-url",
            "/animsmith/dev/",
            "--source-ref",
            "vfixture",
            "--mdbook",
            "fixture-mdbook",
            "--build",
        ]
        with mock.patch.object(BUILD_DOCS_SITE.subprocess, "run", side_effect=run):
            with mock.patch.object(sys, "argv", arguments):
                BUILD_DOCS_SITE.main()

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
        environment = os.environ.copy()
        environment.pop("ANIMSMITH_DOCS_STAGE", None)
        result = subprocess.run(
            ["just", "--dry-run", "docs-serve"],
            cwd=ROOT,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        stage = f"{ROOT}-docs-site"
        rendered = result.stdout + result.stderr
        self.assertIn(f'python3 scripts/build-docs-site.py --stage "{stage}"', rendered)
        self.assertIn(f'cd "{stage}" && mdbook serve -d book', rendered)

    def test_composer_refuses_path_aliases_before_builder_or_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            release_source = root / "release-source"
            main_source = root / "main-source"
            release_stage = root / "release-stage"
            development_stage = root / "development-stage"
            output = root / "output"
            builder = root / "fixture-builder.py"
            invoked = root / "builder-invoked"
            builder.write_text(
                "from pathlib import Path\n"
                f"Path({str(invoked)!r}).write_text('invoked\\n', encoding='utf-8')\n",
                encoding="utf-8",
            )

            cases = [
                ("source-alias", release_source, release_source / "nested-main", release_stage, development_stage, output,
                 "release-source", "main-source"),
                ("release-stage-main-source", release_source, main_source, main_source, development_stage, output,
                 "release-stage", "main-source"),
                ("output-release-source", release_source, main_source, release_stage, development_stage, release_source,
                 "release-source", "output"),
                ("output-release-stage", release_source, main_source, release_stage, development_stage, release_stage,
                 "release-stage", "output"),
                ("equal-stages", release_source, main_source, release_stage, release_stage, output,
                 "release-stage", "development-stage"),
                ("ancestor-descendant-stages", release_source, main_source, root / "stages", root / "stages/development", output,
                 "release-stage", "development-stage"),
            ]
            for name, release, main, release_build, development_build, published, first_role, second_role in cases:
                with self.subTest(name=name):
                    paths = {
                        "release-source": release,
                        "main-source": main,
                        "release-stage": release_build,
                        "development-stage": development_build,
                        "output": published,
                    }
                    sentinels = {}
                    for role, path in paths.items():
                        path.mkdir(parents=True, exist_ok=True)
                        sentinel = path / f"{name}-{role}.sentinel"
                        sentinel.write_text(f"{role}\n", encoding="utf-8")
                        sentinels[sentinel] = sentinel.read_text(encoding="utf-8")
                    result = subprocess.run(
                        [
                            sys.executable,
                            str(COMPOSER),
                            "--builder",
                            str(builder),
                            "--release-source",
                            str(release),
                            "--main-source",
                            str(main),
                            "--release-stage",
                            str(release_build),
                            "--development-stage",
                            str(development_build),
                            "--output",
                            str(published),
                            "--release-tag",
                            "v-fixture",
                            "--release-mdbook",
                            str(root / "release-mdbook"),
                            "--development-mdbook",
                            str(root / "development-mdbook"),
                        ],
                        text=True,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                    )
                    self.assertNotEqual(result.returncode, 0, result.stderr)
                    self.assertIn("Pages composition path conflict", result.stderr)
                    self.assertIn(first_role, result.stderr)
                    self.assertIn(second_role, result.stderr)
                    self.assertFalse(invoked.exists(), "path preflight runs before invoking the builder")
                    for sentinel, expected in sentinels.items():
                        self.assertEqual(
                            sentinel.read_text(encoding="utf-8"),
                            expected,
                            "path preflight preserves source and destination sentinels",
                        )

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

    def test_build_entrypoint_publishes_ref_pinned_routes_and_invokes_final_validation(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.fixture_pages_source(root)
            stage = root / "stage"
            self.invoke_fixture_build(source, stage)

            book = stage / "book"
            aliases = {
                "README.html": "index.html",
                "docs/README.html": "docs/index.html",
                "docs/reports/README.html": "docs/reports/index.html",
            }
            for alias, canonical in aliases.items():
                self.assertEqual(
                    (book / alias).read_bytes(),
                    (book / canonical).read_bytes(),
                    f"build publishes {alias} as an exact compatibility copy",
                )
            redirects = {
                "CONTRIBUTING.html": (
                    "https://github.com/mmannerm/animsmith/blob/vfixture/CONTRIBUTING.md"
                ),
                "examples/assets/README.html": (
                    "https://github.com/mmannerm/animsmith/blob/vfixture/examples/assets/README.md"
                ),
                "schemas/index.html": (
                    "https://github.com/mmannerm/animsmith/tree/vfixture/schemas"
                ),
            }
            for relative, expected_url in redirects.items():
                output = (book / relative).read_text(encoding="utf-8")
                self.assertIn(expected_url, output, f"build pins {relative} to its source ref")
                self.assertNotIn("/main/", output)
            self.assertEqual(
                {
                    path.relative_to(book).as_posix()
                    for path in book.rglob("*.html")
                },
                {
                    "index.html",
                    "README.html",
                    "docs/index.html",
                    "docs/README.html",
                    "docs/reports/index.html",
                    "docs/reports/README.html",
                    "docs/reports/one.html",
                    "docs/reports/one-evidence.html",
                    *redirects,
                },
                "the build publishes exactly every chapter, alias, and eligible source redirect",
            )
            BUILD_DOCS_SITE.validate_rendered_local_links(book, "/animsmith/dev/")

            with self.assertRaises(RuntimeError) as failure:
                self.invoke_fixture_build(source, root / "broken-stage", include_missing_report=True)
            self.assertIn("reports/missing.html", str(failure.exception))
            self.assertIn("missing-root.html", str(failure.exception))

    def test_report_index_parser_refuses_missing_malformed_and_empty_tables(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            index = Path(temporary) / "README.md"
            index.write_text("# No table\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "canonical current-reports table"):
                BUILD_DOCS_SITE.report_rows(index)

            index.write_text(
                "| Technical report | Evidence appendix | Scope | Evaluation status |\n"
                "|---|---|---|---|\n"
                "| [One](one.md) | [Evidence](evidence.md) | Too few |\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "malformed current-reports row"):
                BUILD_DOCS_SITE.report_rows(index)

            index.write_text(
                "| Technical report | Evidence appendix | Scope | Evaluation status |\n"
                "|---|---|---|---|\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "has no rows"):
                BUILD_DOCS_SITE.report_rows(index)

    def test_source_ref_guard_rejects_empty_controls_whitespace_and_oversize(self) -> None:
        cases = [
            ("", "required"),
            ("bad ref", "whitespace or control"),
            ("bad\nref", "whitespace or control"),
            ("x" * (BUILD_DOCS_SITE.MAX_SOURCE_REF_BYTES + 1), "exceeds"),
        ]
        for source_ref, expected in cases:
            with self.subTest(source_ref=source_ref[:20]):
                with self.assertRaisesRegex(ValueError, expected):
                    BUILD_DOCS_SITE.validate_source_ref(source_ref)
        BUILD_DOCS_SITE.validate_source_ref("v0.9.0")

    def test_rendered_link_validator_rejects_missing_local_target(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            book = Path(temporary) / "book"
            reports = book / "docs/reports"
            reports.mkdir(parents=True)
            index = reports / "index.html"
            index.write_text(
                '<a href="protofactor-basic-locomotion.html#technical-issue-register">report</a>\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(RuntimeError, "no published target"):
                BUILD_DOCS_SITE.validate_rendered_local_links(book, "/animsmith/")

            (reports / "protofactor-basic-locomotion.html").write_text(
                '<h1 id="technical-issue-register">report</h1>\n', encoding="utf-8"
            )
            BUILD_DOCS_SITE.validate_rendered_local_links(book, "/animsmith/")

    def test_built_readme_chapters_receive_compatibility_aliases_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            staged = root / "src"
            book = root / "book"
            (staged / "docs").mkdir(parents=True)
            (staged / "unpublished").mkdir()
            (book / "docs").mkdir(parents=True)
            (staged / "README.md").write_text("# Root\n", encoding="utf-8")
            (staged / "docs/README.md").write_text("# Docs\n", encoding="utf-8")
            (staged / "unpublished/README.md").write_text("# Hidden\n", encoding="utf-8")
            (book / "index.html").write_text("root output\n", encoding="utf-8")
            (book / "docs/index.html").write_text("docs output\n", encoding="utf-8")

            BUILD_DOCS_SITE.publish_readme_aliases(staged, book)

            self.assertEqual((book / "README.html").read_text(encoding="utf-8"), "root output\n")
            self.assertEqual(
                (book / "docs/README.html").read_text(encoding="utf-8"), "docs output\n"
            )
            self.assertFalse(
                (book / "unpublished/README.html").exists(),
                "an unbuilt source page does not become a misleading alias",
            )

    def test_non_site_source_references_redirect_but_missing_docs_still_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            staged = root / "src"
            book = root / "book"
            (staged / "examples/assets").mkdir(parents=True)
            (staged / "examples/assets/README.md").write_text("# Assets\n", encoding="utf-8")
            (staged / "docs/reports").mkdir(parents=True)
            (staged / "docs/reports/missing.md").write_text("# Missing report\n", encoding="utf-8")
            (book / "docs").mkdir(parents=True)
            (book / "docs/index.html").write_text(
                '<a href="../examples/assets/README.html">source</a>\n'
                '<a href="reports/missing.html">report</a>\n',
                encoding="utf-8",
            )

            links = BUILD_DOCS_SITE.rendered_local_links(book, "/animsmith/")
            BUILD_DOCS_SITE.publish_source_redirects(staged, book, links, "main")

            redirect = book / "examples/assets/README.html"
            self.assertTrue(redirect.is_file())
            self.assertIn(
                "https://github.com/mmannerm/animsmith/blob/main/examples/assets/README.md",
                redirect.read_text(encoding="utf-8"),
            )
            self.assertFalse(
                (book / "docs/reports/missing.html").exists(),
                "site documentation cannot silently degrade to a source redirect",
            )
            with self.assertRaisesRegex(RuntimeError, "reports/missing.html"):
                BUILD_DOCS_SITE.validate_rendered_local_links(book, "/animsmith/")

    def test_rendered_link_resolution_refuses_relative_and_decoded_root_escape(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            book = Path(temporary) / "book"
            (book / "docs").mkdir(parents=True)
            (book / "docs/index.html").write_text(
                '<a href="../../outside.html">relative escape</a>\n'
                '<a href="/animsmith/%2Foutside.html">decoded root escape</a>\n'
                '<a href="/another-site/outside.html">wrong site</a>\n',
                encoding="utf-8",
            )

            with self.assertRaisesRegex(RuntimeError, "local link escapes artifact") as failure:
                BUILD_DOCS_SITE.validate_rendered_local_links(book, "/animsmith/")
            self.assertIn("../../outside.html", str(failure.exception))
            self.assertIn("/animsmith/%2Foutside.html", str(failure.exception))
            self.assertIn("/another-site/outside.html", str(failure.exception))
            self.assertIn("root-relative link escapes site URL", str(failure.exception))

    def test_rendered_link_resolution_accepts_encoded_file_and_directory_index(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            book = Path(temporary) / "book"
            (book / "docs").mkdir(parents=True)
            (book / "schemas").mkdir()
            (book / "docs/index.html").write_text(
                '<a href="encoded%20report.html">encoded file</a>\n'
                '<a href="../schemas/">directory</a>\n',
                encoding="utf-8",
            )
            (book / "docs/encoded report.html").write_text("report\n", encoding="utf-8")
            (book / "schemas/index.html").write_text("schemas\n", encoding="utf-8")

            BUILD_DOCS_SITE.validate_rendered_local_links(book, "/animsmith/")


if __name__ == "__main__":
    unittest.main()
