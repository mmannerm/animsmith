#!/usr/bin/env python3
"""Behavioral tests for the animation-pack evaluation helper scripts."""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
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

    def test_templates_preserve_reviewed_provenance_boundary(self) -> None:
        skill = self.read("SKILL.md")
        report_template = self.read("assets/report-template.md")
        appendix_template = self.read("assets/evidence-appendix-template.md")

        self.assertIn("collection-level or constituent-level", skill)
        self.assertIn("collection version does not identify", skill)
        self.assertIn("distinguish collection listing", report_template)
        self.assertIn("collection-level and\nconstituent-level", appendix_template)

    def test_templates_preserve_primary_evidence_without_duplication(self) -> None:
        skill = self.read("SKILL.md")
        report_template = self.read("assets/report-template.md")
        appendix_template = self.read("assets/evidence-appendix-template.md")

        self.assertIn("appendix must link directly to that evidence", skill)
        self.assertIn("Treat this table as decision evidence", report_template)
        self.assertIn("link to that table here", appendix_template)
        self.assertIn("without duplicating", appendix_template)

    def test_exact_source_typos_stay_narrow_and_root_motion_stays_safe(self) -> None:
        skill = self.read("SKILL.md")
        taxonomy = self.read("references/clip-taxonomy.md")
        appendix_template = self.read("assets/evidence-appendix-template.md")

        self.assertIn("complete\nidentifier", skill)
        self.assertIn("exact-identifier exception", taxonomy)
        self.assertIn("allowlist the misspelled substring globally", taxonomy)
        self.assertIn("root translation or yaw", appendix_template)
        self.assertIn("displacement and yaw proof", appendix_template)

    def test_version_refresh_keeps_refusal_distinct_from_remediation(self) -> None:
        skill = self.read("SKILL.md")
        report_template = self.read("assets/report-template.md")
        appendix_template = self.read("assets/evidence-appendix-template.md")
        engine_reference = self.read("references/engine-and-compatibility.md")

        self.assertIn("Treat earlier generated outputs as historical", skill)
        self.assertIn("it is not a successful remediation", skill)
        self.assertIn(
            "A current safety refusal is not\na successful remediation",
            report_template,
        )
        self.assertIn(
            "whether the current version accepted or refused", appendix_template
        )
        self.assertIn(
            "whether the operation\nproduced output or refused", engine_reference
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


if __name__ == "__main__":
    unittest.main()
