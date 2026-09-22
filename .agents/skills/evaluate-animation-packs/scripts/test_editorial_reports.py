#!/usr/bin/env python3
"""Behavioral checks for opt-in editorial report pairs (format 3)."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import validate_report as reports
from test_validators import valid_appendix, valid_report


DETAIL_HEADER = (
    "| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |\n"
    "|---|---|---|---|---|---|\n"
)
DETAIL_ROW = (
    "| Walk | F `(0,1)` | IP `Walk.fbx`; RM `Walk_RM.fbx` | "
    "variant=paired-ip-rm | duration=1.0 s; rm_speed=1.0 m/s | "
    "loop_ip=true; loop_rm=true; sync=gait-phase |\n"
)
SUMMARY_HEADER = (
    "| Set | Controller use | Adoption decision | Exact members |\n"
    "|---|---|---|---|\n"
)
SUMMARY_ROW = (
    "| Walk | Forward locomotion | Prototype with controller-owned movement; "
    "test contacts. | [Exact measured members](fixture-evidence.md#exact-runtime-members) |\n"
)


def valid_editorial_pair() -> tuple[str, str]:
    """Adapt the established format-2 fixture without changing its measurements."""
    primary = valid_report().replace("Report format: **2**", "Report format: **3**")
    appendix = valid_appendix().replace("Report format: **2**", "Report format: **3**")
    primary = primary.replace(
        "### Complete core\nFixture capability.\n\n"
        "### Partial supporting gameplay\nFixture capability.\n\n"
        "### Absent\nFixture capability.",
        "### Content present\nWalk and root-motion variants are present.\n\n"
        "### Content gaps and unknowns\nOther gameplay roles are unclassified.\n\n"
        "### Evaluation still needed\nTarget-controller acceptance remains open.",
    )
    primary = primary.replace(DETAIL_HEADER + DETAIL_ROW, SUMMARY_HEADER + SUMMARY_ROW)
    appendix = appendix.replace(
        "### Runtime-set inventory\n",
        "### Exact runtime members\n" + DETAIL_HEADER + DETAIL_ROW +
        "\n### Runtime-set inventory\n",
    )
    return primary, appendix


def no_set_editorial_pair() -> tuple[str, str]:
    primary, appendix = valid_editorial_pair()
    primary = primary.replace(
        SUMMARY_HEADER + SUMMARY_ROW,
        "No important runtime sets were identified.\n",
    )
    appendix = appendix.replace(
        DETAIL_HEADER + DETAIL_ROW,
        "No important runtime sets were identified.\n",
    ).replace(
        "| Runtime set | Type | Members/variants | Grouping evidence | Validation status |\n"
        "|---|---|---|---|---|\n"
        "| Walk | directional-blend | IP/RM pair | Fixture. | Fixture. |\n",
        "No runtime sets were identified.\n",
    )
    return primary, appendix


class EditorialReportTests(unittest.TestCase):
    def test_all_published_editorial_member_links_resolve_to_valid_evidence(self):
        directory = Path(__file__).resolve().parents[4] / "docs/reports"
        for report in directory.glob("*.md"):
            if report.name == "README.md" or report.name.endswith("-evidence.md"):
                continue
            with self.subTest(report=report.name):
                self.assertEqual(reports.validate_member_links(report), [])


    def assert_pair_error(self, primary: str, appendix: str, expected: str) -> None:
        errors = (
            reports.validate(primary)
            + reports.validate_appendix(appendix)
            + reports.validate_pair(primary, appendix, "fixture.md", "fixture-evidence.md")
        )
        self.assertTrue(any(expected in error for error in errors), errors)

    def test_valid_pair_retains_member_measurements(self) -> None:
        primary, appendix = valid_editorial_pair()
        self.assertIn("duration=1.0 s; rm_speed=1.0 m/s", appendix)
        self.assertEqual([], reports.validate(primary))
        self.assertEqual([], reports.validate_appendix(appendix))
        self.assertEqual([], reports.validate_pair(
            primary, appendix, "fixture.md", "fixture-evidence.md",
        ))

    def test_summary_must_be_present_and_unique(self) -> None:
        primary, appendix = valid_editorial_pair()
        self.assert_pair_error(primary.replace(SUMMARY_HEADER + SUMMARY_ROW, ""), appendix,
                               "nonempty set summary table")
        self.assert_pair_error(primary.replace(SUMMARY_ROW, SUMMARY_ROW * 2), appendix,
                               "set summary repeats a set")

    def test_summary_set_must_have_exact_detail(self) -> None:
        primary, appendix = valid_editorial_pair()
        self.assert_pair_error(primary.replace("| Walk | Forward locomotion", "| Run | Forward locomotion"),
                               appendix, "set summary has no exact member evidence")
        self.assert_pair_error(primary, appendix.replace("| Walk | directional-blend", "| Run | directional-blend"),
                               "appendix runtime-set inventory is missing primary sets")

    def test_member_link_requires_local_filename_and_exact_fragment(self) -> None:
        primary, appendix = valid_editorial_pair()
        for target in (
            "https://example.org/fixture-evidence.md#exact-runtime-members",
            "../fixture-evidence.md#exact-runtime-members",
            "fixture-evidence.md#wrong-fragment",
        ):
            with self.subTest(target=target):
                changed = primary.replace(
                    "fixture-evidence.md#exact-runtime-members", target,
                )
                self.assert_pair_error(changed, appendix, "exact-member appendix link")

    def test_foreign_constituent_link_is_syntactically_allowed(self) -> None:
        primary, appendix = valid_editorial_pair()
        changed = primary.replace(
            "fixture-evidence.md#exact-runtime-members",
            "constituent-evidence.md#exact-runtime-members",
        )
        self.assertEqual([], reports.validate(changed))
        self.assertEqual([], reports.validate_pair(
            changed, appendix, "fixture.md", "fixture-evidence.md",
        ))

    def test_appendix_can_record_an_additional_set(self) -> None:
        primary, appendix = valid_editorial_pair()
        extra_detail = DETAIL_ROW.replace("| Walk |", "| Run |", 1).replace(
            "Walk.fbx", "Run.fbx",
        ).replace("Walk_RM.fbx", "Run_RM.fbx")
        inventory = "| Walk | directional-blend | IP/RM pair | Fixture. | Fixture. |\n"
        extra_inventory = "| Run | directional-blend | IP/RM pair | Fixture. | Fixture. |\n"
        changed = appendix.replace(DETAIL_ROW, DETAIL_ROW + extra_detail).replace(
            inventory, inventory + extra_inventory,
        )
        self.assertEqual([], reports.validate_appendix(changed))
        self.assertEqual([], reports.validate_pair(
            primary, changed, "fixture.md", "fixture-evidence.md",
        ))

    def test_moved_member_table_required_and_well_formed(self) -> None:
        primary, appendix = valid_editorial_pair()
        self.assert_pair_error(primary, appendix.replace(DETAIL_HEADER + DETAIL_ROW, ""),
                               "runtime-set inventory must use the required member/contract table")
        self.assert_pair_error(primary, appendix.replace("duration=1.0 s", "duration=unknown"),
                               "malformed timing or motion evidence")

    def test_pair_requires_matching_format_and_evaluator(self) -> None:
        primary, appendix = valid_editorial_pair()
        self.assert_pair_error(primary, appendix.replace("Report format: **3**", "Report format: **2**"),
                               "same report format")
        self.assert_pair_error(primary, appendix.replace("Current evaluator: **AnimSmith 0.7.0**",
                                                        "Current evaluator: **AnimSmith 0.8.0**"),
                               "same current AnimSmith evaluator")

    def test_explicit_no_set_pair_is_valid(self) -> None:
        primary, appendix = no_set_editorial_pair()
        self.assertEqual([], reports.validate(primary))
        self.assertEqual([], reports.validate_appendix(appendix))
        self.assertEqual([], reports.validate_pair(
            primary, appendix, "fixture.md", "fixture-evidence.md",
        ))

    def test_resolved_foreign_member_link(self) -> None:
        primary, appendix = valid_editorial_pair()
        foreign = primary.replace(
            "fixture-evidence.md#exact-runtime-members",
            "constituent-evidence.md#exact-runtime-members",
        )
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            report_path = folder / "fixture.md"
            report_path.write_text(foreign, encoding="utf-8")
            target = folder / "constituent-evidence.md"
            target.write_text(appendix, encoding="utf-8")
            self.assertEqual([], reports.validate_member_links(report_path))

            target.unlink()
            errors = reports.validate_member_links(report_path)
            self.assertTrue(any("missing or outside" in error for error in errors), errors)

            target.write_text(appendix.replace(DETAIL_HEADER + DETAIL_ROW, ""), encoding="utf-8")
            errors = reports.validate_member_links(report_path)
            self.assertTrue(any("no exact-member table" in error for error in errors), errors)

            target.write_text(appendix.replace("duration=1.0 s", "duration=unknown"), encoding="utf-8")
            errors = reports.validate_member_links(report_path)
            self.assertTrue(any("malformed timing or motion evidence" in error for error in errors), errors)

            if hasattr(os, "symlink"):
                outside = folder.parent / (folder.name + "-outside-evidence.md")
                try:
                    outside.write_text(appendix, encoding="utf-8")
                    target.unlink()
                    target.symlink_to(outside)
                    errors = reports.validate_member_links(report_path)
                    self.assertTrue(any("missing or outside" in error for error in errors), errors)
                except OSError:
                    pass
                finally:
                    outside.unlink(missing_ok=True)

    def test_cli_accepts_unicode_with_non_utf8_process_locale(self) -> None:
        primary, appendix = valid_editorial_pair()
        primary = primary.replace("Forward locomotion", "Caf\u00e9 locomotion")
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            report_path = folder / "fixture.md"
            appendix_path = folder / "fixture-evidence.md"
            report_path.write_text(primary, encoding="utf-8")
            appendix_path.write_text(appendix, encoding="utf-8")
            environment = os.environ.copy()
            environment.update(PYTHONUTF8="0", PYTHONCOERCECLOCALE="0", LC_ALL="C")
            result = subprocess.run(
                [sys.executable, str(Path(reports.__file__)), str(report_path)],
                capture_output=True, text=True, encoding="utf-8", check=False,
                env=environment,
            )
        self.assertEqual(0, result.returncode, result.stderr)

    def test_cli_rejects_missing_foreign_member_target(self) -> None:
        primary, appendix = valid_editorial_pair()
        foreign = primary.replace(
            "fixture-evidence.md#exact-runtime-members",
            "missing-evidence.md#exact-runtime-members",
        )
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            report_path = folder / "fixture.md"
            appendix_path = folder / "fixture-evidence.md"
            report_path.write_text(foreign, encoding="utf-8")
            appendix_path.write_text(appendix, encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(Path(reports.__file__)), str(report_path),
                 "--appendix", str(appendix_path)],
                capture_output=True, text=True, check=False,
            )
        self.assertEqual(1, result.returncode, result.stderr)
        self.assertIn("exact-member evidence target is missing", result.stderr)


if __name__ == "__main__":
    unittest.main()
