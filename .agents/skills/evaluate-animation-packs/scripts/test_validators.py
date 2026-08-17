#!/usr/bin/env python3
"""Behavioral tests for the animation-pack evaluation helper scripts."""

from __future__ import annotations

import copy
import hashlib
import tempfile
import unittest
from pathlib import Path, PurePosixPath

import inventory_pack
import validate_evaluation_manifest as manifest_validator
import validate_report as report_validator


def valid_manifest() -> dict[str, object]:
    roles = {
        role: {"logical_motions": 0, "delivered_files": 0}
        for role in manifest_validator.PRIMARY_ROLES
    }
    roles["idle-pose"] = {"logical_motions": 1, "delivered_files": 1}
    profiles = []
    for profile_id in manifest_validator.PROFILE_IDS:
        profile = {
            "profile_id": profile_id,
            "status": "not-selected",
            "rationale": "Not required by this fixture.",
        }
        if profile_id == "marketplace-intake":
            profile.update(
                status="selected",
                activation_basis="user-required",
                rationale="Mandatory intake coverage.",
            )
        profiles.append(profile)

    return {
        "schema": manifest_validator.SCHEMA,
        "taxonomy_version": manifest_validator.TAXONOMY_VERSION,
        "validation_profile_set_version": manifest_validator.PROFILE_SET_VERSION,
        "evaluator": {"version": "0.2.1", "revision": "fixture"},
        "motions": [
            {
                "id": "idle-neutral",
                "vendor_label": "Idle Neutral",
                "primary_role": "idle-pose",
                "tags": ["posture:standing"],
                "classification_basis": ["observed-file"],
                "files": [{"path": "Idle.fbx", "variant": "single"}],
            }
        ],
        "runtime_sets": [],
        "profiles": profiles,
        "pipeline_stages": [
            {
                "stage_id": stage_id,
                "status": "evaluated-clean",
                "evidence": "Fixture evidence.",
            }
            for stage_id in manifest_validator.PIPELINE_STAGES
        ],
        "role_totals": roles,
        "totals": {"logical_motions": 1, "delivered_files": 1, "runtime_sets": 0},
    }


def valid_report() -> str:
    role_rows = "\n".join(f"| `{role}` | 0 |" for role in report_validator.PRIMARY_ROLES)
    stage_rows = "\n".join(
        f"| {stage} | evaluated-clean |" for stage in report_validator.PIPELINE_STAGE_LABELS
    )
    profile_rows = "\n".join(
        f"| {profile} | not-selected |" for profile in report_validator.PROFILE_LABELS
    )
    return f"""# Animation pack evaluation: Validator fixture

## Executive decision

### Decision
Fixture decision.

### Canonical clip-role inventory
{role_rows}

### Runtime-set inventory
No runtime sets.

### Pipeline-stage coverage
| Stage | Status |
|---|---|
{stage_rows}

### Readiness ladder by clip set

#### File-ready and clip-ready
Fixture result.

#### Set-ready and rig/use
Fixture result.

### Tooling frontier
Fixture result.

### Validation-profile status
| Profile | Status |
|---|---|
{profile_rows}

### Common-engine status
Fixture result.

### Best fit
Fixture result.

### Poor fit or material caveats
Fixture result.

### Adoption conditions
Fixture result.

## Evaluation scope and evidence
Schema: `{report_validator.MANIFEST_SCHEMA}`

## Pack inventory and content coverage
Fixture result.

## Out-of-the-box results
Fixture result.

## AnimSmith results
Fixture result.

## Engine integration
Fixture result.

## Blending, masking, and gameplay caveats
Fixture result.

## Compatibility
Fixture result.

## Issue and remediation register
Fixture result.

## Acquisition and adoption guidance
Fixture result.

## Limitations and unknowns
Fixture result.

## Reproduction appendix
Fixture result.

## Sources
Fixture result.
"""


