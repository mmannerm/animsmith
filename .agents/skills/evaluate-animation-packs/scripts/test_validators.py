#!/usr/bin/env python3
"""Behavioral tests for the animation-pack evaluation helper scripts."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest import mock
from pathlib import Path, PurePosixPath

import yaml
import jsonschema

import evaluation_contract_v1 as contract
import inventory_pack
import validate_evaluation_manifest as manifest_validator
import validate_report as report_validator
import evaluation_model_v1 as model_contract
import validate_evaluation_model as model_validator
import render_evaluation_model as model_renderer


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
V1_TECHNICAL_VERDICTS = {
    "Usable",
    "Usable with conditions",
    "Restricted use",
    "Poor fit",
    "Insufficient technical evidence",
}
V1_EVALUATION_COMPLETENESS = {"complete", "partial", "preview-only"}
V1_ISSUE_SEVERITIES = {"blocker", "major", "moderate", "minor", "note"}
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
    "## Technical decision",
    "## Capability coverage",
    "## Runtime sets and authored motion",
    "## Integration recipe",
    "## Technical issue register",
    "## Engine status",
    "## Fit and limitations",
    "## Evidence status",
    "## Sources",
)
V1_REQUIRED_CAPABILITY_HEADINGS = (
    "### Complete core",
    "### Partial supporting gameplay",
    "### Absent",
)
V1_REQUIRED_APPENDIX_HEADINGS = (
    "## Evaluation scope and provenance",
    "## Evaluation manifest and taxonomy",
    "## Pack inventory and content evidence",
    "## Mechanical baseline",
    "## AnimSmith remediation evidence",
    "## Engine procedures and evidence",
    "## Rig, masking, and compatibility evidence",
    "## Limitations and unknowns",
    "## Reproduction",
    "## Sources",
)
V1_REQUIRED_APPENDIX_MANIFEST_HEADINGS = (
    "### Canonical clip-role inventory",
    "### Runtime-set inventory",
    "### Pipeline-stage coverage",
    "### Readiness evidence by clip set",
    "### Validation-profile status",
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


def valid_manifest_with_multiple_counts() -> dict[str, object]:
    manifest = valid_manifest_with_runtime_set()
    manifest["motions"].append(  # type: ignore[union-attr]
        {
            "id": "walk-backward",
            "vendor_label": "Walk Backward",
            "primary_role": "continuous-locomotion",
            "tags": ["direction:backward", "gait:walk"],
            "classification_basis": ["observed-file"],
            "files": [
                {"path": "WalkBack.fbx", "variant": "in-place"},
                {"path": "WalkBack_RM.fbx", "variant": "root-motion"},
            ],
        }
    )
    manifest["runtime_sets"].append(  # type: ignore[union-attr]
        {
            "id": "locomotion-sync",
            "set_type": "sync-group",
            "confidence": "medium",
            "classification_basis": ["vendor-stated"],
            "members": [
                {"motion_id": "walk-forward", "file": "Walk.fbx"},
                {"motion_id": "walk-backward", "file": "WalkBack.fbx"},
            ],
        }
    )
    manifest["role_totals"]["continuous-locomotion"] = {  # type: ignore[index]
        "logical_motions": 2,
        "delivered_files": 3,
    }
    manifest["totals"] = {
        "logical_motions": 3,
        "delivered_files": 4,
        "runtime_sets": 2,
    }
    return manifest


def assign_path(
    document: dict[str, object], path: tuple[str | int, ...], value: object
) -> None:
    target: object = document
    for component in path[:-1]:
        target = target[component]  # type: ignore[index]
    target[path[-1]] = value  # type: ignore[index]


def valid_report() -> str:
    return f"""# Animation pack evaluation: Validator fixture

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — fixture boundary.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-16**
>
> Report format: **1**
>
> Detailed evidence: [fixture evidence appendix](fixture-evidence.md)

## Technical decision
Fixture decision.

## Capability coverage

### Complete core
Fixture capability.

### Partial supporting gameplay
Fixture capability.

### Absent
Fixture capability.

## Runtime sets and authored motion
| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Walk | F `(0,1)` | IP `Walk.fbx`; RM `Walk_RM.fbx` | variant=paired-ip-rm | duration=1.0 s; rm_speed=1.0 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |

## Integration recipe
1. **Members/topology:** `topology=2d-blend`; fixture coordinates `(0,1)`.
2. **Timing/synchronization:** `sync=gait-phase`; fixture loop policy.
3. **State ownership:** `owner=gameplay-controller`; fixture movement policy.
4. **Composition constraints:** `composition=separate-variants`; fixture limits.
5. **Acceptance gate:** `gate=target-character-review`; fixture visual gate.

## Technical issue register
| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| FIX-001 | moderate | [Fixture problem.](../game-ready-clips.md#the-loop-pops) | engine-config | Fixture workaround. | Not applicable. | High. |

## Engine status
| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity | not evaluated | Fixture. | Fixture. |
| Unreal Engine | not evaluated | Fixture. | Fixture. |
| Godot | not evaluated | Fixture. | Fixture. |
| Bevy | not evaluated | Fixture. | Fixture. |

## Fit and limitations
Fixture result.

## Evidence status
Schema: `{V1_SCHEMA}`. See the
[canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder).

## Sources
Fixture result.
"""


def valid_appendix() -> str:
    role_rows = "\n".join(
        f"| `{role}` | 0 | 0 | Fixture. |" for role in V1_PRIMARY_ROLES
    )
    stage_rows = "\n".join(
        f"| {label} | `evaluated-clean` | Fixture. |"
        for _identifier, label in V1_PIPELINE_STAGE_ROWS
    )
    profile_rows = "\n".join(
        f"| {label} | `not-selected` | Fixture. |"
        for _identifier, label in V1_PROFILE_ROWS
    )
    return f"""# Animation pack evidence appendix: Validator fixture

> Companion report: [technical evaluation](fixture.md)
>
> Evidence status: **partial** — fixture boundary.
>
> Evaluation date: **2026-08-16**
>
> Report format: **1**

The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder)
is authoritative.

## Evaluation scope and provenance
Schema: `{V1_SCHEMA}`.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory
| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
{role_rows}
| **Total** | **0** | **0** | Fixture. |

### Runtime-set inventory
| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Walk | directional-blend | IP/RM pair | Fixture. | Fixture. |

### Pipeline-stage coverage
| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
{stage_rows}

### Readiness evidence by clip set
Fixture result.

### Validation-profile status
| Validation profile | Selection | Result / next evidence |
|---|---|---|
{profile_rows}

## Pack inventory and content evidence
Fixture result.

## Mechanical baseline
Fixture result.

## AnimSmith remediation evidence
Fixture result.

## Engine procedures and evidence
Fixture result.

## Rig, masking, and compatibility evidence
Fixture result.

## Limitations and unknowns
Fixture result.

