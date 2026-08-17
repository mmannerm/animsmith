#!/usr/bin/env python3
"""Behavioral tests for the animation-pack evaluation helper scripts."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path, PurePosixPath

import evaluation_contract_v1 as contract
import inventory_pack
import validate_evaluation_manifest as manifest_validator
import validate_report as report_validator


V1_SCHEMA = "urn:animsmith:skill:animation-pack-evaluation-manifest:1"
V1_PRIMARY_ROLES = (
    "idle-pose",
    "continuous-locomotion",
    "locomotion-transition",
    "airborne",
    "traversal",
    "action-interaction",
    "reaction-death",
    "emote-cinematic",
    "other-unknown",
)
V1_VARIANTS = {
    "in-place",
    "root-motion",
    "rotation-only-root",
    "single",
    "unknown",
}
V1_SET_TYPES = {
    "directional-blend",
    "speed-blend",
    "sync-group",
    "transition-chain",
    "mask-composition",
    "retarget-group",
    "paired-interaction",
    "motion-database",
    "other",
}
V1_CLASSIFICATION_BASES = {
    "user-required",
    "vendor-stated",
    "observed-file",
    "inferred",
}
V1_CONFIDENCE = {"high", "medium", "low"}
V1_PROFILE_ROWS = (
    ("marketplace-intake", "Marketplace intake"),
    ("blended-locomotion", "Blended locomotion"),
    ("root-motion-controller", "Root-motion controller"),
    ("state-machine-transitions", "State-machine transitions"),
    ("layered-upper-body-weapons", "Layered upper body/weapons"),
    ("traversal-environment", "Traversal/environment"),
    ("contact-actions-interactions", "Contact actions/interactions"),
    (
        "retargeted-customizable-characters",
        "Retargeted/customizable characters",
    ),
    ("motion-matching-search", "Motion matching/search"),
    ("networked-movement", "Networked movement"),
    ("runtime-performance", "Runtime performance"),
)
V1_PROFILE_IDS = tuple(identifier for identifier, _label in V1_PROFILE_ROWS)
V1_PROFILE_STATUSES = {"selected", "not-selected", "not-applicable"}
V1_ACTIVATION_BASES = {
    "user-required",
    "vendor-intended",
    "observed-pack-capability",
    "evaluator-selected-generic-scenario",
}
V1_PIPELINE_STAGE_ROWS = (
    ("acquire", "Acquire"),
    ("preserve-raw", "Preserve raw"),
    ("inspect", "Inspect"),
    ("segment", "Segment"),
    ("root-motion", "Root motion"),
    ("conform", "Conform"),
    ("validate", "Validate"),
    ("optimize", "Optimize"),
    ("export", "Export"),
    ("gate-report", "Gate/report"),
)
V1_PIPELINE_STAGES = tuple(
    identifier for identifier, _label in V1_PIPELINE_STAGE_ROWS
)
V1_COVERAGE_STATES = {
    "evaluated-clean",
    "evaluated-finding",
    "partially-evaluated",
    "not-applicable",
    "not-evaluated",
    "unsupported-input",
    "unavailable-evidence",
}
V1_PRIMARY_OWNERS = {
    "engine-config",
    "animsmith-current-safe",
    "animsmith-current-declared",
    "animsmith-future-candidate",
    "artist-author",
    "vendor-license",
    "unknown",
}
V1_REQUIRED_HEADINGS = (
    "## Executive decision",
    "## Evaluation scope and evidence",
    "## Pack inventory and content coverage",
    "## Out-of-the-box results",
    "## AnimSmith results",
    "## Engine integration",
    "## Blending, masking, and gameplay caveats",
    "## Compatibility",
    "## Issue and remediation register",
    "## Acquisition and adoption guidance",
    "## Limitations and unknowns",
    "## Reproduction appendix",
    "## Sources",
)
V1_REQUIRED_EXECUTIVE_HEADINGS = (
    "### Decision",
    "### Canonical clip-role inventory",
    "### Runtime-set inventory",
    "### Pipeline-stage coverage",
    "### Readiness ladder by clip set",
    "#### File-ready and clip-ready",
    "#### Set-ready and rig/use",
    "### Tooling frontier",
    "### Validation-profile status",
    "### Common-engine status",
    "### Best fit",
    "### Poor fit or material caveats",
    "### Adoption conditions",
)


def valid_manifest() -> dict[str, object]:
    roles = {
        role: {"logical_motions": 0, "delivered_files": 0}
        for role in V1_PRIMARY_ROLES
    }
    roles["idle-pose"] = {"logical_motions": 1, "delivered_files": 1}
    profiles = []
    for profile_id in V1_PROFILE_IDS:
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
        "schema": V1_SCHEMA,
        "taxonomy_version": "1",
        "validation_profile_set_version": "1",
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
            for stage_id in V1_PIPELINE_STAGES
        ],
        "role_totals": roles,
        "totals": {"logical_motions": 1, "delivered_files": 1, "runtime_sets": 0},
    }


def valid_manifest_with_runtime_set() -> dict[str, object]:
    manifest = valid_manifest()
    manifest["motions"].append(  # type: ignore[union-attr]
        {
            "id": "walk-forward",
            "vendor_label": "Walk Forward",
            "primary_role": "continuous-locomotion",
            "tags": ["direction:forward", "gait:walk"],
            "classification_basis": ["observed-file"],
            "files": [{"path": "Walk.fbx", "variant": "in-place"}],
        }
    )
    manifest["runtime_sets"] = [
        {
            "id": "idle-to-walk",
            "set_type": "transition-chain",
            "confidence": "high",
            "classification_basis": ["observed-file"],
            "members": [
                {"motion_id": "idle-neutral", "file": "Idle.fbx"},
                {"motion_id": "walk-forward", "file": "Walk.fbx"},
            ],
        }
    ]
    manifest["role_totals"]["continuous-locomotion"] = {  # type: ignore[index]
        "logical_motions": 1,
        "delivered_files": 1,
    }
    manifest["totals"] = {
        "logical_motions": 2,
        "delivered_files": 2,
        "runtime_sets": 1,
    }
    return manifest


def valid_report() -> str:
    role_rows = "\n".join(f"| `{role}` | 0 |" for role in V1_PRIMARY_ROLES)
    stage_rows = "\n".join(
        f"| {label} | evaluated-clean |" for _identifier, label in V1_PIPELINE_STAGE_ROWS
    )
    profile_rows = "\n".join(
        f"| {label} | not-selected |" for _identifier, label in V1_PROFILE_ROWS
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
Schema: `{V1_SCHEMA}`

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
| ID | Severity | Problem and impact | Primary owner | Current workaround | Future AnimSmith potential | Confidence/status |
|---|---|---|---|---|---|---|
| FIX-001 | Moderate | Fixture problem. | engine-config | Fixture workaround. | Not applicable. | High. |

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
            alpha = root / "alpha"
            zeta = root / "zeta"
            alpha.mkdir()
            zeta.mkdir()
            (alpha / "A.fbx").write_bytes(b"a")
            (zeta / "A.fbx").write_bytes(b"a")
            (alpha / "B.gltf").write_bytes(b"b")
            (zeta / "B.gltf").write_bytes(b"b")
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
                    "path": "alpha/A.fbx",
                    "type": "file",
                    "kind": "animsmith-input-candidate",
                    "size_bytes": 1,
                    "sha256": hashlib.sha256(b"a").hexdigest(),
                },
                {
                    "path": "alpha/B.gltf",
                    "type": "file",
                    "kind": "animsmith-input-candidate",
                    "size_bytes": 1,
                    "sha256": hashlib.sha256(b"b").hexdigest(),
                },
                {
                    "path": "zeta/A.fbx",
                    "type": "file",
                    "kind": "animsmith-input-candidate",
                    "size_bytes": 1,
                    "sha256": hashlib.sha256(b"a").hexdigest(),
                },
                {
                    "path": "zeta/B.gltf",
                    "type": "file",
                    "kind": "animsmith-input-candidate",
                    "size_bytes": 1,
                    "sha256": hashlib.sha256(b"b").hexdigest(),
                },
            ],
        )
        self.assertEqual(
            first["duplicate_file_groups"],
            [
                {
                    "sha256": hashlib.sha256(b"b").hexdigest(),
                    "paths": ["alpha/B.gltf", "zeta/B.gltf"],
                },
                {
                    "sha256": hashlib.sha256(b"a").hexdigest(),
                    "paths": ["alpha/A.fbx", "zeta/A.fbx"],
                },
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
    def test_catalog_retains_public_v1_vocabulary(self) -> None:
        self.assertEqual(contract.SCHEMA, V1_SCHEMA)
        self.assertEqual(contract.TAXONOMY_VERSION, "1")
        self.assertEqual(contract.PROFILE_SET_VERSION, "1")
        self.assertEqual(contract.PRIMARY_ROLES, V1_PRIMARY_ROLES)
        self.assertEqual(contract.VARIANTS, V1_VARIANTS)
        self.assertEqual(contract.SET_TYPES, V1_SET_TYPES)
        self.assertEqual(contract.CLASSIFICATION_BASES, V1_CLASSIFICATION_BASES)
        self.assertEqual(contract.CONFIDENCE, V1_CONFIDENCE)
        self.assertEqual(contract.PROFILE_ROWS, V1_PROFILE_ROWS)
        self.assertEqual(contract.PROFILE_IDS, V1_PROFILE_IDS)
        self.assertEqual(contract.PROFILE_STATUSES, V1_PROFILE_STATUSES)
        self.assertEqual(contract.ACTIVATION_BASES, V1_ACTIVATION_BASES)
        self.assertEqual(contract.PIPELINE_STAGE_ROWS, V1_PIPELINE_STAGE_ROWS)
        self.assertEqual(contract.PIPELINE_STAGES, V1_PIPELINE_STAGES)
        self.assertEqual(contract.COVERAGE_STATES, V1_COVERAGE_STATES)
        self.assertEqual(contract.PRIMARY_OWNERS, V1_PRIMARY_OWNERS)

    def test_accepts_every_primary_role(self) -> None:
        for role in V1_PRIMARY_ROLES:
            with self.subTest(role=role):
                manifest = valid_manifest()
                manifest["motions"][0]["primary_role"] = role  # type: ignore[index]
                manifest["role_totals"]["idle-pose"] = {  # type: ignore[index]
                    "logical_motions": 0,
                    "delivered_files": 0,
                }
                manifest["role_totals"][role] = {  # type: ignore[index]
                    "logical_motions": 1,
                    "delivered_files": 1,
                }
                self.assertEqual(manifest_validator.validate_manifest(manifest), [])

    def test_accepts_every_motion_and_runtime_set_enum(self) -> None:
        for variant in V1_VARIANTS:
            with self.subTest(variant=variant):
                manifest = valid_manifest_with_runtime_set()
                manifest["motions"][0]["files"][0]["variant"] = variant  # type: ignore[index]
                self.assertEqual(manifest_validator.validate_manifest(manifest), [])
        for set_type in V1_SET_TYPES:
            with self.subTest(set_type=set_type):
                manifest = valid_manifest_with_runtime_set()
                manifest["runtime_sets"][0]["set_type"] = set_type  # type: ignore[index]
                self.assertEqual(manifest_validator.validate_manifest(manifest), [])
        for confidence in V1_CONFIDENCE:
            with self.subTest(confidence=confidence):
                manifest = valid_manifest_with_runtime_set()
                manifest["runtime_sets"][0]["confidence"] = confidence  # type: ignore[index]
                self.assertEqual(manifest_validator.validate_manifest(manifest), [])
        for basis in V1_CLASSIFICATION_BASES:
            with self.subTest(motion_classification_basis=basis):
                manifest = valid_manifest_with_runtime_set()
                manifest["motions"][0]["classification_basis"] = [basis]  # type: ignore[index]
                self.assertEqual(manifest_validator.validate_manifest(manifest), [])
            with self.subTest(runtime_set_classification_basis=basis):
                manifest = valid_manifest_with_runtime_set()
                manifest["runtime_sets"][0]["classification_basis"] = [basis]  # type: ignore[index]
                self.assertEqual(manifest_validator.validate_manifest(manifest), [])

    def test_accepts_every_profile_and_pipeline_enum(self) -> None:
        for status in V1_PROFILE_STATUSES:
            with self.subTest(profile_status=status):
                manifest = valid_manifest()
                profile = manifest["profiles"][1]  # type: ignore[index]
                profile["status"] = status
                if status == "selected":
                    profile["activation_basis"] = "user-required"
                self.assertEqual(manifest_validator.validate_manifest(manifest), [])
        for basis in V1_ACTIVATION_BASES:
            with self.subTest(activation_basis=basis):
                manifest = valid_manifest()
                manifest["profiles"][0]["activation_basis"] = basis  # type: ignore[index]
                self.assertEqual(manifest_validator.validate_manifest(manifest), [])
        for state in V1_COVERAGE_STATES:
            with self.subTest(coverage_state=state):
                manifest = valid_manifest()
                manifest["pipeline_stages"][0]["status"] = state  # type: ignore[index]
                self.assertEqual(manifest_validator.validate_manifest(manifest), [])

    def test_accepts_complete_manifest(self) -> None:
        self.assertEqual(manifest_validator.validate_manifest(valid_manifest()), [])

    def test_accepts_analytic_runtime_set(self) -> None:
        self.assertEqual(
            manifest_validator.validate_manifest(valid_manifest_with_runtime_set()), []
        )

    def test_rejects_unknown_runtime_set_type(self) -> None:
        manifest = valid_manifest_with_runtime_set()
        manifest["runtime_sets"][0]["set_type"] = "unknown-type"  # type: ignore[index]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "runtime_sets[0].set_type has unknown value: 'unknown-type'", errors
        )

    def test_rejects_unknown_primary_role(self) -> None:
        manifest = valid_manifest()
        manifest["motions"][0]["primary_role"] = "unknown-role"  # type: ignore[index]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "motions[0].primary_role has unknown value: 'unknown-role'", errors
        )

    def test_rejects_unknown_runtime_set_confidence(self) -> None:
        manifest = valid_manifest_with_runtime_set()
        manifest["runtime_sets"][0]["confidence"] = "certain"  # type: ignore[index]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn("runtime_sets[0].confidence has unknown value: 'certain'", errors)

    def test_requires_two_runtime_set_members(self) -> None:
        manifest = valid_manifest_with_runtime_set()
        manifest["runtime_sets"][0]["members"] = [  # type: ignore[index]
            {"motion_id": "idle-neutral", "file": "Idle.fbx"}
        ]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn("runtime_sets[0].members must contain at least two members", errors)

    def test_rejects_unknown_runtime_set_motion_reference(self) -> None:
        manifest = valid_manifest_with_runtime_set()
        manifest["runtime_sets"][0]["members"][1]["motion_id"] = "missing"  # type: ignore[index]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "runtime_sets[0].members[1].motion_id references unknown motion", errors
        )

    def test_rejects_runtime_set_file_from_another_motion(self) -> None:
        manifest = valid_manifest_with_runtime_set()
        manifest["runtime_sets"][0]["members"][1]["file"] = "Idle.fbx"  # type: ignore[index]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "runtime_sets[0].members[1].file is not a delivered file of 'walk-forward'",
            errors,
        )

    def test_rejects_duplicate_runtime_set_members(self) -> None:
        manifest = valid_manifest_with_runtime_set()
        first_member = copy.deepcopy(manifest["runtime_sets"][0]["members"][0])  # type: ignore[index]
        manifest["runtime_sets"][0]["members"][1] = first_member  # type: ignore[index]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "runtime_sets[0].members[1] duplicates another member", errors
        )

    def test_runtime_set_total_must_reconcile(self) -> None:
        manifest = valid_manifest_with_runtime_set()
        manifest["totals"]["runtime_sets"] = 0  # type: ignore[index]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "totals does not match motions, physical files, and runtime sets", errors
        )

    def test_rejects_duplicate_physical_file_membership(self) -> None:
        manifest = valid_manifest()
        duplicate = copy.deepcopy(manifest["motions"][0])  # type: ignore[index]
        duplicate["id"] = "idle-duplicate"
        manifest["motions"].append(duplicate)  # type: ignore[union-attr]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn("physical file 'Idle.fbx' belongs to more than one motion", errors)

    def test_rejects_each_declared_total_that_does_not_reconcile(self) -> None:
        declared = {
            "logical_motions": 1,
            "delivered_files": 1,
            "runtime_sets": 0,
        }
        for field in declared:
            with self.subTest(field=field):
                manifest = valid_manifest()
                manifest["totals"][field] = declared[field] + 1  # type: ignore[index]

                errors = manifest_validator.validate_manifest(manifest)

                self.assertIn(
                    "totals does not match motions, physical files, and runtime sets",
                    errors,
                )

    def test_rejects_each_role_total_that_does_not_reconcile(self) -> None:
        for role in V1_PRIMARY_ROLES:
            for field in ("logical_motions", "delivered_files"):
                with self.subTest(role=role, field=field):
                    manifest = valid_manifest()
                    current = manifest["role_totals"][role][field]  # type: ignore[index]
                    manifest["role_totals"][role][field] = current + 1  # type: ignore[index,operator]

                    errors = manifest_validator.validate_manifest(manifest)

                    self.assertIn(
                        "role_totals does not match totals derived from motions", errors
                    )

    def test_validation_error_order_is_deterministic(self) -> None:
        manifest = valid_manifest()
        manifest["schema"] = "urn:wrong:schema"
        manifest["taxonomy_version"] = "wrong"
        manifest["role_totals"]["idle-pose"]["delivered_files"] = 2  # type: ignore[index]
        manifest["totals"]["logical_motions"] = 2  # type: ignore[index]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertEqual(
            errors,
            [
                f"schema must be {V1_SCHEMA!r}",
                "taxonomy_version must be '1'",
                "role_totals does not match totals derived from motions",
                "totals does not match motions, physical files, and runtime sets",
            ],
        )

    def test_rejects_missing_and_duplicate_required_profiles(self) -> None:
        for index, profile_id in enumerate(V1_PROFILE_IDS):
            with self.subTest(profile_id=profile_id, mutation="missing"):
                manifest = valid_manifest()
                manifest["profiles"].pop(index)  # type: ignore[union-attr]
                errors = manifest_validator.validate_manifest(manifest)
                self.assertIn(f"profiles is missing: {profile_id}", errors)
            with self.subTest(profile_id=profile_id, mutation="duplicate"):
                manifest = valid_manifest()
                duplicate = copy.deepcopy(manifest["profiles"][index])  # type: ignore[index]
                manifest["profiles"].append(duplicate)  # type: ignore[union-attr]
                errors = manifest_validator.validate_manifest(manifest)
                self.assertIn(f"duplicate profile_id: {profile_id}", errors)

    def test_rejects_missing_and_duplicate_required_pipeline_stages(self) -> None:
        for index, stage_id in enumerate(V1_PIPELINE_STAGES):
            with self.subTest(stage_id=stage_id, mutation="missing"):
                manifest = valid_manifest()
                manifest["pipeline_stages"].pop(index)  # type: ignore[union-attr]
                errors = manifest_validator.validate_manifest(manifest)
                self.assertIn(f"pipeline_stages is missing: {stage_id}", errors)
            with self.subTest(stage_id=stage_id, mutation="duplicate"):
                manifest = valid_manifest()
                duplicate = copy.deepcopy(manifest["pipeline_stages"][index])  # type: ignore[index]
                manifest["pipeline_stages"].append(duplicate)  # type: ignore[union-attr]
                errors = manifest_validator.validate_manifest(manifest)
                self.assertIn(f"duplicate stage_id: {stage_id}", errors)

    def test_rejects_non_array_top_level_collections(self) -> None:
        for field in ("motions", "runtime_sets", "profiles", "pipeline_stages"):
            with self.subTest(field=field):
                manifest = valid_manifest()
                manifest[field] = {}

                errors = manifest_validator.validate_manifest(manifest)

                self.assertIn(f"{field} must be an array", errors)

    def test_requires_selected_profile_activation_basis(self) -> None:
        manifest = valid_manifest()
        marketplace = manifest["profiles"][0]  # type: ignore[index]
        marketplace.pop("activation_basis")

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "profiles[marketplace-intake].activation_basis is required for selected profiles",
            errors,
        )

    def test_rejects_non_string_motion_tag(self) -> None:
        manifest = valid_manifest()
        motion = manifest["motions"][0]  # type: ignore[index]
        motion["tags"] = [{"not": "a string"}]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "motions[0].tags contains invalid tag: {'not': 'a string'}", errors
        )

    def test_rejects_non_string_classification_basis(self) -> None:
        manifest = valid_manifest()
        motion = manifest["motions"][0]  # type: ignore[index]
        motion["classification_basis"] = [["not-hashable"]]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "motions[0].classification_basis contains unknown values: ['not-hashable']",
            errors,
        )

    def test_rejects_non_string_file_variant(self) -> None:
        manifest = valid_manifest()
        motion = manifest["motions"][0]  # type: ignore[index]
        motion["files"][0]["variant"] = {"not": "a string"}

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "motions[0].files[0].variant has unknown value: {'not': 'a string'}",
            errors,
        )

    def test_rejects_non_string_profile_status(self) -> None:
        manifest = valid_manifest()
        manifest["profiles"][0]["status"] = {"not": "a string"}  # type: ignore[index]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "profiles[marketplace-intake].status has unknown value: {'not': 'a string'}",
            errors,
        )

    def test_rejects_non_string_pipeline_status(self) -> None:
        manifest = valid_manifest()
        manifest["pipeline_stages"][0]["status"] = {  # type: ignore[index]
            "not": "a string"
        }

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn(
            "pipeline_stages[acquire].status has unknown value: {'not': 'a string'}",
            errors,
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

    def test_accepts_every_primary_issue_owner(self) -> None:
        for owner in V1_PRIMARY_OWNERS:
            with self.subTest(owner=owner):
                report = valid_report().replace(
                    "| engine-config | Fixture workaround. |",
                    f"| {owner} | Fixture workaround. |",
                )
                self.assertEqual(report_validator.validate(report), [])

    def test_requires_every_top_level_heading(self) -> None:
        for heading in V1_REQUIRED_HEADINGS:
            with self.subTest(heading=heading):
                report = valid_report().replace(
                    f"{heading}\n", f"{heading} omitted\n", 1
                )

                errors = report_validator.validate(report)

                self.assertIn(f"missing required heading: {heading}", errors)

    def test_requires_every_executive_heading(self) -> None:
        for heading in V1_REQUIRED_EXECUTIVE_HEADINGS:
            with self.subTest(heading=heading):
                report = valid_report().replace(
                    f"{heading}\n", f"{heading} omitted\n", 1
                )

                errors = report_validator.validate(report)

                self.assertIn(
                    f"missing required executive heading: {heading}", errors
                )

    def test_requires_every_pipeline_stage_row(self) -> None:
        for _identifier, label in V1_PIPELINE_STAGE_ROWS:
            with self.subTest(label=label):
                report = valid_report().replace(
                    f"| {label} | evaluated-clean |\n", ""
                )

                errors = report_validator.validate(report)

                self.assertIn(f"pipeline-stage coverage is missing: {label}", errors)

    def test_requires_every_validation_profile_row(self) -> None:
        for _identifier, label in V1_PROFILE_ROWS:
            with self.subTest(label=label):
                report = valid_report().replace(f"| {label} | not-selected |\n", "")

                errors = report_validator.validate(report)

                self.assertIn(
                    f"validation-profile status is missing: {label}", errors
                )

    def test_requires_manifest_schema(self) -> None:
        report = valid_report().replace(V1_SCHEMA, "urn:wrong:schema")

        errors = report_validator.validate(report)

        self.assertIn(
            f"report must identify evaluation manifest schema: {V1_SCHEMA}", errors
        )

    def test_report_error_order_is_deterministic(self) -> None:
        report = valid_report().replace("## Sources\n", "## Sources omitted\n")
        report = report.replace(V1_SCHEMA, "urn:wrong:schema") + "\n{{UNRESOLVED}}\n"

        errors = report_validator.validate(report)

        self.assertEqual(
            errors,
            [
                "missing required heading: ## Sources",
                f"report must identify evaluation manifest schema: {V1_SCHEMA}",
                "unresolved template placeholders: {{UNRESOLVED}}",
            ],
        )

    def test_rejects_unresolved_template_placeholder(self) -> None:
        errors = report_validator.validate(valid_report() + "\n{{UNRESOLVED}}\n")

        self.assertIn("unresolved template placeholders: {{UNRESOLVED}}", errors)

    def test_requires_every_canonical_role(self) -> None:
        for role in V1_PRIMARY_ROLES:
            with self.subTest(role=role):
                report = valid_report().replace(f"| `{role}` | 0 |\n", "")

                errors = report_validator.validate(report)

                self.assertIn(f"canonical role inventory is missing: {role}", errors)

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

    def test_rejects_composite_primary_issue_owner(self) -> None:
        report = valid_report().replace(
            "| engine-config | Fixture workaround. |",
            "| vendor-license / artist-author | Fixture workaround. |",
        )

        errors = report_validator.validate(report)

        self.assertIn(
            "issue FIX-001 has unknown or composite primary owner: "
            "'vendor-license / artist-author'",
            errors,
        )

    def test_rejects_malformed_issue_table_row(self) -> None:
        report = valid_report().replace(
            "| engine-config | Fixture workaround. |",
            "| engine-config | unexpected | Fixture workaround. |",
        )

        errors = report_validator.validate(report)

        self.assertIn(
            "issue FIX-001 row must contain exactly seven cells", errors
        )

    def test_all_published_pack_reports_conform(self) -> None:
        repository = Path(__file__).resolve().parents[4]
        reports = sorted(
            report
            for report in (repository / "docs" / "reports").glob("*.md")
            if report.name != "README.md"
        )
        self.assertTrue(reports, "expected at least one published pack report")
        for report in reports:
            with self.subTest(report=report.name):
                self.assertEqual(
                    report_validator.validate(report.read_text(encoding="utf-8")), []
                )


class ExecutableContractTests(unittest.TestCase):
    scripts = Path(__file__).resolve().parent

    def run_script(
        self, name: str, *arguments: str, hash_seed: str | None = None
    ) -> subprocess.CompletedProcess[str]:
        environment = None
        if hash_seed is not None:
            environment = dict(os.environ, PYTHONHASHSEED=hash_seed)
        return subprocess.run(
            [sys.executable, str(self.scripts / name), *arguments],
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )

    def test_inventory_cli_success_and_missing_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory) / "pack"
            root.mkdir()
            (root / "Walk.fbx").write_bytes(b"motion")
            success = self.run_script(
                "inventory_pack.py", str(root), "--label", "Pack", hash_seed="1"
            )
            repeated = self.run_script(
                "inventory_pack.py", str(root), "--label", "Pack", hash_seed="2"
            )
            missing = self.run_script("inventory_pack.py", str(root / "missing"))

        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertEqual(repeated.returncode, 0, repeated.stderr)
        self.assertEqual(success.stdout, repeated.stdout)
        self.assertEqual(json.loads(success.stdout)["pack_label"], "Pack")
        self.assertEqual(success.stderr, "")
        self.assertEqual(missing.returncode, 2)
        self.assertIn("root is not a directory", missing.stderr)

    def test_manifest_validator_cli_success_and_malformed_json(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            valid_path = directory / "valid.json"
            valid_path.write_text(json.dumps(valid_manifest()), encoding="utf-8")
            malformed_path = directory / "malformed.json"
            malformed_path.write_text("{", encoding="utf-8")
            success = self.run_script(
                "validate_evaluation_manifest.py", str(valid_path)
            )
            malformed = self.run_script(
                "validate_evaluation_manifest.py", str(malformed_path)
            )

        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertIn("validated animation-pack evaluation manifest", success.stdout)
        self.assertEqual(success.stderr, "")
        self.assertEqual(malformed.returncode, 2)
        self.assertIn("validate_evaluation_manifest.py:", malformed.stderr)

    def test_report_validator_cli_success_and_invalid_report(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            valid_path = directory / "valid.md"
            valid_path.write_text(valid_report(), encoding="utf-8")
            invalid_path = directory / "invalid.md"
            invalid_path.write_text("# incomplete\n", encoding="utf-8")
            success = self.run_script("validate_report.py", str(valid_path))
            invalid = self.run_script("validate_report.py", str(invalid_path))

        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertIn("validated animation-pack report", success.stdout)
        self.assertEqual(success.stderr, "")
        self.assertEqual(invalid.returncode, 1)
        self.assertIn("missing required heading: ## Executive decision", invalid.stderr)


if __name__ == "__main__":
    unittest.main()
