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



def pinned_mdbook() -> bool:
    """Report whether the pinned mdBook is on PATH for end-to-end build tests."""
    expected = (ROOT / ".mdbook-version").read_text(encoding="utf-8").strip()
    try:
        version = subprocess.run(
            ["mdbook", "--version"], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
    except OSError:
        return False
    return version.returncode == 0 and version.stdout.strip() == f"mdbook v{expected}"


class NavigationContractTests(unittest.TestCase):
    """Pin the published navigation through the staged book, not internals."""

    INDEX_HEADER = "| Document | Use it to… | Category |\n|---|---|---|\n"
    START_ROWS = [
        ("[Install](../README.md)", "Install it.", "Start"),
        ("[Overview](overview.md)", "Start here.", "Start"),
    ]

    def fixture(
        self,
        root: Path,
        rows: list[tuple[str, str, str]],
        reports: list[str] | None = None,
        site: dict[str, str] | None = None,
    ) -> None:
        (root / "docs").mkdir(parents=True)
        (root / ".mdbook-version").write_text("0.4.52\n", encoding="utf-8")
        (root / "README.md").write_text("# Root\n", encoding="utf-8")
        table = "".join(
            f"| {document} | {description} | {category} |\n"
            for document, description, category in rows
        )
        (root / "docs/README.md").write_text(
            "# Documentation\n\n" + self.INDEX_HEADER + table, encoding="utf-8"
        )
        for document, _, _ in rows:
            destination = document[1:-1].split("](", 1)[1].partition("#")[0]
            if "://" in destination or destination.endswith("/"):
                continue
            page = root / "docs" / destination
            page.parent.mkdir(parents=True, exist_ok=True)
            if not page.exists():
                page.write_text(f"# {page.stem}\n", encoding="utf-8")
        for name in reports or []:
            (root / "docs/reports").mkdir(parents=True, exist_ok=True)
            (root / "docs/reports/README.md").write_text(
                "# Reports\n\n"
                "| Technical report | Evidence appendix | Scope | Evaluation status |\n"
                "|---|---|---|---|\n"
                + "".join(
                    f"| [{report}]({report}.md) | [Evidence]({report}-evidence.md) "
                    "| Fixture | Current |\n"
                    for report in reports or []
                ),
                encoding="utf-8",
            )
            for suffix in ("", "-evidence"):
                (root / f"docs/reports/{name}{suffix}.md").write_text(
                    f"# {name}{suffix}\n", encoding="utf-8"
                )
        for name, content in (site or {}).items():
            asset = root / "docs/site" / name
            asset.parent.mkdir(parents=True, exist_ok=True)
            asset.write_text(content, encoding="utf-8")
        subprocess.run(["git", "init", "--quiet", str(root)], check=True)
        subprocess.run(["git", "-C", str(root), "add", "."], check=True)

    def build_site(
        self, root: Path, arguments: list[str] | None = None, **fixture: object
    ) -> tuple[subprocess.CompletedProcess[str], Path]:
        source = root / "source"
        stage = root / "stage"
        self.fixture(source, **fixture)  # type: ignore[arg-type]
        stage.mkdir()
        sentinel = stage / "previous-publication.txt"
        sentinel.write_text("keep\n", encoding="utf-8")
        result = subprocess.run(
            [sys.executable, str(BUILDER), "--source", str(source), "--stage", str(stage)]
            + (arguments or []),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        return result, stage

    def assert_staging_rejects(self, expected: str, **fixture: object) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            result, stage = self.build_site(Path(temporary), **fixture)
            self.assertNotEqual(result.returncode, 0, result.stderr)
            self.assertIn(expected, result.stderr)
            self.assertTrue((stage / "previous-publication.txt").is_file())
            self.assertFalse((stage / "src").exists(), "rejection publishes no partial tree")

    def stage_site(self, root: Path, **fixture: object) -> Path:
        result, stage = self.build_site(root, **fixture)
        self.assertEqual(result.returncode, 0, result.stderr)
        return stage

    def test_parts_groups_and_report_pairs_nest_in_canonical_table_order(self) -> None:
        rows = [
            ("[Install](../README.md)", "Install it.", "Start"),
            ("[Overview](overview.md)", "Start here.", "Start"),
            ("[Unity](engine-unity.md)", "Engine one.", "Workflows › Engine profiles"),
            (
                "[Bevy](engine-bevy.md)",
                "Engine two, validated by [schemas](schemas/).",
                "Workflows › Engine profiles",
            ),
            ("[Recipes](recipes.md)", "Do the work.", "Workflows"),
            ("[Reports](reports/README.md)", "Read reports.", "More › Pack evaluations"),
            (
                "[API](https://docs.rs/animsmith-core)",
                "Look up [animsmith-core](https://docs.rs/animsmith-core).",
                "More › Pack evaluations",
            ),
        ]
        with tempfile.TemporaryDirectory() as temporary:
            stage = self.stage_site(Path(temporary), rows=rows, reports=["one"])
            digest = hashlib.sha256(b"https://docs.rs/animsmith-core").hexdigest()
            self.assertEqual(
                (stage / "src/SUMMARY.md").read_text(encoding="utf-8"),
                "# Summary\n"
                "\n"
                "- [Documentation](docs/README.md)\n"
                "\n"
                "# Start\n"
                "- [Install](README.md)\n"
                "- [Overview](docs/overview.md)\n"
                "\n"
                "# Workflows\n"
                "- [Engine profiles](_generated/groups/engine-profiles.md)\n"
                "  - [Unity](docs/engine-unity.md)\n"
                "  - [Bevy](docs/engine-bevy.md)\n"
                "- [Recipes](docs/recipes.md)\n"
                "\n"
                "# More\n"
                "- [Pack evaluations](_generated/groups/pack-evaluations.md)\n"
                "  - [Reports](docs/reports/README.md)\n"
                "    - [one](docs/reports/one.md)\n"
                "      - [one evidence](docs/reports/one-evidence.md)\n"
                f"  - [API](_generated/external/{digest}.md)\n",
                "parts, group chapters, members, and report pairs keep canonical order",
            )
            self.assertEqual(
                (stage / "src/_generated/groups/engine-profiles.md").read_text(encoding="utf-8"),
                "# Engine profiles\n"
                "\n"
                "- [Unity](../../docs/engine-unity.md) — Engine one.\n"
                "- [Bevy](../../docs/engine-bevy.md) — Engine two, validated by "
                "[schemas](../../docs/schemas/).\n",
                "a group page relocates member and description destinations without changing them",
            )
            self.assertEqual(
                (stage / "src/_generated/groups/pack-evaluations.md").read_text(encoding="utf-8"),
                "# Pack evaluations\n"
                "\n"
                "- [Reports](../../docs/reports/README.md) — Read reports.\n"
                f"- [API](../external/{digest}.md) — Look up "
                "[animsmith-core](https://docs.rs/animsmith-core).\n",
                "an external member routes through the proxy while prose keeps its exact URL",
            )

    def test_categories_that_cannot_produce_stable_navigation_are_refused(self) -> None:
        cases = [
            (
                [
                    ("[A](a.md)", "One.", "Start"),
                    ("[B](b.md)", "Two.", "More"),
                    ("[C](c.md)", "Three.", "Start"),
                ],
                "index part is not contiguous: Start",
            ),
            (
                [
                    ("[A](a.md)", "One.", "More › Reference"),
                    ("[B](b.md)", "Two.", "More › Pack evaluations"),
                    ("[C](c.md)", "Three.", "More › Reference"),
                ],
                "index group is not contiguous: Reference",
            ),
            (
                [
                    ("[A](a.md)", "One.", "Start › Reference"),
                    ("[B](b.md)", "Two.", "More › Reference"),
                ],
                "index group appears in two parts: Reference",
            ),
            (
                [
                    ("[A](a.md)", "One.", "Start › Engine profiles"),
                    ("[B](b.md)", "Two.", "More › Engine Profiles"),
                ],
                "index groups collide at engine-profiles.md",
            ),
            (
                [("[A](a.md)", "One.", "Start › ✦✦✦")],
                "index group has no slug characters",
            ),
            ([("[A](a.md)", "One.", "Start › ")], "index category must be"),
            ([("[A](a.md)", "One.", "Start › More › Reference")], "index category must be"),
            ([("[A](a.md)", "One.", " › Reference")], "index category must be"),
        ]
        for rows, expected in cases:
            with self.subTest(expected=expected):
                self.assert_staging_rejects(expected, rows=rows)

    def test_generated_group_page_cannot_shadow_a_canonical_page(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            self.fixture(source, rows=[("[A](a.md)", "One.", "Start › Reference")])
            reserved = source / "_generated/groups/reference.md"
            reserved.parent.mkdir(parents=True)
            reserved.write_text("reserved\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(source), "add", "."], check=True)
            result = subprocess.run(
                [
                    sys.executable,
                    str(BUILDER),
                    "--source",
                    str(source),
                    "--stage",
                    str(root / "stage"),
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertNotEqual(result.returncode, 0, result.stderr)
            self.assertIn("generated group path is reserved", result.stderr)

    def theme_site(self) -> dict[str, str]:
        return {
            "animsmith.css": ":root { --animsmith: 1; }\n",
            "fonts/fonts.css": "@font-face { font-family: Fixture; }\n",
            "redirects.toml": '"/docs/old.html" = "overview.html"\n',
        }

    def test_tracked_site_directory_is_staged_as_the_mdbook_theme(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            stage = self.stage_site(
                Path(temporary),
                rows=self.START_ROWS,
                site=self.theme_site(),
            )
            self.assertEqual(
                (stage / "theme/animsmith.css").read_text(encoding="utf-8"),
                ":root { --animsmith: 1; }\n",
            )
            self.assertTrue((stage / "theme/fonts/fonts.css").is_file())
            self.assertFalse(
                (stage / "src/docs/site").exists(), "theme assets are not publishable source"
            )
            for unpublished in ["theme/redirects.toml", "src/docs/site/redirects.toml"]:
                self.assertFalse(
                    (stage / unpublished).exists(), f"{unpublished} is configuration, not content"
                )
            self.assertEqual(
                (stage / "book.toml").read_text(encoding="utf-8"),
                '[book]\n'
                'title = "AnimSmith documentation"\n'
                'authors = ["AnimSmith contributors"]\n'
                'language = "en"\n'
                'src = "src"\n'
                "\n"
                "[output.html]\n"
                'site-url = "/animsmith/"\n'
                'git-repository-url = "https://github.com/mmannerm/animsmith"\n'
                'edit-url-template = "https://github.com/mmannerm/animsmith/edit/main/{path}"\n'
                'default-theme = "light"\n'
                'preferred-dark-theme = "navy"\n'
                "no-section-label = true\n"
                'additional-css = ["theme/animsmith.css"]\n'
                "\n"
                "[output.html.fold]\n"
                "enable = true\n"
                "level = 0\n"
                "\n"
                "[output.html.redirect]\n"
                '"/docs/old.html" = "overview.html"\n',
            )

    def test_a_checkout_without_site_assets_still_folds_and_themes_the_book(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            stage = self.stage_site(
                Path(temporary), rows=self.START_ROWS
            )
            book = (stage / "book.toml").read_text(encoding="utf-8")
            self.assertFalse((stage / "theme").exists(), "no tracked theme is staged")
            self.assertNotIn("additional-css", book, "no stylesheet is wired without one staged")
            self.assertNotIn("[output.html.redirect]", book)
            self.assertIn('default-theme = "light"', book)
            self.assertIn('preferred-dark-theme = "navy"', book)
            self.assertIn("[output.html.fold]\nenable = true\nlevel = 0\n", book)

    def test_redirect_map_refuses_entries_that_are_not_site_routes(self) -> None:
        cases = [
            ('"docs/old.html" = "overview.html"', "redirect route must be a site-root path"),
            ('"/docs/old" = "overview.html"', "redirect route must be a site-root path"),
            ('"/docs/old.html" = ""', "redirect target must be a relative path"),
            ('"/docs/old.html" = "/overview.html"', "redirect target must be a relative path"),
            ('"/docs/old.html" = "https://example.test/x"', "is not a plain site path"),
            ('"/docs/old.html" = "over view.html"', "is not a plain site path"),
            ('"/docs/old.html" = 1', "redirect target must be a string"),
            ("this is not toml", "is not a valid redirect map"),
        ]
        for entry, expected in cases:
            with self.subTest(entry=entry):
                self.assert_staging_rejects(
                    expected,
                    rows=self.START_ROWS,
                    site={"animsmith.css": "/* fixture */\n", "redirects.toml": entry + "\n"},
                )

    @unittest.skipUnless(pinned_mdbook(), "the pinned mdBook is not installed")
    def test_configured_redirects_are_published_and_broken_targets_fail_the_build(self) -> None:
        rows = self.START_ROWS
        arguments = ["--site-url", "/animsmith/", "--build"]
        with tempfile.TemporaryDirectory() as temporary:
            stage = self.stage_site(
                Path(temporary), arguments=arguments, rows=rows, site=self.theme_site()
            )
            redirect = (stage / "book/docs/old.html").read_text(encoding="utf-8")
            self.assertIn('<a href="overview.html">', redirect)
            self.assertTrue((stage / "book/docs/overview.html").is_file())
            self.assertTrue((stage / "book/theme/animsmith.css").is_file())
            self.assertTrue((stage / "book/fonts/fonts.css").is_file())
            self.assertFalse((stage / "book/redirects.toml").exists())

        with tempfile.TemporaryDirectory() as temporary:
            site = self.theme_site() | {"redirects.toml": '"/docs/old.html" = "gone.html"\n'}
            result, _ = self.build_site(
                Path(temporary), arguments=arguments, rows=rows, site=site
            )
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn("rendered link has no published target", result.stderr)
            self.assertIn("gone.html", result.stderr)


if __name__ == "__main__":
    unittest.main()