## Reproduction
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

    def test_inventory_emits_every_documented_file_kind(self) -> None:
        expected = {
            "Animation.bvh": "animation-source",
            "Clip.fbx": "animsmith-input-candidate",
            "Data.bin": "other",
            "LICENSE.bin": "license-or-terms",
            "Manual.pdf": "documentation",
            "Native.unitypackage": "engine-native",
            "Source.zip": "archive",
        }
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory) / "pack"
            root.mkdir()
            for name in expected:
                (root / name).write_bytes(name.encode())

            result = inventory_pack.inventory(root, "Fixture", set(), False)

        actual = {
            record["path"]: record["kind"]
            for record in result["files"]
            if record["type"] == "file"
        }
        self.assertEqual(actual, expected)
        self.assertIsNone(result["hash_algorithm"])
        self.assertEqual(result["duplicate_file_groups"], [])
        self.assertTrue(all("sha256" not in record for record in result["files"]))

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

    def test_accepts_multiple_motions_files_and_runtime_sets(self) -> None:
        self.assertEqual(
            manifest_validator.validate_manifest(valid_manifest_with_multiple_counts()),
            [],
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

    def test_rejects_unknown_values_for_every_manifest_enum(self) -> None:
        cases = (
            (
                "variant",
                lambda manifest: manifest["motions"][0]["files"][0].__setitem__(  # type: ignore[index]
                    "variant", "unknown-enum"
                ),
                "motions[0].files[0].variant has unknown value: 'unknown-enum'",
            ),
            (
                "motion-classification-basis",
                lambda manifest: manifest["motions"][0].__setitem__(  # type: ignore[index]
                    "classification_basis", ["unknown-enum"]
                ),
                "motions[0].classification_basis contains unknown values: 'unknown-enum'",
            ),
            (
                "runtime-set-classification-basis",
                lambda manifest: manifest["runtime_sets"][0].__setitem__(  # type: ignore[index]
                    "classification_basis", ["unknown-enum"]
                ),
                "runtime_sets[0].classification_basis contains unknown values: 'unknown-enum'",
            ),
            (
                "profile-status",
                lambda manifest: manifest["profiles"][1].__setitem__(  # type: ignore[index]
                    "status", "unknown-enum"
                ),
                "profiles[blended-locomotion].status has unknown value: 'unknown-enum'",
            ),
            (
                "activation-basis",
                lambda manifest: manifest["profiles"][0].__setitem__(  # type: ignore[index]
                    "activation_basis", "unknown-enum"
                ),
                "profiles[marketplace-intake].activation_basis is required for selected profiles",
            ),
            (
                "pipeline-state",
                lambda manifest: manifest["pipeline_stages"][0].__setitem__(  # type: ignore[index]
                    "status", "unknown-enum"
                ),
                "pipeline_stages[acquire].status has unknown value: 'unknown-enum'",
            ),
        )
        for name, mutate, expected in cases:
            with self.subTest(enum=name):
                manifest = valid_manifest_with_runtime_set()
                mutate(manifest)
                self.assertIn(expected, manifest_validator.validate_manifest(manifest))

    def test_rejects_non_string_values_for_every_manifest_enum(self) -> None:
        cases = (
            (
                "primary-role",
                ("motions", 0, "primary_role"),
                False,
                "motions[0].primary_role has unknown value:",
            ),
            (
                "variant",
                ("motions", 0, "files", 0, "variant"),
                False,
                "motions[0].files[0].variant has unknown value:",
            ),
            (
                "set-type",
                ("runtime_sets", 0, "set_type"),
                False,
                "runtime_sets[0].set_type has unknown value:",
            ),
            (
                "confidence",
                ("runtime_sets", 0, "confidence"),
                False,
                "runtime_sets[0].confidence has unknown value:",
            ),
            (
                "motion-classification-basis",
                ("motions", 0, "classification_basis"),
                True,
                "motions[0].classification_basis contains unknown values:",
            ),
            (
                "runtime-set-classification-basis",
                ("runtime_sets", 0, "classification_basis"),
                True,
                "runtime_sets[0].classification_basis contains unknown values:",
            ),
            (
                "profile-status",
                ("profiles", 1, "status"),
                False,
                "profiles[blended-locomotion].status has unknown value:",
            ),
            (
                "activation-basis",
                ("profiles", 0, "activation_basis"),
                False,
                "profiles[marketplace-intake].activation_basis is required",
            ),
            (
                "pipeline-state",
                ("pipeline_stages", 0, "status"),
                False,
                "pipeline_stages[acquire].status has unknown value:",
            ),
        )
        for name, path, wrap_in_array, expected in cases:
            for invalid in (None, 7, True, {}, []):
                with self.subTest(enum=name, invalid=repr(invalid)):
                    manifest = valid_manifest_with_runtime_set()
                    value = [invalid] if wrap_in_array else invalid
                    assign_path(manifest, path, value)
                    self.assertTrue(
                        any(
                            error.startswith(expected)
                            for error in manifest_validator.validate_manifest(manifest)
                        )
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
            for delta in (-1, 1):
                with self.subTest(field=field, delta=delta):
                    manifest = valid_manifest()
                    manifest["totals"][field] = declared[field] + delta  # type: ignore[index]

                    errors = manifest_validator.validate_manifest(manifest)

                    self.assertIn(
                        "totals does not match motions, physical files, and runtime sets",
                        errors,
                    )

    def test_rejects_each_role_total_that_does_not_reconcile(self) -> None:
        for role in V1_PRIMARY_ROLES:
            for field in ("logical_motions", "delivered_files"):
                for delta in (-1, 1):
                    with self.subTest(role=role, field=field, delta=delta):
                        manifest = valid_manifest()
                        current = manifest["role_totals"][role][field]  # type: ignore[index]
                        manifest["role_totals"][role][field] = current + delta  # type: ignore[index,operator]

                        errors = manifest_validator.validate_manifest(manifest)

                        self.assertIn(
                            "role_totals does not match totals derived from motions",
                            errors,
                        )

    def test_rejects_malformed_scalar_for_every_declared_count(self) -> None:
        paths = [
            ("role_totals", role, field)
            for role in V1_PRIMARY_ROLES
            for field in ("logical_motions", "delivered_files")
        ] + [
            ("totals", field)
            for field in ("logical_motions", "delivered_files", "runtime_sets")
        ]
        for path in paths:
            manifest = valid_manifest_with_multiple_counts()
            declared: object = manifest
            for component in path:
                declared = declared[component]  # type: ignore[index]
            invalid_values = (
                bool(declared),
                float(declared),  # type: ignore[arg-type]
                1.5,
                -1,
                None,
                "1",
                {},
                [],
            )
            for invalid in invalid_values:
                with self.subTest(path=".".join(path), invalid=repr(invalid)):
                    manifest = valid_manifest_with_multiple_counts()
                    assign_path(manifest, path, invalid)

                    errors = manifest_validator.validate_manifest(manifest)

                    if path[0] == "role_totals":
                        error_path = f"role_totals[{path[1]}].{path[2]}"
                    else:
                        error_path = f"totals.{path[1]}"
                    self.assertIn(
                        f"{error_path} must be a non-negative integer", errors
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

    def test_multiple_missing_identifier_errors_are_deterministic(self) -> None:
        manifest = valid_manifest()
        manifest["profiles"] = manifest["profiles"][2:]  # type: ignore[index]
        manifest["pipeline_stages"] = manifest["pipeline_stages"][2:]  # type: ignore[index]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertEqual(
            errors,
            [
                "profiles is missing: blended-locomotion, marketplace-intake",
                "pipeline_stages is missing: acquire, preserve-raw",
            ],
        )

    def test_multiple_unknown_choice_errors_are_deterministic(self) -> None:
        manifest = valid_manifest()
        manifest["motions"][0]["classification_basis"] = [  # type: ignore[index]
            "z-unknown",
            "a-unknown",
        ]

        errors = manifest_validator.validate_manifest(manifest)

        self.assertEqual(
            errors,
            [
                "motions[0].classification_basis contains unknown values: "
                "'a-unknown', 'z-unknown'"
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

    def test_rejects_unknown_profile_and_pipeline_stage_identifiers(self) -> None:
        manifest = valid_manifest()
        manifest["profiles"].append(  # type: ignore[union-attr]
            {
                "profile_id": "unknown-profile",
                "status": "not-selected",
                "rationale": "Fixture.",
            }
        )
        manifest["pipeline_stages"].append(  # type: ignore[union-attr]
            {
                "stage_id": "unknown-stage",
                "status": "not-evaluated",
                "evidence": "Fixture.",
            }
        )

        errors = manifest_validator.validate_manifest(manifest)

        self.assertIn("profiles contains unknown identifiers: unknown-profile", errors)
        self.assertIn(
            "pipeline_stages contains unknown identifiers: unknown-stage", errors
        )

    def test_rejects_non_array_top_level_collections(self) -> None:
        for field in ("motions", "runtime_sets", "profiles", "pipeline_stages"):
            for invalid in (None, 7, True, "not-an-array", {}):
                with self.subTest(field=field, invalid=repr(invalid)):
                    manifest = valid_manifest()
                    manifest[field] = invalid

                    errors = manifest_validator.validate_manifest(manifest)

                    self.assertIn(f"{field} must be an array", errors)

    def test_rejects_non_array_nested_collections(self) -> None:
        cases = (
            (
                "motion-tags",
                ("motions", 0, "tags"),
                "motions[0].tags must be an array",
            ),
            (
                "motion-classification-basis",
                ("motions", 0, "classification_basis"),
                "motions[0].classification_basis must be a non-empty array",
            ),
            (
                "motion-files",
                ("motions", 0, "files"),
                "motions[0].files must be a non-empty array",
            ),
            (
                "runtime-set-classification-basis",
                ("runtime_sets", 0, "classification_basis"),
                "runtime_sets[0].classification_basis must be a non-empty array",
            ),
            (
                "runtime-set-members",
                ("runtime_sets", 0, "members"),
                "runtime_sets[0].members must contain at least two members",
            ),
        )
        for name, path, expected in cases:
            for invalid in (None, 7, True, "not-an-array", {}):
                with self.subTest(collection=name, invalid=repr(invalid)):
                    manifest = valid_manifest_with_runtime_set()
                    assign_path(manifest, path, invalid)
                    self.assertIn(
                        expected, manifest_validator.validate_manifest(manifest)
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
    def test_accepts_complete_pair(self) -> None:
        self.assertEqual(report_validator.validate(valid_report()), [])
        self.assertEqual(report_validator.validate_appendix(valid_appendix()), [])
        self.assertEqual(
            report_validator.validate_pair(
                valid_report(),
                valid_appendix(),
                "fixture.md",
                "fixture-evidence.md",
            ),
            [],
        )

    def test_structural_contract_ignores_fenced_and_commented_markdown(self) -> None:
        def hidden(document: str, opener: str, closer: str) -> str:
            first, remainder = document.split("\n", 1)
            return f"{first}\n{opener}\n{remainder}\n{closer}\n"

        for opener, closer in (("```markdown", "```"), ("<!--", "-->")):
            with self.subTest(opener=opener):
                report = hidden(valid_report(), opener, closer)
                appendix = hidden(valid_appendix(), opener, closer)
                self.assertNotEqual(report_validator.validate(report), [])
                self.assertNotEqual(report_validator.validate_appendix(appendix), [])
                self.assertNotEqual(
                    report_validator.validate_pair(
                        report,
                        appendix,
                        "fixture.md",
                        "fixture-evidence.md",
                    ),
                    [],
                )

        for opener, false_closer, closer in (
            ("````markdown", "```", "````"),
            ("```markdown", "``` trailing-text", "```"),
            ("```markdown", "    ```", "```"),
        ):
            with self.subTest(opener=opener, false_closer=false_closer):
                first, report_body = valid_report().split("\n", 1)
                _, appendix_body = valid_appendix().split("\n", 1)
                report = (
                    f"{first}\n{opener}\n{false_closer}\n{report_body}\n{closer}\n"
                )
                appendix = (
                    "# Animation pack evidence appendix: Validator fixture\n"
                    f"{opener}\n{false_closer}\n{appendix_body}\n{closer}\n"
                )
                self.assertNotEqual(report_validator.validate(report), [])
                self.assertNotEqual(report_validator.validate_appendix(appendix), [])
                self.assertNotEqual(
                    report_validator.validate_pair(
                        report,
                        appendix,
                        "fixture.md",
                        "fixture-evidence.md",
                    ),
                    [],
                )

        for document, validator in (
            (valid_report(), report_validator.validate),
            (valid_appendix(), report_validator.validate_appendix),
        ):
            with self.subTest(unclosed_comment=validator.__name__):
                first, body = document.split("\n", 1)
                self.assertNotEqual(validator(f"{first}\n<!--\n{body}"), [])

        report_first, report_body = valid_report().split("\n", 1)
        appendix_first, appendix_body = valid_appendix().split("\n", 1)
        self.assertNotEqual(
            report_validator.validate_pair(
                f"{report_first}\n<!--\n{report_body}",
                f"{appendix_first}\n<!--\n{appendix_body}",
                "fixture.md",
                "fixture-evidence.md",
            ),
            [],
        )

    def test_report_and_appendix_h1_must_be_the_first_rendered_block(self) -> None:
        self.assertIn(
            "report must start with '# Animation pack evaluation:'",
            report_validator.validate("Visible preface.\n\n" + valid_report()),
        )
        self.assertIn(
            "appendix must start with '# Animation pack evidence appendix:'",
            report_validator.validate_appendix(
                "Visible preface.\n\n" + valid_appendix()
            ),
        )
        self.assertIn(
            "report must not contain raw HTML",
            report_validator.validate("<!-- invisible -->\n" + valid_report()),
        )
        self.assertIn(
            "appendix must not contain raw HTML",
            report_validator.validate_appendix(
                "<!-- invisible -->\n" + valid_appendix()
            ),
        )

    def test_accepts_every_primary_issue_owner(self) -> None:
        for owner in V1_PRIMARY_OWNERS:
            with self.subTest(owner=owner):
                report = valid_report().replace(
                    "| engine-config | Fixture workaround. |",
                    f"| {owner} | Fixture workaround. |",
                )
                self.assertEqual(report_validator.validate(report), [])

    def test_requires_every_primary_heading(self) -> None:
        for heading in V1_REQUIRED_HEADINGS:
            with self.subTest(heading=heading):
                report = valid_report().replace(
                    f"{heading}\n", f"{heading} omitted\n", 1
                )
                self.assertIn(
                    f"missing required heading: {heading}",
                    report_validator.validate(report),
                )

    def test_requires_every_capability_heading(self) -> None:
        for heading in V1_REQUIRED_CAPABILITY_HEADINGS:
            with self.subTest(heading=heading):
                report = valid_report().replace(
                    f"{heading}\n", f"{heading} omitted\n", 1
                )
                self.assertIn(
                    f"missing required heading: {heading}",
                    report_validator.validate(report),
                )

    def test_required_report_sections_and_subsections_have_rendered_bodies(self) -> None:
        def empty_body(document: str, marker: str, next_markers: tuple[str, ...]) -> str:
            start = document.index(marker) + len(marker)
            candidates = [
                position
                for next_marker in next_markers
                if (position := document.find(next_marker, start)) >= 0
            ]
            end = min(candidates, default=len(document))
            return document[:start] + document[end:]

        for section in report_validator.PRIMARY_HEADINGS:
            with self.subTest(section=section):
                report = empty_body(valid_report(), f"## {section}\n", ("\n## ",))
                self.assertIn(
                    f"required section is empty: ## {section}",
                    report_validator.validate(report),
                )
        for subsection in report_validator.CAPABILITY_HEADINGS:
            with self.subTest(subsection=subsection):
                report = empty_body(
                    valid_report(), f"### {subsection}\n", ("\n### ", "\n## ")
                )
                self.assertIn(
                    f"required subsection is empty: ### {subsection}",
                    report_validator.validate(report),
                )

    def test_required_appendix_sections_and_subsections_have_rendered_bodies(self) -> None:
        def empty_body(document: str, marker: str, next_markers: tuple[str, ...]) -> str:
            start = document.index(marker) + len(marker)
            candidates = [
                position
                for next_marker in next_markers
                if (position := document.find(next_marker, start)) >= 0
            ]
            end = min(candidates, default=len(document))
            return document[:start] + document[end:]

        for section in report_validator.APPENDIX_HEADINGS:
            with self.subTest(section=section):
                appendix = empty_body(
                    valid_appendix(), f"## {section}\n", ("\n## ",)
                )
                self.assertIn(
                    f"required section is empty: ## {section}",
                    report_validator.validate_appendix(appendix),
                )
        for subsection in report_validator.APPENDIX_MANIFEST_HEADINGS:
            with self.subTest(subsection=subsection):
                appendix = empty_body(
                    valid_appendix(), f"### {subsection}\n", ("\n### ", "\n## ")
                )
                self.assertIn(
                    f"required subsection is empty: ### {subsection}",
                    report_validator.validate_appendix(appendix),
                )

    def test_noncontent_blocks_do_not_satisfy_required_section_bodies(self) -> None:
        cases = (
            (
                valid_report(),
                "## Technical decision\n",
                "\n## Capability coverage",
                "required section is empty: ## Technical decision",
                report_validator.validate,
            ),
            (
                valid_appendix(),
                "## Mechanical baseline\n",
                "\n## AnimSmith remediation evidence",
                "required section is empty: ## Mechanical baseline",
                report_validator.validate_appendix,
            ),
        )
        for document, start_marker, end_marker, expected, validator in cases:
            start = document.index(start_marker) + len(start_marker)
            end = document.index(end_marker, start)
            for replacement in ("---\n", "```text\n```\n", "#### Placeholder\n"):
                with self.subTest(start_marker=start_marker, replacement=replacement):
                    mutated = document[:start] + replacement + document[end:]
                    self.assertIn(expected, validator(mutated))

    def test_rejects_out_of_order_required_headings(self) -> None:
        heading_groups = (
            V1_REQUIRED_HEADINGS,
            V1_REQUIRED_CAPABILITY_HEADINGS,
        )
        for headings in heading_groups:
            for first, second in zip(headings, headings[1:]):
                with self.subTest(first=first, second=second):
                    report = valid_report()
                    first_position = report.index(first)
                    second_position = report.index(second)
                    report = (
                        report[:first_position]
                        + second
                        + report[first_position + len(first) : second_position]
                        + first
                        + report[second_position + len(second) :]
                    )
                    errors = report_validator.validate(report)
                    self.assertTrue(
                        any("out of order" in error for error in errors), errors
                    )

    def test_required_headings_are_unique(self) -> None:
        report = valid_report() + "\n## Technical decision\nOpposite decision.\n"
        self.assertIn(
            "required heading appears multiple times: ## Technical decision",
            report_validator.validate(report),
        )
        appendix = (
            valid_appendix()
            + "\n## Evaluation scope and provenance\nContradictory provenance.\n"
        )
        self.assertIn(
            "required heading appears multiple times: ## Evaluation scope and provenance",
            report_validator.validate_appendix(appendix),
        )

    def test_capability_subsections_must_stay_in_capability_section(self) -> None:
        report = valid_report()
        start = report.index("### Complete core")
        end = report.index("## Runtime sets and authored motion")
        relocated = report[start:end]
        report = report[:start] + report[end:] + "\n" + relocated
        self.assertIn(
            "missing required heading: ### Complete core",
            report_validator.validate(report),
        )

    def test_requires_verdict_and_completeness(self) -> None:
        report = valid_report().replace(
            "> Technical verdict:", "> Verdict:"
        ).replace(
            "> Evaluation completeness:", "> Completeness:"
        )
        errors = report_validator.validate(report)
        self.assertIn("report must declare a bold Technical verdict", errors)
        self.assertIn("report must declare bold Evaluation completeness", errors)

        suffixed = valid_report().replace(
            "**partial** — fixture boundary.", "**partial** contradictory", 1
        )
        self.assertIn(
            "report must declare bold Evaluation completeness",
            report_validator.validate(suffixed),
        )

    def test_report_metadata_declarations_are_unique(self) -> None:
        report = valid_report().replace(
            "> Technical verdict: **Usable with conditions**",
            "> Technical verdict: **Usable with conditions**\n>\n"
            "> Technical verdict: **Poor fit**",
            1,
        )
        self.assertIn(
            "report must declare a bold Technical verdict",
            report_validator.validate(report),
        )

    def test_required_metadata_must_stay_in_the_document_preamble(self) -> None:
        report = valid_report()
        start = report.index("> Technical verdict:")
        end = report.index("\n\n## Technical decision", start)
        metadata = report[start:end]
        relocated = report[:start] + report[end:] + "\n\n" + metadata + "\n"
        errors = report_validator.validate(relocated)
        self.assertIn("report must declare a bold Technical verdict", errors)
        self.assertIn("report must declare bold Evaluation completeness", errors)
        self.assertIn("report must declare one canonical bold Confidence", errors)

        appendix = valid_appendix()
        start = appendix.index("> Companion report:")
        end = appendix.index("\n\nThe [canonical readiness ladder]", start)
        metadata = appendix[start:end]
        relocated = appendix[:start] + appendix[end:] + "\n\n" + metadata + "\n"
        errors = report_validator.validate_appendix(relocated)
        self.assertIn("appendix must declare one canonical bold Evidence status", errors)
        self.assertIn("appendix must declare Report format 1", errors)

    def test_requires_confidence_evidence_status_and_evaluation_dates(self) -> None:
        report = valid_report().replace("> Confidence: **medium**\n>\n", "", 1)
        self.assertIn(
            "report must declare one canonical bold Confidence",
            report_validator.validate(report),
        )
        report = valid_report().replace("**2026-08-16**", "**2026-8-16**", 1)
        self.assertIn(
            "report must declare a bold YYYY-MM-DD Evaluation date",
            report_validator.validate(report),
        )
        appendix = valid_appendix().replace(
            "> Evidence status: **partial** — fixture boundary.\n>\n", "", 1
        )
        self.assertIn(
            "appendix must declare one canonical bold Evidence status",
            report_validator.validate_appendix(appendix),
        )
        appendix = valid_appendix().replace("**2026-08-16**", "**not-a-date**", 1)
        self.assertIn(
            "appendix must declare a bold YYYY-MM-DD Evaluation date",
            report_validator.validate_appendix(appendix),
        )

    def test_enforces_report_decision_vocabularies(self) -> None:
        self.assertEqual(report_validator.TECHNICAL_VERDICTS, V1_TECHNICAL_VERDICTS)
        self.assertEqual(
            report_validator.EVALUATION_COMPLETENESS,
            V1_EVALUATION_COMPLETENESS,
        )
        self.assertEqual(report_validator.ISSUE_SEVERITIES, V1_ISSUE_SEVERITIES)
        for verdict in V1_TECHNICAL_VERDICTS:
            with self.subTest(verdict=verdict):
                report = valid_report().replace(
                    "**Usable with conditions**", f"**{verdict}**", 1
                )
                self.assertEqual(report_validator.validate(report), [])
        for completeness in V1_EVALUATION_COMPLETENESS:
            with self.subTest(completeness=completeness):
                report = valid_report().replace(
                    "**partial** — fixture boundary.",
                    f"**{completeness}** — fixture boundary.",
                    1,
                )
                self.assertEqual(report_validator.validate(report), [])
        for severity in V1_ISSUE_SEVERITIES:
            with self.subTest(severity=severity):
                report = valid_report().replace(
                    "| FIX-001 | moderate |", f"| FIX-001 | {severity} |", 1
                )
                self.assertEqual(report_validator.validate(report), [])

        unknown = valid_report().replace(
            "**Usable with conditions**", "**Buy immediately**", 1
        ).replace(
            "**partial** — fixture boundary.", "**Banana** — fixture boundary.", 1
        ).replace(
            "| FIX-001 | moderate |", "| FIX-001 | Potato |", 1
        )
        errors = report_validator.validate(unknown)
        self.assertIn("report has unknown Technical verdict: Buy immediately", errors)
        self.assertIn("report has unknown Evaluation completeness: Banana", errors)
        self.assertIn("issue FIX-001 has unknown severity: 'Potato'", errors)

    def test_requires_report_format_in_both_documents(self) -> None:
        report_errors = report_validator.validate(
            valid_report().replace("> Report format: **1**\n", "")
        )
        appendix_errors = report_validator.validate_appendix(
            valid_appendix().replace("> Report format: **1**\n", "")
        )
        self.assertIn("report must declare Report format 1", report_errors)
        self.assertIn("appendix must declare Report format 1", appendix_errors)

    def test_primary_report_word_limit_boundary(self) -> None:
        base = valid_report().replace("Fixture decision.", "", 1)
        base_words = report_validator.rendered_word_count(base)

        def with_words(total: int) -> str:
            return base.replace(
                "## Technical decision\n",
                "## Technical decision\n" + " ".join(["word"] * (total - base_words)) + "\n",
                1,
            )

        accepted = report_validator.validate(with_words(2000))
        rejected = report_validator.validate(with_words(2001))
        self.assertFalse(
            any(error.startswith("primary report has ") for error in accepted),
            accepted,
        )
        self.assertIn(
            "primary report has 2001 words; maximum is 2000",
            rejected,
        )

        fenced = valid_report() + "\n```text\n" + "visibleword " * 2500 + "\n```\n"
        errors = report_validator.validate(fenced)
        self.assertTrue(
            any(error.startswith("primary report has ") for error in errors), errors
        )

    def test_rejects_raw_html_documents(self) -> None:
        for suffix in (
            "\n<div>Rendered report prose.</div>\n",
            "\n<!-- invisible --><div>Rendered report prose.</div>\n",
            "\n<!-- invisible --> <script>alert(1)</script>\n",
            "\n<!-- invisible -->" + "visibleword " * 2500 + "\n",
            "\n<!-->" + "visibleword " * 2500 + "-->\n",
            "\n<!-- --!><div>visible</div><!-- -->\n",
        ):
            with self.subTest(suffix=suffix):
                self.assertIn(
                    "report must not contain raw HTML",
                    report_validator.validate(valid_report() + suffix),
                )
        appendix = valid_appendix() + "\n<div>Rendered appendix prose.</div>\n"
        self.assertIn(
            "appendix must not contain raw HTML",
            report_validator.validate_appendix(appendix),
        )

    def test_vendor_marker_validator_scope_matches_the_typos_grammar(self) -> None:
        marker = "`Saber Foward & (Spin)/v2`<!-- vendor-id -->"
        for document, validator, raw_html_error in (
            (valid_report(), report_validator.validate, "report must not contain raw HTML"),
            (
                valid_appendix(),
                report_validator.validate_appendix,
                "appendix must not contain raw HTML",
            ),
        ):
            marked = document.replace("Fixture.", f"{marker}.", 1)
            self.assertNotIn(raw_html_error, validator(marked))
            for malformed in (
                f"`Saber Foward`<!-- vendor-id-->",
                f"``Saber Foward`<!-- vendor-id -->",
                f"```Saber Foward`<!-- vendor-id -->",
                "`first``Saber Foward & (Spin)/v2`<!-- vendor-id -->",
                r"\`Saber Foward`<!-- vendor-id -->",
            ):
                candidate = document + f"\n{malformed}\n"
                self.assertIn(raw_html_error, validator(candidate), malformed)

        fenced = valid_report() + (
            "\n```markdown\n"
            "`Saber Foward & (Spin)/v2`<!-- vendor-id -->\n"
            "```\n\n"
            "A valid inline marker remains `Saber Foward & (Spin)/v2`<!-- vendor-id -->.\n"
        )
        self.assertNotIn(
            "report must not contain raw HTML",
            report_validator.validate(fenced),
        )
        self.assertIn(
            "report must not contain raw HTML",
            report_validator.validate(valid_report() + "\n<div>visible</div>\n"),
        )

    def test_requires_runtime_set_member_and_contract_columns(self) -> None:
        replacements = (
            ("Exact members", "Members omitted"),
            ("Variant/type", "Variant omitted"),
            ("Timing or motion", "Timing omitted"),
            ("Runtime contract", "Contract omitted"),
        )
        for old, new in replacements:
            with self.subTest(field=old):
                errors = report_validator.validate(
                    valid_report().replace(old, new, 1)
                )
                self.assertIn(
                    "runtime-set inventory must use the required member/contract table",
                    errors,
                )

    def test_requires_named_runtime_set_members(self) -> None:
        report = valid_report().replace(
            "| IP `Walk.fbx`; RM `Walk_RM.fbx` |",
            "| unnamed members |",
        )
        self.assertIn(
            "runtime-set member row 1 must name every exact member in code",
            report_validator.validate(report),
        )

        partial = valid_report().replace(
            "IP `Walk.fbx`; RM `Walk_RM.fbx`",
            "IP `Walk.fbx`; RM Walk_RM.fbx",
        )
        self.assertIn(
            "runtime-set member row 1 must name every exact member in code",
            report_validator.validate(partial),
        )

        for members in ("IP ` `; RM `  `", "` `"):
            with self.subTest(members=members):
                whitespace = valid_report().replace(
                    "IP `Walk.fbx`; RM `Walk_RM.fbx`", members
                )
                self.assertIn(
                    "runtime-set member row 1 must name every exact member in code",
                    report_validator.validate(whitespace),
                )

        misdistributed = valid_report().replace(
            "IP `Walk.fbx`; RM `Walk_RM.fbx`",
            "IP `Walk.fbx` `Walk_RM.fbx`; RM Walk.fbx Walk_RM.fbx",
        )
        self.assertIn(
            "runtime-set member row 1 must name every exact member in code",
            report_validator.validate(misdistributed),
        )

    def test_runtime_member_code_spans_support_utf8_and_distinct_pairs(self) -> None:
        utf8 = valid_report().replace(
            "IP `Walk.fbx`; RM `Walk_RM.fbx`",
            "IP `Wälk.fbx`; RM `Wälk_RM.fbx`",
        )
        self.assertEqual(report_validator.validate(utf8), [])

        duplicate = valid_report().replace(
            "IP `Walk.fbx`; RM `Walk_RM.fbx`",
            "IP `Walk.fbx`; RM `Walk.fbx`",
        )
        self.assertIn(
            "runtime-set member row 1 must name distinct exact members",
            report_validator.validate(duplicate),
        )

        generic_duplicate = valid_report().replace(
            "IP `Walk.fbx`; RM `Walk_RM.fbx` | variant=paired-ip-rm | "
            "duration=1.0 s; rm_speed=1.0 m/s | "
            "loop_ip=true; loop_rm=true; sync=gait-phase",
            "`Aim.fbx`; `Aim.fbx` | set_type=directional-blend | N/A | state=aim",
        )
        self.assertIn(
            "runtime-set member row 1 must name distinct exact members",
            report_validator.validate(generic_duplicate),
        )

    def test_rejects_malformed_runtime_timing_and_contract_values(self) -> None:
        mutations = (
            (
                "duration=1.0 s; rm_speed=1.0 m/s",
                "duration=yesterday; rm_speed=very-fast",
                "runtime-set member row 1 has malformed timing or motion evidence",
            ),
            (
                "loop_ip=true; loop_rm=true; sync=gait-phase",
                "loop_ip=banana; loop_rm=perhaps; sync=gait-phase",
                "runtime-set member row 1 has malformed runtime contract",
            ),
            (
                "duration=1.0 s; rm_speed=1.0 m/s",
                "duration=1.0 Hz; rm_speed=1.0 s",
                "runtime-set member row 1 has malformed timing or motion evidence",
            ),
            (
                "duration=1.0 s; rm_speed=1.0 m/s",
                "duration=0 s; rm_speed=1.0 m/s",
                "runtime-set member row 1 has malformed timing or motion evidence",
            ),
            (
                "duration=1.0 s; rm_speed=1.0 m/s",
                "N/A",
                "runtime-set member row 1 has malformed timing or motion evidence",
            ),
            (
                "loop_ip=true; loop_rm=true; sync=gait-phase",
                "N/A",
                "runtime-set member row 1 has malformed runtime contract",
            ),
            (
                "duration=1.0 s; rm_speed=1.0 m/s",
                "duration=1.0 s",
                "runtime-set member row 1 has malformed timing or motion evidence",
            ),
            (
                "duration=1.0 s; rm_speed=1.0 m/s",
                "rm_speed=1.0 m/s",
                "runtime-set member row 1 has malformed timing or motion evidence",
            ),
            (
                "duration=1.0 s; rm_speed=1.0 m/s",
                "threshold=0",
                "runtime-set member row 1 has malformed timing or motion evidence",
            ),
            (
                "duration=1.0 s; rm_speed=1.0 m/s",
                "duration=1.0 s; duration=2.0 s; rm_speed=1.0 m/s",
                "runtime-set member row 1 has malformed timing or motion evidence",
            ),
            (
                "loop_ip=true; loop_rm=true; sync=gait-phase",
                "loop_ip=true; loop_ip=false; loop_rm=true; sync=gait-phase",
                "runtime-set member row 1 has malformed runtime contract",
            ),
            (
                "loop_ip=true; loop_rm=true; sync=gait-phase",
                "loop_ip=true; sync=gait-phase",
                "runtime-set member row 1 has malformed runtime contract",
            ),
        )
        for old, new, expected in mutations:
            with self.subTest(new=new):
                self.assertIn(
                    expected,
                    report_validator.validate(valid_report().replace(old, new, 1)),
                )

        oversized = "9" * 309
        overflow_values = (
            f"duration={oversized} s; rm_speed=1.0 m/s",
            f"duration=1.0 s; rm_speed={oversized} m/s",
            f"duration=1.0 s; rm_speed=1.0 m/s; sample_rate={oversized} Hz",
            f"duration=1.0 s; rm_speed=1.0 m/s; frames={oversized} frames",
            f"duration=1.0 s; rm_speed=1.0 m/s; threshold={oversized}",
            "duration=0." + "0" * 400 + "1 s; rm_speed=1.0 m/s",
        )
        for timing in overflow_values:
            with self.subTest(timing=timing):
                report = valid_report().replace(
                    "duration=1.0 s; rm_speed=1.0 m/s", timing, 1
                )
                self.assertIn(
                    "runtime-set member row 1 has malformed timing or motion evidence",
                    report_validator.validate(report),
                )

        root_motion = valid_report().replace(
            "IP `Walk.fbx`; RM `Walk_RM.fbx` | variant=paired-ip-rm | "
            "duration=1.0 s; rm_speed=1.0 m/s | "
            "loop_ip=true; loop_rm=true; sync=gait-phase",
            "`Walk_RM.fbx` | variant=root-motion | N/A | movement=animation",
        )
        self.assertIn(
            "runtime-set member row 1 has malformed timing or motion evidence",
            report_validator.validate(root_motion),
        )

    def test_requires_runtime_variant_or_set_type_token(self) -> None:
        report = valid_report().replace(
            "variant=paired-ip-rm", "paired IP/RM", 1
        )
        self.assertIn(
            "runtime-set member row 1 has malformed variant or set type",
            report_validator.validate(report),
        )

        dual_discriminator = valid_report().replace(
            "variant=paired-ip-rm",
            "variant=paired-ip-rm; set_type=directional-blend",
            1,
        )
        self.assertIn(
            "runtime-set member row 1 has malformed variant or set type",
            report_validator.validate(dual_discriminator),
        )

        disguised_root_motion = valid_report().replace(
            "IP `Walk.fbx`; RM `Walk_RM.fbx` | variant=paired-ip-rm | "
            "duration=1.0 s; rm_speed=1.0 m/s | "
            "loop_ip=true; loop_rm=true; sync=gait-phase",
            "RM `Walk_RM.fbx` | variant=directional-blend | "
            "rm_speed=1.0 m/s | movement=controller",
        )
        errors = report_validator.validate(disguised_root_motion)
        self.assertIn(
            "runtime-set member row 1 has malformed variant or set type", errors
        )
        self.assertIn(
            "runtime-set member row 1 has RM members without a root-motion variant",
            errors,
        )
        self.assertIn(
            "runtime-set member row 1 has malformed timing or motion evidence",
            errors,
        )

    def test_paired_variant_requires_exactly_one_ip_and_rm_member(self) -> None:
        for members in (
            "IP `Walk.fbx`",
            "IP `Walk.fbx`; IP `Walk_RM.fbx`",
            "RM `Walk.fbx`; RM `Walk_RM.fbx`",
        ):
            with self.subTest(members=members):
                report = valid_report().replace(
                    "IP `Walk.fbx`; RM `Walk_RM.fbx`", members
                )
                self.assertIn(
                    "runtime-set member row 1 must name exactly one IP and one RM member",
                    report_validator.validate(report),
                )

    def test_accepts_explicit_absence_of_runtime_sets(self) -> None:
        report = valid_report()
        start = report.index("| Set/profile |")
        end = report.index("\n\n## Integration recipe")
        report = (
            report[:start]
            + "No important runtime sets were identified."
            + report[end:]
        )
        self.assertEqual(report_validator.validate(report), [])

    def test_rejects_hidden_runtime_set_absence_statement(self) -> None:
        report = valid_report()
        start = report.index("| Set/profile |")
        end = report.index("\n\n## Integration recipe")
        report = (
            report[:start]
            + "<!-- No important runtime sets were identified. -->"
            + report[end:]
        )
        self.assertIn(
            "runtime-set inventory must use the required member/contract table",
            report_validator.validate(report),
        )
        fenced = report.replace(
            "<!-- No important runtime sets were identified. -->",
            "```\nNo important runtime sets were identified.\n```",
        )
        self.assertIn(
            "runtime-set inventory must use the required member/contract table",
            report_validator.validate(fenced),
        )
        quoted = report.replace(
            "<!-- No important runtime sets were identified. -->",
            "> No important runtime sets were identified.",
        )
        self.assertIn(
            "runtime-set inventory must use the required member/contract table",
            report_validator.validate(quoted),
        )

    def test_rejects_absence_statement_beside_runtime_table(self) -> None:
        report = valid_report().replace(
            "## Runtime sets and authored motion\n",
            "## Runtime sets and authored motion\n"
            "No important runtime sets were identified.\n",
            1,
        )
        self.assertIn(
            "runtime-set inventory contradicts the explicit no-set result",
            report_validator.validate(report),
        )

    def test_runtime_set_absence_must_be_the_sole_section_body(self) -> None:
        report = valid_report()
        start = report.index("| Set/profile |")
        end = report.index("\n\n## Integration recipe", start)
        base = report[:start] + "No important runtime sets were identified." + report[end:]
        for addition in (
            "\n\n- Important runtime set Walk exists.",
            "\n\n> Important runtime set Walk exists.",
            "\n\n```text\nImportant runtime set Walk exists.\n```",
            "\n\n---",
        ):
            with self.subTest(addition=addition):
                mutated = base.replace(
                    "\n\n## Integration recipe", addition + "\n\n## Integration recipe", 1
                )
                self.assertIn(
                    "runtime-set inventory must use the required member/contract table",
                    report_validator.validate(mutated),
                )

    def test_requires_every_recipe_decision(self) -> None:
        for label in report_validator.RECIPE_LABELS:
            with self.subTest(label=label):
                report = valid_report().replace(f"**{label}:**", "**Omitted:**", 1)
                errors = report_validator.validate(report)
                self.assertTrue(
                    any(label in error for error in errors), errors
                )

    def test_rejects_empty_recipe_decisions(self) -> None:
        for label in report_validator.RECIPE_LABELS:
            with self.subTest(label=label):
                marker = f"**{label}:**"
                report = valid_report()
                report = re.sub(
                    rf"({re.escape(marker)})[^\n]+",
                    rf"\1 x",
                    report,
                    count=1,
                )
                errors = report_validator.validate(report)
                self.assertTrue(
                    any("lacks an implementable" in error and label in error for error in errors),
                    errors,
                )

        state = valid_report().replace(
            "`owner=gameplay-controller`; fixture movement policy.",
            "State ownership was not evaluated.",
        )
        self.assertIn(
            "integration recipe step 3 lacks an implementable State ownership decision",
            report_validator.validate(state),
        )

    def test_recipe_steps_must_be_direct_list_items(self) -> None:
        nested = valid_report().replace(
            "1. **Members/topology:**",
            "1. **Members/topology:**",
            1,
        )
        nested = nested.replace("\n2. **Timing", "\n\n   2. **Timing", 1)
        for number, label in (
            (3, "State"),
            (4, "Composition"),
            (5, "Acceptance"),
        ):
            nested = nested.replace(
                f"\n{number}. **{label}", f"\n   {number}. **{label}", 1
            )
        errors = report_validator.validate(nested)
        for number, label in enumerate(report_validator.RECIPE_LABELS[1:], start=2):
            self.assertIn(
                f"integration recipe is missing step {number}: {label}", errors
            )

        report = valid_report()
        start = report.index("1. **Members/topology:**")
        end = report.index("\n\n## Technical issue register", start)
        quoted = "\n".join("> " + line for line in report[start:end].splitlines())
        report = report[:start] + quoted + report[end:]
        errors = report_validator.validate(report)
        for number, label in enumerate(report_validator.RECIPE_LABELS, start=1):
            self.assertIn(
                f"integration recipe is missing step {number}: {label}", errors
            )

        report = valid_report()
        start = report.index("1. **Members/topology:**")
        end = report.index("\n\n## Technical issue register", start)
        steps = report[start:end].splitlines()
        reordered = "\n\n---\n\n".join([steps[4], *steps[:4]])
        report = report[:start] + reordered + report[end:]
        self.assertIn(
            "integration recipe steps must appear in order 1 through 5",
            report_validator.validate(report),
        )

    def test_requires_all_common_engines(self) -> None:
        for engine in ("Unity", "Unreal Engine", "Godot", "Bevy"):
            with self.subTest(engine=engine):
                report = valid_report().replace(f"| {engine} |", "| Omitted |", 1)
                self.assertIn(
                    f"engine status is missing: {engine}",
                    report_validator.validate(report),
                )

    def test_engine_rows_must_stay_in_engine_section(self) -> None:
        row = "| Unity | not evaluated | Fixture. | Fixture. |\n"
        report = valid_report().replace(row, "", 1) + "\n" + row
        self.assertIn(
            "engine status is missing: Unity",
            report_validator.validate(report),
        )

    def test_engine_rows_require_full_table_shape(self) -> None:
        report = valid_report().replace(
            "| Unity | not evaluated | Fixture. | Fixture. |",
            "| Unity |",
        )
        self.assertIn(
            "engine status row is malformed: Unity",
            report_validator.validate(report),
        )

    def test_engine_table_requires_header_and_separator(self) -> None:
        header = "| Runtime | Evidence level | Technical result | Remaining gate |\n"
        separator = "|---|---|---|---|\n"
        no_header = valid_report().replace(header, "", 1)
        no_separator = valid_report().replace(header + separator, header, 1)
        self.assertTrue(
            any("missing required table header" in error for error in report_validator.validate(no_header))
        )
        self.assertTrue(
            any("missing required table header" in error for error in report_validator.validate(no_separator))
        )

    def test_report_tables_reject_wrong_width_separators(self) -> None:
        tables = (
            (
                "| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |\n",
                "|---|---|---|---|---|---|\n",
            ),
            (
                "| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |\n",
                "|---|---|---|---|---|---|---|\n",
            ),
            (
                "| Runtime | Evidence level | Technical result | Remaining gate |\n",
                "|---|---|---|---|\n",
            ),
        )
        for header, separator in tables:
            with self.subTest(header=header):
                report = valid_report().replace(
                    header + separator, header + "|---|\n", 1
                )
                self.assertNotEqual(report_validator.validate(report), [])

    def test_requires_issue_guidance_link(self) -> None:
        report = valid_report().replace(
            "[Fixture problem.](../game-ready-clips.md#the-loop-pops)",
            "Fixture problem.",
        )
        self.assertIn(
            "issue FIX-001 must link applicable docs/game-ready-clips.md guidance or mark it not applicable",
            report_validator.validate(report),
        )

    def test_issue_guidance_must_be_a_rendered_markdown_link(self) -> None:
        link = "[Fixture problem.](../game-ready-clips.md#the-loop-pops)"
        for replacement in (
            "Fixture problem: ../game-ready-clips.md#the-loop-pops",
            f"`{link}`",
            "\\[Fixture problem.](../game-ready-clips.md#the-loop-pops)",
        ):
            with self.subTest(replacement=replacement):
                report = valid_report().replace(link, replacement)
                self.assertIn(
                    "issue FIX-001 must link applicable docs/game-ready-clips.md guidance or mark it not applicable",
                    report_validator.validate(report),
                )

    def test_accepts_explicitly_inapplicable_issue_guidance(self) -> None:
        report = valid_report().replace(
            "[Fixture problem.](../game-ready-clips.md#the-loop-pops)",
            "Fixture problem. Guidance: not applicable.",
        )
        self.assertEqual(report_validator.validate(report), [])

    def test_rejects_hidden_or_inline_issue_guidance_opt_out(self) -> None:
        for replacement in (
            "Fixture problem. <!-- Guidance: not applicable. -->",
            "Fixture problem. `Guidance: not applicable.`",
        ):
            with self.subTest(replacement=replacement):
                report = valid_report().replace(
                    "[Fixture problem.](../game-ready-clips.md#the-loop-pops)",
                    replacement,
                )
                self.assertIn(
                    "issue FIX-001 must link applicable docs/game-ready-clips.md guidance or mark it not applicable",
                    report_validator.validate(report),
                )

    def test_rejects_absolute_local_docs_link(self) -> None:
        report = valid_report().replace(
            "../game-ready-clips.md#the-loop-pops",
            "https://github.com/example/repo/blob/revision/docs/game-ready-clips.md#the-loop-pops",
        )
        self.assertIn(
            "report must use repository-relative links for local AnimSmith docs",
            report_validator.validate(report),
        )

    def test_rejects_unknown_or_composite_issue_owner(self) -> None:
        for owner in ("unknown-owner", "vendor-license / artist-author"):
            with self.subTest(owner=owner):
                report = valid_report().replace(
                    "| engine-config | Fixture workaround. |",
                    f"| {owner} | Fixture workaround. |",
                )
                self.assertIn(
                    f"issue FIX-001 has unknown or composite primary owner: {owner!r}",
                    report_validator.validate(report),
                )

    def test_issue_rows_accept_optional_outer_pipes(self) -> None:
        full_row = (
            "| FIX-001 | moderate | [Fixture problem.]"
            "(../game-ready-clips.md#the-loop-pops) | engine-config | "
            "Fixture workaround. | Not applicable. | High. |"
        )
        for row in (full_row.lstrip("| "), full_row.rstrip(" |")):
            with self.subTest(row=row):
                report = valid_report().replace(full_row, row)
                self.assertEqual(report_validator.validate(report), [])

    def test_rejects_malformed_or_empty_issue_rows(self) -> None:
        malformed = valid_report().replace(
            "| Fixture workaround. | Not applicable. | High. |",
            "| Fixture workaround. | High. |",
        )
        self.assertIn(
            "issue FIX-001 has empty required cells: Evidence/status",
            report_validator.validate(malformed),
        )
        empty = valid_report().replace(
            "| FIX-001 | moderate |",
            "| FIX-001 |  |",
        )
        self.assertIn(
            "issue FIX-001 has empty required cells: Severity",
            report_validator.validate(empty),
        )

    def test_rejects_duplicate_issue_ids_in_deterministic_order(self) -> None:
        report = valid_report()
        row = next(line for line in report.splitlines() if line.startswith("| FIX-001 |"))
        zzz_row = row.replace("FIX-001", "ZZZ-001", 1)
        duplicate_rows = zzz_row + "\n" + zzz_row + "\n" + row
        report = report.replace(row, row + "\n" + duplicate_rows, 1)
        self.assertIn(
            "technical issue register contains duplicate IDs: FIX-001, ZZZ-001",
            report_validator.validate(report),
        )

    def test_rejects_duplicate_issue_tables(self) -> None:
        report = valid_report()
        start = report.index("| ID | Severity |")
        end = report.index("\n\n## Engine status", start)
        table = report[start:end]
        duplicated = report[:end] + "\n\n" + table + report[end:]
        self.assertIn(
            "technical issue register must contain an issue table or the explicit clean result",
            report_validator.validate(duplicated),
        )

    def test_requires_issue_table_header_and_separator(self) -> None:
        header = "| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |\n"
        separator = "|---|---|---|---|---|---|---|\n"
        without_header = valid_report().replace(header, "", 1)
        without_separator = valid_report().replace(separator, "", 1)
        self.assertIn(
            "technical issue register must contain an issue table or the explicit clean result",
            report_validator.validate(without_header),
        )
        self.assertIn(
            "technical issue register must contain an issue table or the explicit clean result",
            report_validator.validate(without_separator),
        )

    def test_rejects_runtime_and_issue_tables_rendered_as_indented_code(self) -> None:
        cases = (
            (
                "| Set/profile |",
                "\n\n## Integration recipe",
                "runtime-set inventory must use the required member/contract table",
            ),
            (
                "| ID | Severity |",
                "\n\n## Engine status",
                "technical issue register must contain an issue table or the explicit clean result",
            ),
        )
        for start_marker, end_marker, expected in cases:
            with self.subTest(start_marker=start_marker):
                report = valid_report()
                start = report.index(start_marker)
                end = report.index(end_marker, start)
                table = report[start:end]
                indented = "\n".join(f"    {line}" for line in table.splitlines())
                report = report[:start] + indented + report[end:]
                self.assertIn(expected, report_validator.validate(report))

    def test_rejects_runtime_and_issue_tables_inside_raw_html_blocks(self) -> None:
        cases = (
            (
                "| Set/profile |",
                "\n\n## Integration recipe",
                "runtime-set inventory must use the required member/contract table",
            ),
            (
                "| ID | Severity |",
                "\n\n## Engine status",
                "technical issue register must contain an issue table or the explicit clean result",
            ),
        )
        for start_marker, end_marker, expected in cases:
            for tag in ("pre", "div"):
                with self.subTest(start_marker=start_marker, tag=tag):
                    report = valid_report()
                    start = report.index(start_marker)
                    end = report.index(end_marker, start)
                    table = report[start:end]
                    report = (
                        report[:start]
                        + f"<{tag}>\n{table}\n</{tag}>"
                        + report[end:]
                    )
                    self.assertIn(expected, report_validator.validate(report))

    def test_blockquoted_headings_and_tables_do_not_satisfy_report_structure(self) -> None:
        report = valid_report().replace(
            "## Runtime sets and authored motion\n",
            "> ## Runtime sets and authored motion\n>\n",
            1,
        )
        start = report.index("| Set/profile |")
        end = report.index("\n\n## Integration recipe", start)
        quoted_table = "\n".join(
            "> " + line for line in report[start:end].splitlines()
        )
        report = report[:start] + quoted_table + report[end:]
        errors = report_validator.validate(report)
        self.assertIn(
            "missing required heading: ## Runtime sets and authored motion", errors
        )
        self.assertIn(
            "runtime-set inventory must use the required member/contract table", errors
        )

    def test_list_nested_headings_and_tables_do_not_satisfy_report_structure(self) -> None:
        heading = valid_report().replace(
            "## Technical decision", "- ## Technical decision", 1
        )
        self.assertIn(
            "missing required heading: ## Technical decision",
            report_validator.validate(heading),
        )

        report = valid_report()
        start = report.index("| Runtime | Evidence level |")
        end = report.index("\n\n## Fit and limitations", start)
        table = report[start:end]
        nested = "- Nested table:\n\n" + "\n".join(
            "  " + line for line in table.splitlines()
        )
        report = report[:start] + nested + report[end:]
        errors = report_validator.validate(report)
        self.assertTrue(
            any("missing required table header" in error for error in errors), errors
        )
        for engine in ("Unity", "Unreal Engine", "Godot", "Bevy"):
            self.assertIn(f"engine status is missing: {engine}", errors)

    def test_accepts_explicit_clean_issue_register(self) -> None:
        report = valid_report()
        start = report.index("| ID | Severity |")
        end = report.index("\n\n## Engine status")
        report = (
            report[:start]
            + "No material technical issues were found at the stated scope."
            + report[end:]
        )
        self.assertEqual(report_validator.validate(report), [])

    def test_rejects_hidden_clean_issue_register(self) -> None:
        report = valid_report()
        start = report.index("| ID | Severity |")
        end = report.index("\n\n## Engine status")
        report = (
            report[:start]
            + "<!-- No material technical issues were found at the stated scope. -->"
            + report[end:]
        )
        self.assertIn(
            "technical issue register must contain an issue table or the explicit clean result",
            report_validator.validate(report),
        )
        fenced = report.replace(
            "<!-- No material technical issues were found at the stated scope. -->",
            "```\nNo material technical issues were found at the stated scope.\n```",
        )
        self.assertIn(
            "technical issue register must contain an issue table or the explicit clean result",
            report_validator.validate(fenced),
        )
        quoted = report.replace(
            "<!-- No material technical issues were found at the stated scope. -->",
            "> No material technical issues were found at the stated scope.",
        )
        self.assertIn(
            "technical issue register must contain an issue table or the explicit clean result",
            report_validator.validate(quoted),
        )

    def test_rejects_clean_result_beside_issue_table(self) -> None:
        report = valid_report().replace(
            "## Technical issue register\n",
            "## Technical issue register\n"
            "No material technical issues were found at the stated scope.\n",
            1,
        )
        self.assertIn(
            "technical issue register contradicts the explicit clean result",
            report_validator.validate(report),
        )

    def test_clean_result_must_be_the_sole_issue_section_body(self) -> None:
        report = valid_report()
        start = report.index("| ID | Severity |")
        end = report.index("\n\n## Engine status", start)
        base = (
            report[:start]
            + "No material technical issues were found at the stated scope."
            + report[end:]
        )
        for addition in (
            "\n\n- Material issue exists.",
            "\n\n> Material issue exists.",
            "\n\n```text\nMaterial issue exists.\n```",
            "\n\n---",
        ):
            with self.subTest(addition=addition):
                mutated = base.replace(
                    "\n\n## Engine status", addition + "\n\n## Engine status", 1
                )
                self.assertIn(
                    "technical issue register must contain an issue table or the explicit clean result",
                    report_validator.validate(mutated),
                )

    def test_placeholder_errors_are_deterministic(self) -> None:
        errors = report_validator.validate(
            valid_report() + "\n{{ZETA_PLACEHOLDER}} {{ALPHA_PLACEHOLDER}}\n"
        )
        self.assertEqual(
            errors,
            [
                "unresolved template placeholders: "
                "{{ALPHA_PLACEHOLDER}}, {{ZETA_PLACEHOLDER}}"
            ],
        )

    def test_requires_every_appendix_heading(self) -> None:
        for heading in V1_REQUIRED_APPENDIX_HEADINGS:
            with self.subTest(heading=heading):
                appendix = valid_appendix().replace(
                    f"{heading}\n", f"{heading} omitted\n", 1
                )
                self.assertIn(
                    f"missing required heading: {heading}",
                    report_validator.validate_appendix(appendix),
                )

    def test_requires_appendix_manifest_headings(self) -> None:
        for heading in V1_REQUIRED_APPENDIX_MANIFEST_HEADINGS:
            with self.subTest(heading=heading):
                appendix = valid_appendix().replace(
                    f"{heading}\n", f"{heading} omitted\n", 1
                )
                self.assertIn(
                    f"missing required heading: {heading}",
                    report_validator.validate_appendix(appendix),
                )

    def test_rejects_out_of_order_appendix_headings(self) -> None:
        heading_groups = (
            V1_REQUIRED_APPENDIX_HEADINGS,
            V1_REQUIRED_APPENDIX_MANIFEST_HEADINGS,
        )
        for headings in heading_groups:
            for first, second in zip(headings, headings[1:]):
                with self.subTest(first=first, second=second):
                    appendix = valid_appendix()
                    first_position = appendix.index(first)
                    second_position = appendix.index(second)
                    appendix = (
                        appendix[:first_position]
                        + second
                        + appendix[first_position + len(first) : second_position]
                        + first
                        + appendix[second_position + len(second) :]
                    )
                    errors = report_validator.validate_appendix(appendix)
                    self.assertTrue(
                        any("out of order" in error for error in errors), errors
                    )

    def test_manifest_subsections_must_stay_in_manifest_section(self) -> None:
        appendix = valid_appendix()
        start = appendix.index("### Canonical clip-role inventory")
        end = appendix.index("## Pack inventory and content evidence")
        relocated = appendix[start:end]
        appendix = appendix[:start] + appendix[end:] + "\n" + relocated
        self.assertIn(
            "missing required heading: ### Canonical clip-role inventory",
            report_validator.validate_appendix(appendix),
        )

    def test_requires_every_canonical_role(self) -> None:
        for role in V1_PRIMARY_ROLES:
            with self.subTest(role=role):
                appendix = valid_appendix().replace(
                    f"| `{role}` | 0 | 0 | Fixture. |\n", ""
                )
                self.assertIn(
                    f"canonical role inventory is missing: {role}",
                    report_validator.validate_appendix(appendix),
                )

    def test_requires_every_pipeline_stage(self) -> None:
        for _identifier, label in V1_PIPELINE_STAGE_ROWS:
            with self.subTest(label=label):
                appendix = valid_appendix().replace(
                    f"| {label} | `evaluated-clean` | Fixture. |\n", ""
                )
                self.assertIn(
                    f"pipeline-stage coverage is missing: {label}",
                    report_validator.validate_appendix(appendix),
                )

    def test_requires_every_validation_profile(self) -> None:
        for _identifier, label in V1_PROFILE_ROWS:
            with self.subTest(label=label):
                appendix = valid_appendix().replace(
                    f"| {label} | `not-selected` | Fixture. |\n", ""
                )
                self.assertIn(
                    f"validation-profile status is missing: {label}",
                    report_validator.validate_appendix(appendix),
                )

    def test_appendix_inventory_rows_require_full_typed_shapes(self) -> None:
        role = valid_appendix().replace(
            "| `idle-pose` | 0 | 0 | Fixture. |",
            "| `idle-pose` | banana | 0 | Fixture. |",
        )
        stage = valid_appendix().replace(
            "| Acquire | `evaluated-clean` | Fixture. |",
            "| Acquire |",
        )
        profile = valid_appendix().replace(
            "| Marketplace intake | `not-selected` | Fixture. |",
            "| Marketplace intake | unknown | Fixture. |",
        )
        self.assertIn(
            "canonical role inventory row is malformed: idle-pose",
            report_validator.validate_appendix(role),
        )
        self.assertIn(
            "pipeline-stage coverage row is malformed: Acquire",
            report_validator.validate_appendix(stage),
        )
        self.assertIn(
            "validation-profile status row is malformed: Marketplace intake",
            report_validator.validate_appendix(profile),
        )

        suffixed_stage = valid_appendix().replace(
            "| Acquire | `evaluated-clean` | Fixture. |",
            "| Acquire | `evaluated-clean` contradictory | Fixture. |",
        )
        self.assertIn(
            "pipeline-stage coverage row is malformed: Acquire",
            report_validator.validate_appendix(suffixed_stage),
        )

    def test_appendix_taxonomy_tables_reject_unknown_rows(self) -> None:
        stage = valid_appendix().replace(
            "| Gate/report | `evaluated-clean` | Fixture. |",
            "| Gate/report | `evaluated-clean` | Fixture. |\n"
            "| Invented | `evaluated-clean` | Fixture. |",
        )
        profile = valid_appendix().replace(
            "| Runtime performance | `not-selected` | Fixture. |",
            "| Runtime performance | `not-selected` | Fixture. |\n"
            "| Invented | `not-selected` | Fixture. |",
        )
        self.assertIn(
            "pipeline-stage coverage contains an unknown stage row",
            report_validator.validate_appendix(stage),
        )
        self.assertIn(
            "validation-profile status contains an unknown profile row",
            report_validator.validate_appendix(profile),
        )

    def test_profile_rows_fail_closed_at_every_truncation(self) -> None:
        full = "| Marketplace intake | `not-selected` | Fixture. |"
        for replacement in (
            "| Marketplace intake |",
            "| Marketplace intake | `not-selected` |",
        ):
            with self.subTest(replacement=replacement):
                appendix = valid_appendix().replace(full, replacement)
                self.assertIn(
                    "validation-profile status row is malformed: Marketplace intake",
                    report_validator.validate_appendix(appendix),
                )

    def test_profile_selection_uses_exact_status_and_activation_shape(self) -> None:
        mutations = (
            "`not-selected` contradictory",
            "`not-applicable` — `user-required`",
            "`selected`",
            "`selected` — `unknown-basis`",
        )
        for selection in mutations:
            with self.subTest(selection=selection):
                appendix = valid_appendix().replace(
                    "| Marketplace intake | `not-selected` | Fixture. |",
                    f"| Marketplace intake | {selection} | Fixture. |",
                )
                self.assertIn(
                    "validation-profile status row is malformed: Marketplace intake",
                    report_validator.validate_appendix(appendix),
                )

        for basis in V1_ACTIVATION_BASES:
            with self.subTest(basis=basis):
                appendix = valid_appendix().replace(
                    "| Marketplace intake | `not-selected` | Fixture. |",
                    f"| Marketplace intake | `selected` — `{basis}` | Fixture. |",
                )
                self.assertEqual(report_validator.validate_appendix(appendix), [])

    def test_role_totals_reconcile_every_role_and_both_total_counts(self) -> None:
        for role in V1_PRIMARY_ROLES:
            for column in ("logical", "files"):
                with self.subTest(role=role, column=column):
                    old = f"| `{role}` | 0 | 0 | Fixture. |"
                    new = (
                        f"| `{role}` | 1 | 0 | Fixture. |"
                        if column == "logical"
                        else f"| `{role}` | 0 | 1 | Fixture. |"
                    )
                    appendix = valid_appendix().replace(old, new)
                    self.assertIn(
                        "canonical role inventory totals do not reconcile",
                        report_validator.validate_appendix(appendix),
                    )
        for total in (
            "| **Total** | **1** | **0** | Fixture. |",
            "| **Total** | **0** | **1** | Fixture. |",
        ):
            with self.subTest(total=total):
                appendix = valid_appendix().replace(
                    "| **Total** | **0** | **0** | Fixture. |", total
                )
                self.assertIn(
                    "canonical role inventory totals do not reconcile",
                    report_validator.validate_appendix(appendix),
                )

    def test_role_inventory_rejects_unknown_rows(self) -> None:
        appendix = valid_appendix().replace(
            "| **Total** | **0** | **0** | Fixture. |",
            "| `bogus-role` | 99 | 99 | Fixture. |\n"
            "| **Total** | **0** | **0** | Fixture. |",
        )
        self.assertIn(
            "canonical role inventory contains an unknown role row",
            report_validator.validate_appendix(appendix),
        )

    def test_role_inventory_rejects_unicode_numeric_counts(self) -> None:
        for old, new in (("| 0 | 0 |", "| ² | 0 |"), ("**0** | **0**", "**²** | **0**")):
            with self.subTest(new=new):
                appendix = valid_appendix().replace(old, new, 1)
                errors = report_validator.validate_appendix(appendix)
                self.assertTrue(
                    any(
                        "canonical role inventory" in error
                        and ("malformed" in error or "requires one Total" in error)
                        for error in errors
                    ),
                    errors,
                )

    def test_role_inventory_rejects_oversized_ascii_counts_without_crashing(self) -> None:
        oversized = "9" * 5000
        mutations = (
            ("| `idle-pose` | 0 | 0 |", f"| `idle-pose` | {oversized} | 0 |"),
            (
                "| **Total** | **0** | **0** |",
                f"| **Total** | **{oversized}** | **0** |",
            ),
        )
        for old, new in mutations:
            with self.subTest(total="**Total**" in old):
                errors = report_validator.validate_appendix(
                    valid_appendix().replace(old, new, 1)
                )
                self.assertTrue(
                    any(
                        "canonical role inventory" in error and "malformed" in error
                        for error in errors
                    ),
                    errors,
                )

    def test_appendix_runtime_inventory_requires_table_or_exact_absence(self) -> None:
        appendix = valid_appendix().replace(
            "| Walk | directional-blend | IP/RM pair | Fixture. | Fixture. |",
            "| Walk |",
        )
        self.assertIn(
            "runtime-set appendix row 1 is malformed",
            report_validator.validate_appendix(appendix),
        )
        contradictory = valid_appendix().replace(
            "### Runtime-set inventory\n",
            "### Runtime-set inventory\nNo runtime sets were identified.\n",
            1,
        )
        self.assertIn(
            "runtime-set appendix inventory contradicts the explicit no-set result",
            report_validator.validate_appendix(contradictory),
        )
        for unknown in ("banana", "unknown"):
            with self.subTest(unknown=unknown):
                malformed_type = valid_appendix().replace(
                    "| Walk | directional-blend |",
                    f"| Walk | {unknown} |",
                )
                self.assertIn(
                    f"runtime-set appendix row 1 has unknown type: {unknown}",
                    report_validator.validate_appendix(malformed_type),
                )

        duplicate = valid_appendix().replace(
            "| Walk | directional-blend | IP/RM pair | Fixture. | Fixture. |",
            "| Walk | directional-blend | IP/RM pair | Fixture. | Fixture. |\n"
            "| Walk | speed-blend | IP/RM pair | Fixture. | Fixture. |",
        )
        self.assertIn(
            "runtime-set appendix inventory contains duplicate sets: Walk",
            report_validator.validate_appendix(duplicate),
        )

        appendix = valid_appendix()
        start = appendix.index("| Runtime set | Type |")
        end = appendix.index("\n\n### Pipeline-stage coverage", start)
        contradictory_absence = (
            appendix[:start]
            + "No runtime sets were identified.\n\nVisible contradictory prose."
            + appendix[end:]
        )
        self.assertIn(
            "runtime-set appendix inventory contradicts the explicit no-set result",
            report_validator.validate_appendix(contradictory_absence),
        )

        quoted_absence = (
            appendix[:start] + "> No runtime sets were identified." + appendix[end:]
        )
        self.assertIn(
            "missing required table header: "
            "Runtime set | Type | Members/variants | Grouping evidence | "
            "Validation status",
            report_validator.validate_appendix(quoted_absence),
        )

    def test_appendix_runtime_absence_must_be_the_sole_subsection_body(self) -> None:
        appendix = valid_appendix()
        start = appendix.index("| Runtime set | Type |")
        end = appendix.index("\n\n### Pipeline-stage coverage", start)
        base = appendix[:start] + "No runtime sets were identified." + appendix[end:]
        for addition in (
            "\n\n- Candidate set Walk exists.",
            "\n\n> Candidate set Walk exists.",
            "\n\n```text\nCandidate set Walk exists.\n```",
            "\n\n---",
        ):
            with self.subTest(addition=addition):
                mutated = base.replace(
                    "\n\n### Pipeline-stage coverage",
                    addition + "\n\n### Pipeline-stage coverage",
                    1,
                )
                self.assertIn(
                    "runtime-set appendix inventory contradicts the explicit no-set result",
                    report_validator.validate_appendix(mutated),
                )

    def test_pair_rejects_runtime_set_presence_disagreement(self) -> None:
        appendix = valid_appendix()
        start = appendix.index("| Runtime set | Type |")
        end = appendix.index("\n\n### Pipeline-stage coverage")
        no_sets_appendix = (
            appendix[:start] + "No runtime sets were identified." + appendix[end:]
        )
        self.assertEqual(report_validator.validate_appendix(no_sets_appendix), [])
        self.assertIn(
            "report and appendix disagree on runtime-set presence",
            report_validator.validate_pair(
                valid_report(),
                no_sets_appendix,
                "fixture.md",
                "fixture-evidence.md",
            ),
        )

        renamed = valid_appendix().replace(
            "| Walk | directional-blend |", "| Sprint | directional-blend |", 1
        )
        self.assertEqual(report_validator.validate_appendix(renamed), [])
        self.assertIn(
            "appendix runtime-set inventory is missing primary sets: Walk",
            report_validator.validate_pair(
                valid_report(),
                renamed,
                "fixture.md",
                "fixture-evidence.md",
            ),
        )

    def test_pair_allows_appendix_only_candidate_runtime_sets(self) -> None:
        report = valid_report()
        start = report.index("| Set/profile |")
        end = report.index("\n\n## Integration recipe", start)
        report = (
            report[:start]
            + "No important runtime sets were identified."
            + report[end:]
        )
        self.assertEqual(report_validator.validate(report), [])
        self.assertEqual(
            report_validator.validate_pair(
                report,
                valid_appendix(),
                "fixture.md",
                "fixture-evidence.md",
            ),
            [],
        )

    def test_appendix_inventory_tables_require_headers_and_separators(self) -> None:
        role_header = "| Canonical primary role | Logical motions | Delivered files | Evidence boundary |\n"
        stage_header = "| Stage | Coverage state | Evidence / remaining gate |\n"
        stage_separator = "|---|---|---|\n"
        profile_header = "| Validation profile | Selection | Result / next evidence |\n"
        cases = (
            valid_appendix().replace(role_header, "", 1),
            valid_appendix().replace(
                stage_header + stage_separator, stage_header, 1
            ),
            valid_appendix().replace(profile_header, "", 1),
        )
        for appendix in cases:
            with self.subTest():
                errors = report_validator.validate_appendix(appendix)
                self.assertTrue(
                    any(
                        "missing required table header" in error
                        or "missing Markdown separator" in error
                        for error in errors
                    ),
                    errors,
                )

    def test_appendix_tables_reject_wrong_width_separators(self) -> None:
        tables = (
            (
                "| Canonical primary role | Logical motions | Delivered files | Evidence boundary |\n",
                "|---|---:|---:|---|\n",
            ),
            (
                "| Runtime set | Type | Members/variants | Grouping evidence | Validation status |\n",
                "|---|---|---|---|---|\n",
            ),
            (
                "| Stage | Coverage state | Evidence / remaining gate |\n",
                "|---|---|---|\n",
            ),
            (
                "| Validation profile | Selection | Result / next evidence |\n",
                "|---|---|---|\n",
            ),
        )
        for header, separator in tables:
            with self.subTest(header=header):
                appendix = valid_appendix().replace(
                    header + separator, header + "|---|\n", 1
                )
                self.assertNotEqual(
                    report_validator.validate_appendix(appendix), []
                )

    def test_appendix_requires_schema_and_canonical_ladder(self) -> None:
        appendix = valid_appendix().replace(
            V1_SCHEMA, "urn:wrong:schema"
        ).replace(
            "../game-ready-clips.md#the-readiness-ladder",
            "../game-ready-clips.md#wrong",
        )
        errors = report_validator.validate_appendix(appendix)
        self.assertIn(
            f"appendix must identify evaluation manifest schema: {V1_SCHEMA}",
            errors,
        )
        self.assertIn("appendix must link the canonical readiness ladder", errors)

    def test_canonical_ladder_must_be_a_rendered_markdown_link(self) -> None:
        link = "[canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder)"
        replacements = (
            "canonical readiness ladder: ../game-ready-clips.md#the-readiness-ladder",
            f"`{link}`",
            "\\[canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder)",
        )
        for replacement in replacements:
            with self.subTest(replacement=replacement):
                report = valid_report().replace(link, replacement)
                appendix = valid_appendix().replace(link, replacement)
                self.assertIn(
                    "report must link the canonical readiness ladder",
                    report_validator.validate(report),
                )
                self.assertIn(
                    "appendix must link the canonical readiness ladder",
                    report_validator.validate_appendix(appendix),
                )

    def test_appendix_rejects_duplicate_issue_register(self) -> None:
        appendix = valid_appendix() + "\n## Technical issue register\nFixture.\n"
        self.assertIn(
            "appendix must not duplicate the technical issue register",
            report_validator.validate_appendix(appendix),
        )

        issue_table = """| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| FIX-001 | moderate | Fixture. | engine-config | Fixture. | N/A. | Fixture. |
"""
        appendix = valid_appendix() + "\n" + issue_table
        self.assertIn(
            "appendix must not duplicate the technical issue register",
            report_validator.validate_appendix(appendix),
        )

    def test_pair_requires_stable_names_and_links(self) -> None:
        errors = report_validator.validate_pair(
            valid_report().replace("](fixture-evidence.md)", "](wrong.md)"),
            valid_appendix().replace("](fixture.md)", "](wrong.md)"),
            "fixture.md",
            "appendix.md",
        )
        self.assertIn(
            "appendix filename must be fixture-evidence.md, got appendix.md",
            errors,
        )
        self.assertIn("report must link companion appendix: appendix.md", errors)
        self.assertIn("appendix must link companion report: fixture.md", errors)

    def test_pair_reconciles_pack_identity_and_evaluation_date(self) -> None:
        renamed = valid_appendix().replace(
            "# Animation pack evidence appendix: Validator fixture",
            "# Animation pack evidence appendix: Completely Different Pack",
            1,
        )
        self.assertIn(
            "report and appendix must declare the same pack identity",
            report_validator.validate_pair(
                valid_report(), renamed, "fixture.md", "fixture-evidence.md"
            ),
        )
        redated = valid_appendix().replace("**2026-08-16**", "**2026-08-15**", 1)
        self.assertIn(
            "report and appendix must declare the same evaluation date",
            report_validator.validate_pair(
                valid_report(), redated, "fixture.md", "fixture-evidence.md"
            ),
        )

        duplicate_h1 = valid_report() + "\n# Animation pack evaluation: Validator fixture\n"
        self.assertIn(
            "report must start with '# Animation pack evaluation:'",
            report_validator.validate(duplicate_h1),
        )

    def test_pair_rejects_links_hidden_in_comments_or_wrong_declarations(self) -> None:
        report = valid_report().replace(
            "> Detailed evidence: [fixture evidence appendix](fixture-evidence.md)",
            "<!-- ](fixture-evidence.md) -->",
        )
        appendix = valid_appendix().replace(
            "> Companion report: [technical evaluation](fixture.md)",
            "<!-- ](fixture.md) -->",
        )
        errors = report_validator.validate_pair(
            report,
            appendix,
            "fixture.md",
            "fixture-evidence.md",
        )
        self.assertIn(
            "report must link companion appendix: fixture-evidence.md", errors
        )
        self.assertIn("appendix must link companion report: fixture.md", errors)

        for wrapper in ("`{}`", "\\{}"):
            with self.subTest(wrapper=wrapper):
                report_link = "[fixture evidence appendix](fixture-evidence.md)"
                appendix_link = "[technical evaluation](fixture.md)"
                errors = report_validator.validate_pair(
                    valid_report().replace(report_link, wrapper.format(report_link)),
                    valid_appendix().replace(appendix_link, wrapper.format(appendix_link)),
                    "fixture.md",
                    "fixture-evidence.md",
                )
                self.assertIn(
                    "report must link companion appendix: fixture-evidence.md", errors
                )
                self.assertIn("appendix must link companion report: fixture.md", errors)

    def test_all_published_report_pairs_conform(self) -> None:
        repository = Path(__file__).resolve().parents[4]
        report_directory = repository / "docs" / "reports"
        reports = sorted(
            report
            for report in report_directory.glob("*.md")
            if report.name != "README.md" and not report.stem.endswith("-evidence")
        )
        appendices = sorted(report_directory.glob("*-evidence.md"))
        self.assertTrue(reports, "expected at least one published pack report")
        self.assertEqual(
            {report.stem for report in reports},
            {appendix.stem.removesuffix("-evidence") for appendix in appendices},
            "every evidence appendix must have one primary report and vice versa",
        )
        for report in reports:
            appendix = report.with_name(f"{report.stem}-evidence.md")
            with self.subTest(report=report.name):
                self.assertTrue(appendix.is_file(), f"missing {appendix.name}")
                report_text = report.read_text(encoding="utf-8")
                appendix_text = appendix.read_text(encoding="utf-8")
                self.assertEqual(report_validator.validate(report_text), [])
                self.assertEqual(
                    report_validator.validate_appendix(appendix_text), []
                )
                self.assertEqual(
                    report_validator.validate_pair(
                        report_text,
                        appendix_text,
                        report.name,
                        appendix.name,
                    ),
                    [],
                )


class RegenerationContractTests(unittest.TestCase):
    repository = Path(__file__).resolve().parents[4]
    skill = repository / ".agents/skills/evaluate-animation-packs"

    def read(self, relative_path: str) -> str:
        return (self.skill / relative_path).read_text(encoding="utf-8")

    def rendered_paragraph_text(self, relative_path: str) -> str:
        document = report_validator.parse_markdown(self.read(relative_path))
        return "\n".join(paragraph["text"] for paragraph in document["paragraphs"])

    def test_templates_preserve_reviewed_provenance_boundary(self) -> None:
        skill = self.rendered_paragraph_text("SKILL.md")
        report_template = self.rendered_paragraph_text("assets/report-template.md")
        appendix_template = self.rendered_paragraph_text(
            "assets/evidence-appendix-template.md"
        )

        self.assertIn("collection-level or constituent-level", skill)
        self.assertIn("collection version does not identify", skill)
        self.assertIn("distinguish collection listing", report_template)
        self.assertIn("collection-level and constituent-level", appendix_template)

    def test_templates_preserve_primary_evidence_without_duplication(self) -> None:
        skill = self.rendered_paragraph_text("SKILL.md")
        report_template = self.rendered_paragraph_text("assets/report-template.md")
        appendix_template = self.rendered_paragraph_text(
            "assets/evidence-appendix-template.md"
        )

        self.assertIn("appendix must link directly to that evidence", skill)
        self.assertIn("Treat this table as decision evidence", report_template)
        self.assertIn("link to that table here", appendix_template)
        self.assertIn("without duplicating", appendix_template)

    def test_exact_source_typos_stay_narrow_and_root_motion_stays_safe(self) -> None:
        skill = self.rendered_paragraph_text("SKILL.md")
        taxonomy = self.rendered_paragraph_text("references/clip-taxonomy.md")
        appendix_template = self.rendered_paragraph_text(
            "assets/evidence-appendix-template.md"
        )

        self.assertIn("complete identifier", skill)
        self.assertIn("complete single-backtick code span", taxonomy)
        self.assertIn("<!-- vendor-id -->", taxonomy)
        self.assertIn("exact identifier", taxonomy)
        self.assertIn("spaces and punctuation", taxonomy)
        self.assertIn("an identical spelling in ordinary prose remains checked", taxonomy)
        self.assertIn("root translation or yaw", appendix_template)
        self.assertIn("displacement and yaw proof", appendix_template)

    def test_vendor_identifier_typos_are_marked_and_prose_is_not_exempt(self) -> None:
        config = (self.repository / "_typos.toml").read_text(encoding="utf-8")
        config_data = tomllib.loads(config)
        self.assertNotIn("extend-ignore-re", config_data["default"])
        self.assertEqual(
            config_data["files"]["extend-exclude"],
            ["*.fbx", "*.glb", "Cargo.lock"],
        )
        self.assertEqual(
            config_data["type"]["md"]["extend-ignore-re"],
            [report_validator.VENDOR_ID_MARKER_RE.pattern],
        )

        report_identifiers = {
            self.repository
            / "docs/reports/protofactor-basic-locomotion-evidence.md": "WalkForwadRight",
            self.repository
            / "docs/reports/protofactor-sword-and-shield-evidence.md": "ParryHight2",
        }
        for report, identifier in report_identifiers.items():
            text = report.read_text(encoding="utf-8")
            self.assertIn(f"`{identifier}`<!-- vendor-id -->", text)
            self.assertNotIn(f"vendor-id:{identifier}", text)

        marker = report_validator.VENDOR_ID_MARKER_RE
        synthetic_identifier = "Saber Foward & (Spin)/v2"
        self.assertRegex(
            f"the exact `{synthetic_identifier}`<!-- vendor-id --> member", marker
        )
        self.assertNotRegex(f"ordinary prose says {synthetic_identifier}", marker)
        self.assertIsNotNone(
            marker.fullmatch(f"`{synthetic_identifier}`<!-- vendor-id -->")
        )
        self.assertIsNone(marker.fullmatch("`Saber Foward`\n`member`<!-- vendor-id -->"))
        self.assertIsNone(marker.fullmatch("`Saber Foward`\r`member`<!-- vendor-id -->"))
        self.assertIsNone(
            marker.fullmatch("`Saber `Foward`<!-- vendor-id -->")
        )
        for delimiters in ("``", "```"):
            self.assertIsNone(
                marker.search(
                    f"{delimiters}{synthetic_identifier}`<!-- vendor-id -->"
                )
            )
        self.assertIsNone(
            marker.search(
                f"`first``{synthetic_identifier}`<!-- vendor-id -->"
            )
        )

        typos = shutil.which("typos")
        if typos is None:
            self.skipTest("typos is required by the full local gate")
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            marked = directory / "marked.md"
            ordinary_contexts = {
                "paragraph.md": f"Narrative accidentally repeats {synthetic_identifier}.\n",
                "heading.md": f"# {synthetic_identifier}\n",
                "table-cell.md": (
                    "| member |\n| --- |\n"
                    f"| {synthetic_identifier} |\n"
                ),
            }
            marked.write_text(
                f"The delivered name is `{synthetic_identifier}`<!-- vendor-id -->.\n",
                encoding="utf-8",
            )
            marked_result = subprocess.run(
                [typos, "--config", str(self.repository / "_typos.toml"), str(marked)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(marked_result.returncode, 0, marked_result.stderr)
            non_markdown = directory / "marked.txt"
            non_markdown.write_text(marked.read_text(encoding="utf-8"), encoding="utf-8")
            non_markdown_result = subprocess.run(
                [
                    typos,
                    "--config",
                    str(self.repository / "_typos.toml"),
                    str(non_markdown),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(non_markdown_result.returncode, 0, "marked.txt")
            self.assertIn(
                "Foward",
                non_markdown_result.stdout + non_markdown_result.stderr,
                "marked.txt",
            )
            for filename, contents in ordinary_contexts.items():
                context = directory / filename
                context.write_text(contents, encoding="utf-8")
                result = subprocess.run(
                    [
                        typos,
                        "--config",
                        str(self.repository / "_typos.toml"),
                        str(context),
                    ],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertNotEqual(result.returncode, 0, filename)
                self.assertIn("Foward", result.stdout + result.stderr, filename)
            for filename, contents in {
                "double-delimiter.md": f"The delivered name is ``{synthetic_identifier}`<!-- vendor-id -->.\n",
                "triple-delimiter.md": f"The delivered name is ```{synthetic_identifier}`<!-- vendor-id -->.\n",
                "adjacent-delimiters.md": f"The delivered names are `{synthetic_identifier}` `{synthetic_identifier}`<!-- vendor-id -->.\n",
                "escaped-opening.md": f"The delivered name is \\`{synthetic_identifier}`<!-- vendor-id -->.\n",
            }.items():
                context = directory / filename
                context.write_text(contents, encoding="utf-8")
                result = subprocess.run(
                    [
                        typos,
                        "--config",
                        str(self.repository / "_typos.toml"),
                        str(context),
                    ],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertNotEqual(result.returncode, 0, filename)
                self.assertIn("Foward", result.stdout + result.stderr, filename)

    def test_version_refresh_keeps_refusal_distinct_from_remediation(self) -> None:
        skill = self.rendered_paragraph_text("SKILL.md")
        report_template = self.rendered_paragraph_text("assets/report-template.md")
        appendix_template = self.rendered_paragraph_text(
            "assets/evidence-appendix-template.md"
        )
        engine_reference = self.rendered_paragraph_text(
            "references/engine-and-compatibility.md"
        )

        self.assertIn("Treat earlier generated outputs as historical", skill)
        self.assertIn("it is not a successful remediation", skill)
        self.assertIn(
            "A current safety refusal is not a successful remediation",
            report_template,
        )
        self.assertIn(
            "whether the current version accepted or refused", appendix_template
        )
        self.assertIn(
            "whether the operation produced output or refused",
            engine_reference,
        )

    def test_discovery_adapters_route_to_the_canonical_skill(self) -> None:
        adapter_path = (
            self.repository / ".claude/skills/evaluate-animation-packs/SKILL.md"
        )
        adapter = adapter_path.read_text(encoding="utf-8")
        document = report_validator.parse_markdown(adapter)
        destinations = [link["destination"] for link in document["links"]]

        self.assertEqual(
            destinations,
            ["../../../.agents/skills/evaluate-animation-packs/SKILL.md"],
        )
        self.assertFalse(document["has_raw_html"])
        self.assertEqual(document["placeholders"], [])
        canonical = (adapter_path.parent / destinations[0]).resolve()
        self.assertEqual(canonical, (self.skill / "SKILL.md").resolve())
        self.assertTrue(canonical.is_file())
        metadata = yaml.safe_load(self.read("agents/openai.yaml"))
        self.assertEqual(
            metadata,
            {
                "interface": {
                    "display_name": "Evaluate Animation Packs",
                    "short_description": (
                        "Assess game-engine animation pack readiness"
                    ),
                    "default_prompt": (
                        "Use $evaluate-animation-packs to assess this animation "
                        "pack for game-engine use and write the standard concise "
                        "technical report plus evidence appendix."
                    ),
                }
            },
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
            (root / "a-excluded").mkdir()
            (root / "a-excluded" / "Ignored.fbx").write_bytes(b"ignored-a")
            (root / "z-excluded").mkdir()
            (root / "z-excluded" / "Ignored.fbx").write_bytes(b"ignored-z")
            (root / "Alpha.fbx").write_bytes(b"motion")
            (root / "Walk.fbx").write_bytes(b"motion")
            arguments = (
                str(root),
                "--label",
                "Pack",
                "--exclude",
                "z-excluded",
                "--exclude",
                "a-excluded",
            )
            success = self.run_script(
                "inventory_pack.py", *arguments, hash_seed="1"
            )
            repeated = self.run_script(
                "inventory_pack.py", *arguments, hash_seed="2"
            )
            unhashed = self.run_script(
                "inventory_pack.py", *arguments, "--no-hash", hash_seed="3"
            )
            missing = self.run_script("inventory_pack.py", str(root / "missing"))

        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertEqual(repeated.returncode, 0, repeated.stderr)
        self.assertEqual(success.stdout, repeated.stdout)
        inventory = json.loads(success.stdout)
        self.assertEqual(inventory["pack_label"], "Pack")
        self.assertEqual(inventory["excluded_paths"], ["a-excluded", "z-excluded"])
        self.assertEqual(
            inventory["duplicate_file_groups"][0]["paths"],
            ["Alpha.fbx", "Walk.fbx"],
        )
        self.assertEqual(success.stderr, "")
        self.assertEqual(unhashed.returncode, 0, unhashed.stderr)
        unhashed_inventory = json.loads(unhashed.stdout)
        self.assertIsNone(unhashed_inventory["hash_algorithm"])
        self.assertEqual(unhashed_inventory["duplicate_file_groups"], [])
        self.assertTrue(
            all("sha256" not in record for record in unhashed_inventory["files"])
        )
        self.assertEqual(missing.returncode, 2)
        self.assertIn("root is not a directory", missing.stderr)

    def test_manifest_validator_cli_success_and_malformed_json(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            valid_path = directory / "valid.json"
            valid_path.write_text(json.dumps(valid_manifest()), encoding="utf-8")
            invalid_manifest = valid_manifest()
            invalid_manifest["profiles"] = invalid_manifest["profiles"][2:]  # type: ignore[index]
            invalid_manifest["pipeline_stages"] = invalid_manifest["pipeline_stages"][2:]  # type: ignore[index]
            invalid_manifest["motions"][0]["classification_basis"] = [  # type: ignore[index]
                "z-unknown",
                "a-unknown",
            ]
            invalid_path = directory / "invalid.json"
            invalid_path.write_text(json.dumps(invalid_manifest), encoding="utf-8")
            malformed_path = directory / "malformed.json"
            malformed_path.write_text("{", encoding="utf-8")
            success = self.run_script(
                "validate_evaluation_manifest.py", str(valid_path)
            )
            invalid_first = self.run_script(
                "validate_evaluation_manifest.py", str(invalid_path), hash_seed="1"
            )
            invalid_second = self.run_script(
                "validate_evaluation_manifest.py", str(invalid_path), hash_seed="2"
            )
            malformed = self.run_script(
                "validate_evaluation_manifest.py", str(malformed_path)
            )

        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertIn("validated animation-pack evaluation manifest", success.stdout)
        self.assertEqual(success.stderr, "")
        self.assertEqual(invalid_first.returncode, 1)
        self.assertEqual(invalid_second.returncode, 1)
        self.assertEqual(invalid_first.stderr, invalid_second.stderr)
        self.assertEqual(malformed.returncode, 2)
        self.assertIn("validate_evaluation_manifest.py:", malformed.stderr)

    def test_report_validator_cli_success_and_failures_across_pair(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            valid_path = directory / "fixture.md"
            valid_path.write_text(valid_report(), encoding="utf-8")
            valid_appendix_path = directory / "fixture-evidence.md"
            valid_appendix_path.write_text(valid_appendix(), encoding="utf-8")
            invalid_path = directory / "invalid.md"
            invalid_path.write_text(
                valid_report() + "\n{{ZETA_PLACEHOLDER}} {{ALPHA_PLACEHOLDER}}\n",
                encoding="utf-8",
            )
            invalid_appendix_path = directory / "invalid-evidence.md"
            invalid_appendix_path.write_text(
                valid_appendix().replace(
                    "## Mechanical baseline", "## Mechanical baseline omitted"
                ),
                encoding="utf-8",
            )
            broken_reverse_path = directory / "broken.md"
            broken_reverse_path.write_text(
                valid_report().replace("fixture-evidence.md", "broken-evidence.md"),
                encoding="utf-8",
            )
            broken_reverse_appendix = directory / "broken-evidence.md"
            broken_reverse_appendix.write_text(
                valid_appendix().replace("](fixture.md)", "](wrong.md)"),
                encoding="utf-8",
            )
            explicit_report_path = directory / "explicit.md"
            explicit_appendix_path = directory / "explicit-evidence.md"
            explicit_report_path.write_text(
                valid_report().replace(
                    "fixture-evidence.md", "explicit-evidence.md"
                ),
                encoding="utf-8",
            )
            explicit_appendix_path.write_text(
                valid_appendix().replace("fixture.md", "explicit.md"),
                encoding="utf-8",
            )
            success = self.run_script("validate_report.py", str(valid_path))
            direct = subprocess.run(
                [str(self.scripts / "validate_report.py"), str(valid_path)],
                check=False,
                capture_output=True,
                text=True,
            )
            invalid = self.run_script(
                "validate_report.py", str(invalid_path), hash_seed="1"
            )
            invalid_repeated = self.run_script(
                "validate_report.py", str(invalid_path), hash_seed="2"
            )
            malformed_appendix = self.run_script(
                "validate_report.py",
                str(invalid_path),
                "--appendix",
                str(invalid_appendix_path),
            )
            broken_reverse = self.run_script(
                "validate_report.py", str(broken_reverse_path)
            )
            explicit = self.run_script(
                "validate_report.py",
                str(explicit_report_path),
                "--appendix",
                str(explicit_appendix_path),
            )

        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertTrue(os.access(self.scripts / "validate_report.py", os.X_OK))
        self.assertEqual(direct.returncode, 0, direct.stderr)
        self.assertIn("validated animation-pack report", success.stdout)
        self.assertEqual(success.stderr, "")
        self.assertEqual(invalid.returncode, 1)
        self.assertEqual(invalid_repeated.returncode, 1)
        self.assertEqual(invalid.stderr, invalid_repeated.stderr)
        self.assertIn(
            "unresolved template placeholders: "
            "{{ALPHA_PLACEHOLDER}}, {{ZETA_PLACEHOLDER}}",
            invalid.stderr,
        )
        self.assertEqual(malformed_appendix.returncode, 1)
        self.assertIn(
            "missing required heading: ## Mechanical baseline",
            malformed_appendix.stderr,
        )
        self.assertEqual(broken_reverse.returncode, 1)
        self.assertIn(
            "appendix must link companion report: broken.md",
            broken_reverse.stderr,
        )
        self.assertEqual(explicit.returncode, 0, explicit.stderr)
        self.assertIn("validated animation-pack report pair", explicit.stdout)


def valid_collection_output_projection() -> dict[str, object]:
    """Self-authored projection of an independently validated Rust envelope."""
    unavailable = {"state": "unavailable", "reason": "source_unavailable"}
    clips = [
        {"id": "fixture:idle-ip", "source": "fixture", "take_index": 0, "take_name": "Idle IP", "binding": unavailable},
        {"id": "fixture:walk-rm", "source": "fixture", "take_index": 1, "take_name": "Walk RM", "binding": unavailable},
    ]
    members = [
        {"id": clip["id"], "resolution": unavailable, "root_travel": {"translation_availability": "unavailable", "speed_mps_availability": "unavailable"}}
        for clip in clips
    ]
    return {
        "schema": "urn:animsmith:schema:collection-output:2",
        "schema_version": 2,
        "tool": {"name": "animsmith", "version": "0.0.0", "source": {"revision": None, "dirty": None}},
        "command": "collection lint",
        "manifest": {
            "schema": "urn:animsmith:schema:collection-manifest:1",
            "schema_version": 1,
            "collection_id": "fixture-collection",
            "input": {"sha256": "a" * 64, "bytes": 17},
        },
        "budget": {"id": "urn:animsmith:collection-output-budget:1", "max_source_bytes": 1073741824, "max_aggregate_source_bytes": 17179869184, "max_serialized_bytes": 268435456, "max_sources": 4096, "max_clips": 4096, "max_runtime_sets": 4096, "max_aggregate_members": 16384, "max_aggregate_work": 24576},
        "summary": {"sources": 1, "readable_sources": 0, "established_sources": 0, "clips": 2, "established_clips": 0, "runtime_sets": 1, "complete_runtime_sets": 0, "incomplete": True},
        "work": {"manifest_rows": 4, "runtime_set_members": 2, "aggregate_work": 6, "primary_source_bytes": 0, "serialized_bytes": 0},
        "clips": clips,
        "sources": [{"key": "fixture", "locator": "fixture.gltf", "input": {"state": "unavailable", "reason": "missing", "inspected_bytes": 0}, "digest": {"state": "unpinned"}, "config": {"state": "default"}, "loader": unavailable, "take_inventory": "unavailable", "observed_takes": [], "result": unavailable}],
        "runtime_sets": [{"id": "fixture:paired", "kind": "paired-interaction", "members": members, "lifecycle": "incomplete", "decision": "not_evaluated", "gaps": ["source_unavailable"], "evidence": {"root_travel": {"lifecycle": "incomplete", "members_measured": 0}}}],
    }


def valid_evaluation_model() -> dict[str, object]:
    """License-safe synthetic V1 model exercising all value categories."""
    evidence = [{"id": "evidence-a", "kind": "observed-animsmith", "locator": "docs/synthetic-evidence.md", "summary": "Self-authored fixture evidence."}]
    profiles = sorted(({"id": profile_id, "status": "selected" if profile_id == "marketplace-intake" else "not-selected", "activation_basis": "user-required", "evidence_refs": ["evidence-a"]} for profile_id in model_contract.PROFILE_IDS), key=lambda item: item["id"])
    stages = sorted(({"id": stage_id, "coverage": "evaluated-clean", "evidence_refs": ["evidence-a"]} for stage_id in model_contract.PIPELINE_STAGES), key=lambda item: item["id"])
    readiness = [{"id": lane, "state": "unknown", "adoption_consequence": "More evidence is required.", "evidence_refs": ["evidence-a"]} for lane in sorted(model_contract.READINESS_LANES)]
    return {
        "schema": model_contract.SCHEMA,
        "schema_version": 1,
        "binding": {"collection_id": "fixture-collection", "manifest_sha256": "a" * 64, "manifest_bytes": 17},
        "presentation": {"id": "fixture-evaluation", "title": "Synthetic fixture", "evaluation_date": "2026-08-24", "verdict": "Restricted use", "completeness": "partial", "confidence": "low"},
        "evidence": evidence,
        "runs": [{"id": "current-run", "state": "current", "evidence_refs": ["evidence-a"], "summary": "Current refusal.", "supersedes": "historical-run"}, {"id": "historical-run", "state": "historical", "evidence_refs": ["evidence-a"], "summary": "Historical generated candidate."}],
        "clips": [
            {"id": "fixture:idle-ip", "source": "fixture", "take_index": 0, "take_name": "Idle IP", "primary_role": "idle-pose", "tags": [], "classification_basis": ["observed-file"], "evidence_refs": ["evidence-a"], "loop": "not-loop", "duration_s": {"state": "available", "value": 1.0}, "root_motion_speed_mps": {"state": "not-applicable"}, "movement_owner": "engine-config", "assessment": "pass", "coverage": "evaluated-clean"},
            {"id": "fixture:walk-rm", "source": "fixture", "take_index": 1, "take_name": "Walk RM", "primary_role": "continuous-locomotion", "tags": [], "classification_basis": ["observed-file"], "evidence_refs": ["evidence-a"], "loop": "loop", "duration_s": {"state": "available", "value": 1.0}, "root_motion_speed_mps": {"state": "available", "value": 1.0}, "movement_owner": "engine-config", "assessment": "finding", "coverage": "evaluated-finding"},
        ],
        "runtime_sets": [{"id": "fixture:paired", "kind": "paired-interaction", "members": [{"clip_id": "fixture:idle-ip", "eligibility": "complete"}, {"clip_id": "fixture:walk-rm", "eligibility": "incomplete"}], "assessment": "not-evaluated", "coverage": "not-evaluated", "evidence_refs": ["evidence-a"]}],
        "profiles": profiles, "pipeline_stages": stages,
        "readiness": readiness,
        "capabilities": [{"id": "contact-actions", "state": "not-evaluated", "evidence_refs": ["evidence-a"]}],
        "integration_steps": [{"id": "full-body", "order": 1, "action": "topology", "movement_owner": "engine-config", "phase_owner": "engine-config", "coordinates_or_thresholds": "none", "evidence_refs": ["evidence-a"]}],
        "issues": [{"id": "artist-contact", "severity": "major", "impact": "Contact evidence is missing.", "primary_owner": "artist-author", "current_action": "Author contact cleanup.", "future_candidate": "not-applicable", "secondary_workaround": "none", "evidence_refs": ["evidence-a"]}],
        "remediations": [
            {"id": "current-refusal", "run_id": "current-run", "state": "refused", "input_evidence_refs": ["evidence-a"], "output_id": "none", "refusal_evidence_refs": ["evidence-a"], "historical_output_id": "candidate-v0"},
            {"id": "historical-output", "run_id": "historical-run", "state": "produced", "input_evidence_refs": ["evidence-a"], "output_id": "candidate-v0", "refusal_evidence_refs": [], "historical_output_id": "none"},
        ],
        "engine_evidence": [{"id": "unity", "runtime": "Unity", "version": "unknown", "level": "not-evaluated", "coverage": "not-evaluated", "settings": "not-evaluated", "procedure": "not-evaluated", "evidence_refs": ["evidence-a"]}],
        "limitations": [{"id": "unknown-engine", "summary": "Runtime behavior is unknown.", "evidence_refs": ["evidence-a"]}],
        "sources": [{"id": "source-record", "source_commit": "fixture", "report_sha256": "b" * 64, "acquisition_scope": "synthetic", "license_scope": "self-authored", "evidence_kind": "documentation-stated", "evidence_refs": ["evidence-a"]}],
        "narratives": [{"id": "technical-decision", "slot": "technical-decision", "text": "A fixed-slot interpretation.", "fact_refs": ["fixture:idle-ip"]}],
        "collection": {"constituents": [], "exclusions": [], "cross_pack_records": []},
    }


class EvaluationModelV1Tests(unittest.TestCase):
    def test_synthetic_fixture_covers_pair_refusal_unknown_and_artist_work(self) -> None:
        self.assertEqual(model_validator.validate_model(valid_evaluation_model(), valid_collection_output_projection()), [])

    def test_fixed_renderer_is_deterministic_ast_valid_and_model_bound(self) -> None:
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        first = model_renderer.render_views(model, binding, report_name="fixture.md", appendix_name="fixture-evidence.md")
        second = model_renderer.render_views(copy.deepcopy(model), binding, report_name="fixture.md", appendix_name="fixture-evidence.md")
        self.assertEqual(first, second)
        self.assertEqual(model_renderer.validate_views(model, binding, first, report_name="fixture.md", appendix_name="fixture-evidence.md"), [])
        self.assertLessEqual(report_validator.rendered_word_count(first.report), report_validator.MAX_PRIMARY_WORDS)
        stale = model_renderer.RenderedViews(first.report.replace("fixture:walk-rm", "fixture:stale"), first.appendix.replace("fixture:walk-rm", "fixture:stale"))
        self.assertTrue(model_renderer.validate_views(model, binding, stale, report_name="fixture.md", appendix_name="fixture-evidence.md"))
        hostile = copy.deepcopy(model)
        hostile_binding = copy.deepcopy(binding)
        hostile["clips"][0]["take_name"] = "[hostile] | <raw>"  # type: ignore[index]
        hostile_binding["clips"][0]["take_name"] = "[hostile] | <raw>"  # type: ignore[index]
        escaped = model_renderer.render_views(hostile, hostile_binding, report_name="fixture.md", appendix_name="fixture-evidence.md")
        self.assertEqual(model_renderer.validate_views(hostile, hostile_binding, escaped, report_name="fixture.md", appendix_name="fixture-evidence.md"), [])
        # Exact authority values may contain hostile punctuation in the ledger,
        # but the pinned parser must keep it in code spans rather than HTML.
        self.assertFalse(report_validator.parse_markdown(escaped.appendix)["has_raw_html"])

    def test_renderer_ledger_rejects_a_mutation_in_every_authority_family(self) -> None:
        """The parsed fixed tables, rather than a provenance blob, are the view proof."""
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["collection"] = {
            "constituents": [
                {"id": "basic", "model_sha256": "b" * 64, "clip_ids": ["fixture:idle-ip"], "source_file_count": 1, "runtime_set_ids": ["fixture:paired"]},
                {"id": "sword", "model_sha256": "c" * 64, "clip_ids": ["fixture:walk-rm"], "source_file_count": 0, "runtime_set_ids": []},
            ],
            "exclusions": [],
            "cross_pack_records": [{"id": "basic-sword", "left": "basic", "right": "sword", "result": "artist-required", "evidence_refs": ["evidence-a"]}],
        }
        self.assertEqual(model_validator.validate_model(model, binding), [])
        views = model_renderer.render_views(model, binding, report_name="fixture.md", appendix_name="fixture-evidence.md")
        for title, columns, records in model_renderer._ledger_sections(model, binding):
            if not records:
                continue
            for column in columns:
                with self.subTest(authority_family=title, field=column):
                    old_cells = [model_renderer._literal(records[0].get(name)) for name in columns]
                    new_cells = old_cells.copy()
                    new_cells[columns.index(column)] = model_renderer._literal("tampered")
                    old_row = "| " + " | ".join(old_cells) + " |"
                    new_row = "| " + " | ".join(new_cells) + " |"
                    altered = views.appendix.replace(old_row, new_row, 1)
                    self.assertNotEqual(altered, views.appendix)
                errors = model_renderer.validate_views(model, binding, model_renderer.RenderedViews(views.report, altered), report_name="fixture.md", appendix_name="fixture-evidence.md")
                self.assertTrue(any("model-to-view" in error for error in errors), errors)

    def test_renderer_preserves_hostile_public_link_destinations_in_the_ast(self) -> None:
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["evidence"][0]["locator"] = "https://example.test/a_(b)?q=(c)&encoded=%3Cvalue%3E"  # type: ignore[index]
        self.assertEqual(model_validator.validate_model(model, binding), [])
        appendix = model_renderer.render_views(model, binding, report_name="fixture.md", appendix_name="fixture-evidence.md").appendix
        self.assertIn(("Self-authored fixture evidence.", "https://example.test/a_(b)?q=(c)&encoded=%3Cvalue%3E"), {(link["text"], link["destination"]) for link in report_validator.parse_markdown(appendix)["links"]})

    def test_renderer_counts_one_bound_source_in_each_applicable_role_and_keeps_total_unique(self) -> None:
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["issues"][0]["secondary_workaround"] = "Use a full-body handoff."  # type: ignore[index]
        views = model_renderer.render_views(model, binding, report_name="fixture.md", appendix_name="fixture-evidence.md")
        self.assertEqual(model_renderer.validate_views(model, binding, views, report_name="fixture.md", appendix_name="fixture-evidence.md"), [])
        appendix = report_validator.parse_markdown(views.appendix)
        role_table = next(table for table in appendix["tables"] if tuple(cell["text"] for cell in table["header"]) == report_validator.ROLE_HEADER)
        role_counts = {row[0]["text"]: row[2]["text"] for row in role_table["rows"]}
        self.assertEqual(role_counts["idle-pose"], "1")
        self.assertEqual(role_counts["continuous-locomotion"], "1")
        self.assertEqual(role_counts["Total"], "1")
        issue_table = next(table for table in report_validator.parse_markdown(views.report)["tables"] if tuple(cell["text"] for cell in table["header"]) == report_validator.ISSUE_HEADER)
        self.assertIn("Use a full-body handoff.", issue_table["rows"][0][2]["text"])

    def test_renderer_refuses_input_output_and_output_output_aliases(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            model, binding = directory / "model.json", directory / "binding.json"
            model.write_bytes(model_contract.canonical_json(valid_evaluation_model()))
            binding.write_bytes(model_contract.canonical_json(valid_collection_output_projection()))
            with self.assertRaises(ValueError):
                model_renderer._checked_paths(model, binding, model, directory / "appendix.md")
            with self.assertRaises(ValueError):
                model_renderer._checked_paths(model, binding, directory / "same.md", directory / "." / "same.md")
            output = directory / "output.md"; output.write_text("old\n", encoding="utf-8")
            output_hardlink = directory / "output-hardlink.md"; os.link(output, output_hardlink)
            with self.assertRaises(ValueError):
                model_renderer._checked_paths(model, binding, output, output_hardlink)
            hardlink = directory / "hardlink.md"
            os.link(binding, hardlink)
            with self.assertRaises(ValueError):
                model_renderer._checked_paths(model, binding, hardlink, directory / "appendix.md")
            symlink = directory / "symlink.md"
            try:
                symlink.symlink_to(model)
            except OSError as error:
                self.skipTest(f"symlink unavailable: {error}")
            with self.assertRaises(ValueError):
                model_renderer._checked_paths(model, binding, symlink, directory / "appendix.md")

    def test_renderer_pair_failure_restores_both_outputs_and_cleans_staging(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            report, appendix = directory / "report.md", directory / "appendix.md"
            report.write_bytes(b"old report\n")
            appendix.write_bytes(b"old appendix\n")
            views = model_renderer.RenderedViews("new report\n", "new appendix\n")
            real_replace = os.replace
            calls = 0

            def fail_second(source: os.PathLike[str] | str, destination: os.PathLike[str] | str) -> None:
                nonlocal calls
                if Path(destination) == appendix:
                    calls += 1
                    if calls == 1:
                        raise OSError("injected second publish failure")
                real_replace(source, destination)

            with mock.patch.object(model_renderer.os, "replace", side_effect=fail_second):
                with self.assertRaises(OSError):
                    model_renderer._publish_pair(report, appendix, views)
            self.assertEqual(report.read_bytes(), b"old report\n")
            self.assertEqual(appendix.read_bytes(), b"old appendix\n")
            self.assertEqual(list(directory.glob("tmp*")), [])
            report.unlink(); appendix.unlink(); calls = 0
            with mock.patch.object(model_renderer.os, "replace", side_effect=fail_second):
                with self.assertRaises(OSError):
                    model_renderer._publish_pair(report, appendix, views)
            self.assertFalse(report.exists())
            self.assertFalse(appendix.exists())
            self.assertEqual(list(directory.glob("tmp*")), [])
            report.mkdir()
            with self.assertRaises(ValueError):
                model_renderer._publish_pair(report, appendix, views)
            self.assertEqual(list(directory.glob("tmp*")), [])
            self.assertEqual(list(directory.glob(".animsmith-render-*")), [])
            report.rmdir()
            fifo = directory / "output.fifo"
            try:
                os.mkfifo(fifo)
            except (AttributeError, OSError) as error:
                self.assertIsNotNone(error)  # Directory refusal above is portable.
            else:
                with self.assertRaises(ValueError):
                    model_renderer._publish_pair(fifo, appendix, views)
                self.assertEqual(list(directory.glob(".animsmith-render-*")), [])
                fifo.unlink()
            with report.open("wb") as handle:
                handle.truncate(64 * 1024 * 1024)
            appendix.write_bytes(b"small old appendix\n")
            calls = 0
            with mock.patch.object(Path, "read_bytes", side_effect=AssertionError("must not snapshot output bytes")):
                with mock.patch.object(model_renderer.os, "replace", side_effect=fail_second):
                    with self.assertRaises(OSError):
                        model_renderer._publish_pair(report, appendix, views)
            self.assertEqual(report.stat().st_size, 64 * 1024 * 1024)
            self.assertEqual(appendix.read_bytes(), b"small old appendix\n")
            self.assertEqual(list(directory.glob(".animsmith-render-*")), [])

    def test_renderer_cli_check_is_byte_exact_and_never_writes_stale_views(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            model_path, binding_path = directory / "model.json", directory / "binding.json"
            report_path, appendix_path = directory / "fixture.md", directory / "fixture-evidence.md"
            model_path.write_bytes(model_contract.canonical_json(valid_evaluation_model()))
            binding_path.write_bytes(model_contract.canonical_json(valid_collection_output_projection()))
            command = [str(Path(__file__).with_name("render_evaluation_model.py")), str(model_path), "--binding", str(binding_path), "--report", str(report_path), "--appendix", str(appendix_path)]
            rendered = subprocess.run(command, check=False, text=True, capture_output=True)
            self.assertEqual(rendered.returncode, 0, rendered.stderr)
            clean = subprocess.run(command + ["--check"], check=False, text=True, capture_output=True)
            self.assertEqual(clean.returncode, 0, clean.stderr)
            before = report_path.read_bytes(), appendix_path.read_bytes()
            report_path.write_text("stale\n", encoding="utf-8")
            stale = subprocess.run(command + ["--check"], check=False, text=True, capture_output=True)
            self.assertEqual(stale.returncode, 1, stale.stderr)
            self.assertEqual(report_path.read_text(encoding="utf-8"), "stale\n")
            self.assertEqual(appendix_path.read_bytes(), before[1])
            missing_parent = directory / "check-only" / "nested"
            check_only = subprocess.run(
                command[:4] + ["--report", str(missing_parent / "report.md"), "--appendix", str(missing_parent / "report-evidence.md"), "--check"],
                check=False, text=True, capture_output=True,
            )
            self.assertEqual(check_only.returncode, 1, check_only.stderr)
            self.assertFalse(missing_parent.parent.exists())
            seeded_views = []
            for seed in ("1", "2"):
                seeded_directory = directory / f"seed-{seed}"
                seeded_directory.mkdir()
                seeded_report = seeded_directory / "fixture.md"
                seeded_appendix = seeded_directory / "fixture-evidence.md"
                environment = os.environ.copy()
                environment["PYTHONHASHSEED"] = seed
                seeded = subprocess.run(
                    command[:4] + ["--report", str(seeded_report), "--appendix", str(seeded_appendix)],
                    check=False, text=True, capture_output=True, env=environment,
                )
                self.assertEqual(seeded.returncode, 0, seeded.stderr)
                seeded_views.append((seeded_report.read_bytes(), seeded_appendix.read_bytes()))
            self.assertEqual(seeded_views[0], seeded_views[1])

    def test_no_set_and_no_issue_sentinels_are_valid(self) -> None:
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["issues"] = []
        model["runtime_sets"] = []
        binding["runtime_sets"] = []
        self.assertEqual(model_validator.validate_model(model, binding), [])

    def test_rejects_unknown_dangling_duplicate_and_stale_identity(self) -> None:
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["unknown"] = "no"
        self.assertTrue(any("unknown fields" in error for error in model_validator.validate_model(model, binding)))
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["clips"].append(copy.deepcopy(model["clips"][0]))  # type: ignore[index]
        self.assertTrue(any("duplicate id" in error for error in model_validator.validate_model(model, binding)))
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["remediations"][1]["historical_output_id"] = "missing"  # type: ignore[index]
        model["binding"]["manifest_sha256"] = "b" * 64  # type: ignore[index]
        errors = model_validator.validate_model(model, binding)
        self.assertTrue(any("stale" in error or "historical_output_id" in error for error in errors))

    def test_collection_pairs_are_derived_and_n_plus_one_is_rejected(self) -> None:
        model = valid_evaluation_model()
        model["collection"] = {
            "constituents": [{"id": "basic", "model_sha256": "b" * 64, "clip_ids": ["fixture:idle-ip"], "source_file_count": 1, "runtime_set_ids": ["fixture:paired"]}, {"id": "sword", "model_sha256": "c" * 64, "clip_ids": ["fixture:walk-rm"], "source_file_count": 0, "runtime_set_ids": []}],
            "exclusions": [],
            "cross_pack_records": [{"id": "basic-sword", "left": "basic", "right": "sword", "result": "artist-required", "evidence_refs": ["evidence-a"]}],
        }
        self.assertEqual(model_validator.validate_model(model, valid_collection_output_projection()), [])
        model["collection"]["constituents"].append({"id": "third", "model_sha256": "d" * 64, "clip_ids": [], "source_file_count": 0, "runtime_set_ids": []})  # type: ignore[index]
        self.assertIn("model.collection.cross_pack_records must contain exactly one derived unordered pair per constituent tuple", model_validator.validate_model(model, valid_collection_output_projection()))
        bounded = valid_evaluation_model()
        bounded["collection"]["constituents"] = [  # type: ignore[index]
            {"id": f"constituent-{index}", "model_sha256": "b" * 64, "clip_ids": [], "source_file_count": 0, "runtime_set_ids": []}
            for index in range(model_contract.MAX_COLLECTION_CONSTITUENTS + 1)
        ]
        schema_path = Path(__file__).parents[1] / "schemas" / "evaluation-model-v1.schema.json"
        validator = jsonschema.Draft202012Validator(json.loads(schema_path.read_text(encoding="utf-8")))
        self.assertTrue(list(validator.iter_errors(bounded)))
        self.assertTrue(model_validator.validate_model(bounded, valid_collection_output_projection()))

    def test_canonical_round_trip_number_domain_and_hash_seed_determinism(self) -> None:
        model = valid_evaluation_model()
        model["presentation"]["ratio"] = 1.0  # type: ignore[index]
        # The added field is rightly rejected by the closed schema, while the
        # canonical encoder itself remains stable for permitted JSON values.
        self.assertIn("unknown fields", "\n".join(model_validator.validate_model(model, valid_collection_output_projection())))
        value = {"z": 1e-07, "a": -0.0, "nested": [1.0, 1.25]}
        expected = b'{"a":0,"nested":[1,1.25],"z":1e-7}'
        self.assertEqual(model_contract.canonical_json(value), expected)
        reparsed = json.loads(model_contract.canonical_json(valid_evaluation_model()))
        self.assertEqual(model_contract.canonical_json(reparsed), model_contract.canonical_json(valid_evaluation_model()))
        self.assertEqual(model_contract.canonical_digest(value), model_contract.canonical_digest(dict(reversed(list(value.items())))))
        with self.assertRaises(model_contract.CanonicalJsonError):
            model_contract.canonical_json(float("nan"))
        self.assertEqual(model_contract.canonical_json(True), b"true")
        with self.assertRaises(model_contract.CanonicalJsonError):
            model_contract.canonical_number(True)
        with self.assertRaises(model_contract.CanonicalJsonError):
            model_contract.canonical_json({1: "not-a-string-key"})  # type: ignore[dict-item]
        too_deep: object = None
        for _ in range(model_contract.MAX_DEPTH + 1):
            too_deep = [too_deep]
        with self.assertRaises(model_contract.CanonicalJsonError):
            model_contract.canonical_json(too_deep)

    def test_schema_matches_the_closed_validator_root(self) -> None:
        schema_path = Path(__file__).parents[1] / "schemas" / "evaluation-model-v1.schema.json"
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        self.assertEqual(schema["$id"], model_contract.SCHEMA)
        self.assertEqual(set(schema["required"]), set(valid_evaluation_model()))
        self.assertFalse(schema["additionalProperties"])
        validator = jsonschema.Draft202012Validator(schema)
        self.assertEqual(list(validator.iter_errors(valid_evaluation_model())), [])
        malformed = valid_evaluation_model()
        malformed["clips"] = [{"id": "only-id"}]
        self.assertTrue(list(validator.iter_errors(malformed)))

    def test_schema_catalog_enums_are_the_python_contract_catalogs(self) -> None:
        schema_path = Path(__file__).parents[1] / "schemas" / "evaluation-model-v1.schema.json"
        definitions = json.loads(schema_path.read_text(encoding="utf-8"))["$defs"]
        self.assertEqual(set(definitions["profile"]["properties"]["id"]["enum"]), set(model_contract.PROFILE_IDS))
        self.assertEqual(set(definitions["pipeline_stage"]["properties"]["id"]["enum"]), set(model_contract.PIPELINE_STAGES))
        self.assertEqual(set(definitions["readiness"]["properties"]["id"]["enum"]), set(model_contract.READINESS_LANES))

    def test_schema_and_validator_reject_the_same_local_mutations(self) -> None:
        """Keep every locally expressible closed-model rule in both authorities."""
        schema_path = Path(__file__).parents[1] / "schemas" / "evaluation-model-v1.schema.json"
        validator = jsonschema.Draft202012Validator(json.loads(schema_path.read_text(encoding="utf-8")))
        mutations = (
            ("presentation-id", lambda m: m["presentation"].update(id="Uppercase")),
            ("presentation-title", lambda m: m["presentation"].update(title="")),
            ("run-summary", lambda m: m["runs"][0].update(summary="")),
            ("private-locator", lambda m: m["evidence"][0].update(locator="file:///home/fixture/private.fbx")),
            ("remediation-private-output", lambda m: m["remediations"][0].update(historical_output_id="file:///home/fixture/output.glb")),
            ("remediation-windows-output", lambda m: m["remediations"][1].update(output_id="C:\\fixture\\output.glb")),
            ("source-evidence-kind", lambda m: m["sources"][0].update(evidence_kind="garbage")),
            ("narrative-table", lambda m: m["narratives"][0].update(text="| state | count |")),
            ("remediation-output-type", lambda m: m["remediations"][0].update(output_id=7)),
            ("remediation-produced-history", lambda m: m["remediations"][1].update(historical_output_id="old")),
            ("classification-bound", lambda m: m["clips"][0].update(classification_basis=["observed-file"] * (model_contract.MAX_RECORDS + 1))),
            ("member-bound", lambda m: m["runtime_sets"][0].update(members=[{"clip_id": "fixture:idle-ip", "eligibility": "complete"}] * (model_contract.MAX_RECORDS + 1))),
            ("exclusion-reason", lambda m: m["collection"].update(exclusions=[{"id": "excluded", "reason": 7, "evidence_refs": []}])),
            ("exclusion-bound", lambda m: m["collection"].update(exclusions=[{"id": f"excluded-{index}", "reason": "Synthetic exclusion.", "evidence_refs": []} for index in range(model_contract.MAX_RECORDS + 1)])),
            ("pair-bound", lambda m: m["collection"].update(cross_pack_records=[{"id": f"pair-{index}", "left": "a", "right": "b", "result": "unknown", "evidence_refs": []} for index in range(model_contract.MAX_RECORDS + 1)])),
        )
        for label, mutate in mutations:
            with self.subTest(label=label):
                model = valid_evaluation_model()
                mutate(model)
                self.assertTrue(list(validator.iter_errors(model)), label)
                self.assertTrue(model_validator.validate_model(model, valid_collection_output_projection()), label)

    def test_public_locators_and_plain_narrative_have_positive_and_negative_alignment(self) -> None:
        schema_path = Path(__file__).parents[1] / "schemas" / "evaluation-model-v1.schema.json"
        validator = jsonschema.Draft202012Validator(json.loads(schema_path.read_text(encoding="utf-8")))
        for locator in (
            "https://example.test",
            "https://example.test/proof?revision=v1#evidence",
            "https://example.test/proof%3Csource%3E.json",
            "docs/synthetic-evidence.md",
            "references/assessment-taxonomy.md",
            "evidence/synthetic-output.json",
        ):
            with self.subTest(locator=locator):
                model = valid_evaluation_model()
                model["evidence"][0]["locator"] = locator  # type: ignore[index]
                self.assertEqual(list(validator.iter_errors(model)), [])
                self.assertEqual(model_validator.validate_model(model, valid_collection_output_projection()), [])
        for locator in (
            "docs/..",
            "https://example.test/foo/..",
            "docs/private-evaluator-notes.fbx",
            "reports/licensed-motion.glb",
            "evidence/raw-pack.zip",
            "https://example.test/proof<source>.json",
            "https://example.test/proof>source.json",
        ):
            with self.subTest(invalid_locator=locator):
                model = valid_evaluation_model()
                model["evidence"][0]["locator"] = locator  # type: ignore[index]
                self.assertTrue(list(validator.iter_errors(model)))
                self.assertTrue(model_validator.validate_model(model, valid_collection_output_projection()))
                if "<" in locator or ">" in locator:
                    with self.assertRaises(ValueError):
                        model_renderer.render_views(model, valid_collection_output_projection(), report_name="fixture.md", appendix_name="fixture-evidence.md")
        ordinary = valid_evaluation_model()
        ordinary["narratives"][0]["text"] = "The evaluator notes: linked observations remain conservative."  # type: ignore[index]
        self.assertEqual(list(validator.iter_errors(ordinary)), [])
        self.assertEqual(model_validator.validate_model(ordinary, valid_collection_output_projection()), [])
        slash_prose = valid_evaluation_model()
        slash_prose["narratives"][0]["text"] = "The evaluator considers and/or alternatives."  # type: ignore[index]
        self.assertEqual(list(validator.iter_errors(slash_prose)), [])
        self.assertEqual(model_validator.validate_model(slash_prose, valid_collection_output_projection()), [])
        for text in (
            "Usable with conditions.", "Poor fit.", "Insufficient technical evidence.", "not applicable.",
            "/root/.ssh/id_rsa", "/opt/evaluation/clip.fbx", "/srv/private/clip.fbx", "/évaluation/clip.fbx", "/~evaluator/clip.fbx", "/$private/clip.fbx",
            r"\\server\share\private.fbx", r"C:\\fixture\\private.fbx", "../private/clip.fbx", r"..\private\clip.fbx",
            "file:///private.fbx", "data:application/json,{}", "https://user:pass@example.test/private", "docs/private-evaluator-notes.fbx", "Model:ABC", "model:Abc",
        ):
            with self.subTest(text=text):
                model = valid_evaluation_model()
                model["narratives"][0]["text"] = text  # type: ignore[index]
                self.assertTrue(list(validator.iter_errors(model)))
                self.assertTrue(model_validator.validate_model(model, valid_collection_output_projection()))

    def test_schema_runtime_rejects_newline_terminators_and_malformed_nested_arrays(self) -> None:
        schema_path = Path(__file__).parents[1] / "schemas" / "evaluation-model-v1.schema.json"
        validator = jsonschema.Draft202012Validator(json.loads(schema_path.read_text(encoding="utf-8")))
        for mutate in (
            lambda m: m["presentation"].update(id="fixture-evaluation\n"),
            lambda m: m["evidence"][0].update(locator="docs/proof.md\n"),
            lambda m: m["clips"][0].update(tags=None),
            lambda m: m["clips"][0].update(classification_basis=None),
            lambda m: m["clips"][0].update(evidence_refs=None),
            lambda m: m["runtime_sets"][0].update(members=None),
            lambda m: m["runtime_sets"][0].update(evidence_refs=None),
            lambda m: m["remediations"][0].update(input_evidence_refs=None),
            lambda m: m["remediations"][0].update(refusal_evidence_refs=None),
            lambda m: m["collection"].update(constituents=[{"id": "fixture", "model_sha256": "b" * 64, "clip_ids": None, "source_file_count": 0, "runtime_set_ids": None}]),
            lambda m: m["collection"].update(constituents=[{"id": "fixture", "model_sha256": "b" * 64, "clip_ids": [["nonhashable"]], "source_file_count": 0, "runtime_set_ids": []}]),
        ):
            with self.subTest(mutate=mutate):
                model = valid_evaluation_model()
                mutate(model)
                self.assertTrue(list(validator.iter_errors(model)))
                self.assertTrue(model_validator.validate_model(model, valid_collection_output_projection()))

    def test_exact_binding_and_catalog_sets_are_relational_requirements(self) -> None:
        for label, mutate, needle in (
            ("clips", lambda m: m["clips"].pop(), "binding clip IDs"),
            ("sets", lambda m: m["runtime_sets"].clear(), "binding runtime-set IDs"),
            ("profiles", lambda m: m["profiles"].pop(), "validation profile"),
            ("stages", lambda m: m["pipeline_stages"].pop(), "canonical stages"),
            ("readiness", lambda m: m["readiness"].pop(), "readiness lane"),
        ):
            with self.subTest(label=label):
                model = valid_evaluation_model()
                mutate(model)
                self.assertTrue(any(needle in error for error in model_validator.validate_model(model, valid_collection_output_projection())))

    def test_runtime_set_witness_and_remediation_shapes_are_closed(self) -> None:
        model = valid_evaluation_model()
        model["runtime_sets"][0]["kind"] = "speed-blend"  # type: ignore[index]
        self.assertTrue(any("preserve binding kind" in error for error in model_validator.validate_model(model, valid_collection_output_projection())))
        model = valid_evaluation_model()
        model["runtime_sets"][0]["members"].reverse()  # type: ignore[index]
        self.assertTrue(any("member order" in error for error in model_validator.validate_model(model, valid_collection_output_projection())))
        model = valid_evaluation_model()
        model["remediations"][0]["output_id"] = "candidate-v1"  # type: ignore[index]
        self.assertTrue(model_validator.validate_model(model, valid_collection_output_projection()))
        model = valid_evaluation_model()
        model["remediations"][0]["state"] = "not-run"  # type: ignore[index]
        self.assertTrue(model_validator.validate_model(model, valid_collection_output_projection()))

    def test_refusal_cannot_link_an_output_from_the_current_run(self) -> None:
        model = valid_evaluation_model()
        model["remediations"][1]["run_id"] = "current-run"  # type: ignore[index]
        errors = model_validator.validate_model(model, valid_collection_output_projection())
        self.assertTrue(any("historical_output_id" in error for error in errors))

    def test_collection_constituents_and_exclusions_are_disjoint(self) -> None:
        model = valid_evaluation_model()
        model["collection"] = {
            "constituents": [{"id": "fixture", "model_sha256": "b" * 64, "clip_ids": ["fixture:idle-ip", "fixture:walk-rm"], "source_file_count": 1, "runtime_set_ids": ["fixture:paired"]}],
            "exclusions": [{"id": "fixture", "reason": "Excluded only for the overlap regression.", "evidence_refs": ["evidence-a"]}],
            "cross_pack_records": [],
        }
        self.assertTrue(any("disjointly" in error for error in model_validator.validate_model(model, valid_collection_output_projection())))

    def test_bounded_json_loader_rejects_depth_constants_and_duplicate_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name, raw in (("constant", b'{"x":Infinity}'), ("duplicate", b'{"x":1,"x":2}')):
                path = directory / f"{name}.json"
                path.write_bytes(raw)
                with self.subTest(name=name), self.assertRaises(ValueError):
                    model_validator.load_json(path)
            nested = "[" * (model_contract.MAX_DEPTH + 2) + "0" + "]" * (model_contract.MAX_DEPTH + 2)
            path = directory / "deep.json"; path.write_text(nested, encoding="utf-8")
            with self.assertRaises(ValueError):
                model_validator.load_json(path)

    def test_cli_distinguishes_invalid_model_from_invalid_json_and_noncanonical_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            binding = directory / "binding.json"; binding.write_bytes(model_contract.canonical_json(valid_collection_output_projection()))
            model = directory / "model.json"; model.write_text(json.dumps(valid_evaluation_model()), encoding="utf-8")
            noncanonical = subprocess.run([str(Path(__file__).with_name("validate_evaluation_model.py")), str(model), "--binding", str(binding), "--check-canonical"], check=False, text=True, capture_output=True)
            model.write_text("{", encoding="utf-8")
            malformed = subprocess.run([str(Path(__file__).with_name("validate_evaluation_model.py")), str(model), "--binding", str(binding)], check=False, text=True, capture_output=True)
            model.write_bytes(model_contract.canonical_json(valid_evaluation_model()))
            binding.write_text("{", encoding="utf-8")
            malformed_binding = subprocess.run([str(Path(__file__).with_name("validate_evaluation_model.py")), str(model), "--binding", str(binding)], check=False, text=True, capture_output=True)
            binding.write_bytes(model_contract.canonical_json(valid_collection_output_projection()))
            success = subprocess.run([str(Path(__file__).with_name("validate_evaluation_model.py")), str(model), "--binding", str(binding), "--check-canonical"], check=False, text=True, capture_output=True)
        self.assertEqual(noncanonical.returncode, 1, noncanonical.stderr)
        self.assertEqual(malformed.returncode, 2, malformed.stderr)
        self.assertEqual(malformed_binding.returncode, 2, malformed_binding.stderr)
        self.assertEqual(success.returncode, 0, success.stderr)
        self.assertIn("validated animation-pack evaluation model", success.stdout)

    def test_duplicate_binding_source_keys_are_rejected(self) -> None:
        binding = valid_collection_output_projection()
        binding["sources"].append({"key": "fixture"})  # type: ignore[index]
        self.assertTrue(any("duplicate key" in error for error in model_validator.validate_model(valid_evaluation_model(), binding)))

    def test_binding_requires_the_complete_offline_collection_output_envelope(self) -> None:
        for mutate in (
            lambda value: value.pop("schema_version"),
            lambda value: value.update(command="lint"),
            lambda value: value.update(schema="urn:example:wrong"),
            lambda value: value.update(extra=True),
            lambda value: value.update(summary=[]),
        ):
            binding = valid_collection_output_projection()
            mutate(binding)
            self.assertTrue(model_validator.validate_model(valid_evaluation_model(), binding))

    def test_bounded_reader_and_relations_fail_closed_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            oversized = directory / "oversized.json"
            oversized.write_bytes(b"{" + b" " * model_contract.MAX_MODEL_BYTES)
            with self.assertRaises(ValueError):
                model_validator.load_json(oversized)
        model = valid_evaluation_model()
        model["runs"][0]["supersedes"] = "current-run"  # type: ignore[index]
        errors = model_validator.validate_model(model, valid_collection_output_projection())
        self.assertTrue(any("historical run" in error for error in errors))

    def test_remediation_states_and_empty_source_projection_fail_closed(self) -> None:
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["remediations"].append(copy.deepcopy(model["remediations"][1]))  # type: ignore[index]
        model["remediations"][2]["id"] = "duplicate-output"  # type: ignore[index]
        errors = model_validator.validate_model(model, binding)
        self.assertTrue(any("duplicate produced output_id" in error for error in errors))
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        binding["sources"] = []
        model["collection"] = {
            "constituents": [
                {"id": "basic", "model_sha256": "b" * 64, "clip_ids": ["fixture:idle-ip", "fixture:walk-rm"], "source_file_count": 1, "runtime_set_ids": ["fixture:paired"]}
            ],
            "exclusions": [], "cross_pack_records": [],
        }
        self.assertTrue(any("source_file_count" in error for error in model_validator.validate_model(model, binding)))

    def test_populated_source_projection_derives_exact_source_total(self) -> None:
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["collection"] = {
            "constituents": [
                {"id": "fixture", "model_sha256": "b" * 64, "clip_ids": ["fixture:idle-ip", "fixture:walk-rm"], "source_file_count": 1, "runtime_set_ids": ["fixture:paired"]}
            ],
            "exclusions": [], "cross_pack_records": [],
        }
        self.assertEqual(model_validator.validate_model(model, binding), [])
        model["collection"]["constituents"][0]["source_file_count"] = 0  # type: ignore[index]
        self.assertTrue(any("source_file_count" in error for error in model_validator.validate_model(model, binding)))

    def test_clip_witness_projection_is_typed_even_when_model_matches_it(self) -> None:
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["clips"][0]["take_index"] = -1  # type: ignore[index]
        binding["clips"][0]["take_index"] = -1  # type: ignore[index]
        self.assertTrue(model_validator.validate_model(model, binding))
        model, binding = valid_evaluation_model(), valid_collection_output_projection()
        model["clips"][0]["take_name"] = 7  # type: ignore[index]
        binding["clips"][0]["take_name"] = 7  # type: ignore[index]
        self.assertTrue(model_validator.validate_model(model, binding))

    def test_text_byte_limit_is_intentionally_stricter_than_schema_codepoints(self) -> None:
        schema_path = Path(__file__).parents[1] / "schemas" / "evaluation-model-v1.schema.json"
        validator = jsonschema.Draft202012Validator(json.loads(schema_path.read_text(encoding="utf-8")))
        model = valid_evaluation_model()
        model["presentation"]["title"] = "☃" * 11_000  # type: ignore[index]
        self.assertEqual(list(validator.iter_errors(model)), [])
        self.assertTrue(any("within" in error for error in model_validator.validate_model(model, valid_collection_output_projection())))

    def test_nonhashable_malformed_values_are_reported_not_raised(self) -> None:
        model = valid_evaluation_model()
        model["clips"][0]["id"] = ["not-an-id"]  # type: ignore[index]
        model["remediations"][0]["run_id"] = ["not-a-run"]  # type: ignore[index]
        model["collection"]["cross_pack_records"] = [{"id": "bad-pair", "left": ["not-an-id"], "right": "basic", "result": "unknown", "evidence_refs": []}]  # type: ignore[index]
        errors = model_validator.validate_model(model, valid_collection_output_projection())
        self.assertTrue(errors)

    def test_typed_text_narrative_locator_and_lane_mutations_fail_closed(self) -> None:
        model = valid_evaluation_model()
        model["evidence"][0]["locator"] = "file:///private/asset.fbx"  # type: ignore[index]
        model["clips"][0]["movement_owner"] = "garbage"  # type: ignore[index]
        model["readiness"][0]["adoption_consequence"] = 7  # type: ignore[index]
        model["integration_steps"][0]["coordinates_or_thresholds"] = 7  # type: ignore[index]
        model["issues"][0]["impact"] = 7  # type: ignore[index]
        model["engine_evidence"][0]["runtime"] = 7  # type: ignore[index]
        model["sources"][0]["license_scope"] = 7  # type: ignore[index]
        model["limitations"][0]["summary"] = 7  # type: ignore[index]
        model["narratives"][0]["text"] = "| state | count |\n|---|---:|\n| pass | 1 |"  # type: ignore[index]
        errors = model_validator.validate_model(model, valid_collection_output_projection())
        self.assertTrue(errors)
        self.assertTrue(all("schema:" in error for error in errors))

    def test_validator_cli_is_seed_deterministic_and_checks_canonical_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            model_path = directory / "model.json"
            binding_path = directory / "binding.json"
            model_path.write_bytes(model_contract.canonical_json(valid_evaluation_model()))
            binding_path.write_bytes(model_contract.canonical_json(valid_collection_output_projection()))
            commands = []
            for seed in ("1", "2"):
                environment = os.environ.copy()
                environment["PYTHONHASHSEED"] = seed
                commands.append(subprocess.run(
                    [str(Path(__file__).with_name("validate_evaluation_model.py")), str(model_path), "--binding", str(binding_path), "--check-canonical"],
                    check=False, text=True, capture_output=True, env=environment,
                ))
        self.assertEqual(commands[0].returncode, 0, commands[0].stderr)
        self.assertEqual(commands[1].returncode, 0, commands[1].stderr)
        self.assertEqual(commands[0].stdout, commands[1].stdout)
        self.assertEqual(commands[0].stderr, commands[1].stderr)
        expression = "import sys;sys.path.insert(0, sys.argv[1]);import evaluation_model_v1 as m;print(m.canonical_json({'z':1e-7,'a':-0.0,'u':'\\u2603'}).decode());print(m.canonical_digest({'z':1e-7,'a':-0.0,'u':'\\u2603'}))"
        generated = []
        for seed in ("1", "2"):
            environment = os.environ.copy(); environment["PYTHONHASHSEED"] = seed
            generated.append(subprocess.run([sys.executable, "-c", expression, str(Path(__file__).parent)], check=False, text=True, capture_output=True, env=environment))
        self.assertEqual(generated[0].returncode, 0, generated[0].stderr)
        self.assertEqual(generated[0].stdout, generated[1].stdout)


if __name__ == "__main__":
    unittest.main()