class InventoryTests(unittest.TestCase):
    def test_inventory_is_stable_classified_hashed_and_excludable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory) / "pack"
            root.mkdir()
            (root / "Walk.fbx").write_bytes(b"motion")
            notes = root / "Notes"
            notes.mkdir()
            (notes / "LICENSE.txt").write_text("terms", encoding="utf-8")

            exclusions = {PurePosixPath("Notes")}
            first = inventory_pack.inventory(root, "Fixture", exclusions, True)
            second = inventory_pack.inventory(root, "Fixture", exclusions, True)

        self.assertEqual(first, second)
        self.assertEqual(first["excluded_paths"], ["Notes"])
        self.assertEqual(
            first["files"],
            [
                {
                    "path": "Walk.fbx",
                    "type": "file",
                    "kind": "animsmith-input-candidate",
                    "size_bytes": 6,
                    "sha256": hashlib.sha256(b"motion").hexdigest(),
                }
            ],
        )

    def test_inventory_records_symlink_without_following_it(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary = Path(temporary_directory)
            root = temporary / "pack"
            outside = temporary / "outside"
            root.mkdir()
            outside.mkdir()
            (outside / "Hidden.fbx").write_bytes(b"not pack content")
            link = root / "linked-directory"
            try:
                link.symlink_to(outside, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation is unavailable: {error}")

            result = inventory_pack.inventory(root, "Fixture", set(), True)

        self.assertEqual(
            result["files"], [{"path": "linked-directory", "type": "symlink"}]
        )
        self.assertEqual(result["summary"]["symlink_count"], 1)

    def test_exclusion_must_not_escape_pack_root(self) -> None:
        with self.assertRaisesRegex(ValueError, "stay below root"):
            inventory_pack.normalize_exclusion("../outside")


class ManifestValidatorTests(unittest.TestCase):
    def test_accepts_complete_manifest(self) -> None:
        self.assertEqual(manifest_validator.validate_manifest(valid_manifest()), [])

    def test_rejects_duplicate_physical_file_membership(self) -> None:
        manifest = valid_manifest()
        duplicate = copy.deepcopy(manifest["motions"][0])  # type: ignore[index]
        duplicate["id"] = "idle-duplicate"
        manifest["motions"].append(duplicate)  # type: ignore[union-attr]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn("physical file 'Idle.fbx' belongs to more than one motion", errors)

    def test_rejects_declared_totals_that_do_not_reconcile(self) -> None:
        manifest = valid_manifest()
        manifest["totals"] = {
            "logical_motions": 99,
            "delivered_files": 1,
            "runtime_sets": 0,
        }

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "totals does not match motions, physical files, and runtime sets", errors
        )

    def test_requires_selected_profile_activation_basis(self) -> None:
        manifest = valid_manifest()
        marketplace = manifest["profiles"][0]  # type: ignore[index]
        marketplace.pop("activation_basis")

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "profiles[marketplace-intake].activation_basis is required for selected profiles",
            errors,
        )

    def test_reports_malformed_collection_values_without_crashing(self) -> None:
        manifest = valid_manifest()
        motion = manifest["motions"][0]  # type: ignore[index]
        motion["tags"] = [{"not": "a string"}]
        motion["classification_basis"] = [["not-hashable"]]
        motion["files"][0]["variant"] = {"not": "a string"}
        manifest["profiles"][0]["status"] = {"not": "a string"}  # type: ignore[index]
        manifest["pipeline_stages"][0]["status"] = {  # type: ignore[index]
            "not": "a string"
        }

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "motions[0].tags contains invalid tag: {'not': 'a string'}", errors
        )
        self.assertTrue(
            any("classification_basis contains unknown values" in error for error in errors)
        )
        self.assertIn(
            "motions[0].files[0].variant has unknown value: {'not': 'a string'}",
            errors,
        )
        self.assertTrue(
            any("status has unknown value" in error for error in errors), errors
        )

    def test_rejects_non_string_selected_profile_activation_basis(self) -> None:
        manifest = valid_manifest()
        marketplace = manifest["profiles"][0]  # type: ignore[index]
        marketplace["activation_basis"] = {"not": "a string"}

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "profiles[marketplace-intake].activation_basis is required for selected profiles",
            errors,
        )


class ReportValidatorTests(unittest.TestCase):
    def test_accepts_complete_report(self) -> None:
        self.assertEqual(report_validator.validate(valid_report()), [])

    def test_rejects_unresolved_template_placeholder(self) -> None:
        errors = report_validator.validate(valid_report() + "\n{{UNRESOLVED}}\n")

        self.assertIn("unresolved template placeholders: {{UNRESOLVED}}", errors)

    def test_requires_every_canonical_role(self) -> None:
        report = valid_report().replace("| `idle-pose` | 0 |\n", "")

        errors = report_validator.validate(report)

        self.assertIn("canonical role inventory is missing: idle-pose", errors)

    def test_requires_top_level_section_order(self) -> None:
        report = valid_report().replace(
            "## Reproduction appendix\nFixture result.\n\n## Sources",
            "## Sources\nFixture result.\n\n## Reproduction appendix",
        )

        errors = report_validator.validate(report)

        self.assertIn("required heading is out of order: ## Sources", errors)

    def test_prose_mention_does_not_satisfy_required_heading(self) -> None:
        report = valid_report().replace(
            "## Sources\nFixture result.",
            "This prose mentions ## Sources but is not a heading.",
        )

        errors = report_validator.validate(report)

        self.assertIn("missing required heading: ## Sources", errors)


if __name__ == "__main__":
    unittest.main()
