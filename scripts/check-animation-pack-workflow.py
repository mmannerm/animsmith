#!/usr/bin/env python3
"""Validate the structural contract of the animation-pack CI job."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from workflow_contract import (
    WorkflowContractError,
    load_workflow,
    require_mapping,
)


CHECKOUT = "actions/checkout@v7"
HEAD_REF = "${{ github.event.pull_request.head.sha || github.sha }}"
VALIDATOR_NAME = "Validate animation-pack skill and published reports"
BUILD_NAME = "Build checkout-matched AnimSmith validator"
BUILD_RUN = "cargo build -p animsmith --bin animsmith"
VALIDATOR_RUN = (
    "ANIMSMITH_TEST_BINARY=target/debug/animsmith PYTHONDONTWRITEBYTECODE=1 python "
    ".agents/skills/evaluate-animation-packs/scripts/test_validators.py"
)
JOB_CONTROLS = {"if", "continue-on-error", "strategy", "needs", "defaults"}
CHECKOUT_KEYS = {"uses", "with"}
VALIDATOR_KEYS = {"name", "run"}


def check_workflow_text(text: str, source: str) -> None:
    document = load_workflow(text, source)
    if "defaults" in document:
        raise WorkflowContractError(f"{source}: workflow must not define defaults")
    jobs = require_mapping(document.get("jobs"), f"{source}: jobs")
    if "animation-pack" not in jobs:
        raise WorkflowContractError(f"{source}: jobs must contain animation-pack")
    job = require_mapping(jobs["animation-pack"], f"{source}: animation-pack job")

    forbidden_job_controls = sorted(JOB_CONTROLS.intersection(job))
    if forbidden_job_controls:
        controls = ", ".join(forbidden_job_controls)
        raise WorkflowContractError(
            f"{source}: animation-pack job must not define {controls}"
        )

    steps = job.get("steps")
    if not isinstance(steps, list):
        raise WorkflowContractError(f"{source}: animation-pack steps must be a list")
    step_mappings = [
        require_mapping(step, f"{source}: animation-pack step") for step in steps
    ]

    checkout_steps = [
        step
        for step in step_mappings
        if isinstance(step.get("uses"), str)
        and step["uses"].startswith("actions/checkout@")
    ]
    if len(checkout_steps) != 1:
        raise WorkflowContractError(
            f"{source}: animation-pack must contain exactly one actions/checkout step"
        )
    checkout = checkout_steps[0]
    if checkout.get("uses") != CHECKOUT:
        raise WorkflowContractError(
            f"{source}: the checkout step must use {CHECKOUT}"
        )
    checkout_extras = sorted(set(checkout) - CHECKOUT_KEYS)
    if checkout_extras:
        raise WorkflowContractError(
            f"{source}: checkout step must not define {', '.join(checkout_extras)}"
        )
    checkout_with = require_mapping(
        checkout.get("with"), f"{source}: checkout with"
    )
    if checkout_with.get("ref") != HEAD_REF:
        raise WorkflowContractError(
            f"{source}: checkout with.ref must be {HEAD_REF!r}"
        )

    validator_steps = [step for step in step_mappings if step.get("name") == VALIDATOR_NAME]
    if len(validator_steps) != 1:
        raise WorkflowContractError(
            f"{source}: animation-pack must contain exactly one named validator step"
        )
    validator = validator_steps[0]
    validator_extras = sorted(set(validator) - VALIDATOR_KEYS)
    if validator_extras:
        raise WorkflowContractError(
            f"{source}: validator step must not define {', '.join(validator_extras)}"
        )
    if validator.get("run") != VALIDATOR_RUN:
        raise WorkflowContractError(
            f"{source}: validator step run must be {VALIDATOR_RUN!r}"
        )

    build_steps = [step for step in step_mappings if step.get("name") == BUILD_NAME]
    if len(build_steps) != 1:
        raise WorkflowContractError(
            f"{source}: animation-pack must contain exactly one checkout-binary build step"
        )
    build = build_steps[0]
    build_extras = sorted(set(build) - VALIDATOR_KEYS)
    if build_extras:
        raise WorkflowContractError(
            f"{source}: checkout-binary build step must not define {', '.join(build_extras)}"
        )
    if build.get("run") != BUILD_RUN:
        raise WorkflowContractError(
            f"{source}: checkout-binary build step run must be {BUILD_RUN!r}"
        )
    if step_mappings.index(build) >= step_mappings.index(validator):
        raise WorkflowContractError(
            f"{source}: checkout-binary build step must precede the validator"
        )


VALID_WORKFLOW = f"""\
jobs:
  animation-pack:
    steps:
      - uses: {CHECKOUT}
        with:
          ref: {HEAD_REF}
      - name: {BUILD_NAME}
        run: {BUILD_RUN}
      - name: {VALIDATOR_NAME}
        run: {VALIDATOR_RUN}
"""


def expect_rejected(label: str, text: str) -> None:
    try:
        check_workflow_text(text, label)
    except WorkflowContractError:
        return
    raise AssertionError(f"{label}: invalid fixture was accepted")


def self_test() -> None:
    check_workflow_text(VALID_WORKFLOW, "valid fixture")
    expect_rejected(
        "missing-build fixture",
        VALID_WORKFLOW.replace(
            f"      - name: {BUILD_NAME}\n        run: {BUILD_RUN}\n", ""
        ),
    )
    expect_rejected(
        "late-build fixture",
        VALID_WORKFLOW.replace(
            f"      - name: {BUILD_NAME}\n        run: {BUILD_RUN}\n", ""
        )
        + f"      - name: {BUILD_NAME}\n        run: {BUILD_RUN}\n",
    )
    expect_rejected(
        "commented fixture",
        VALID_WORKFLOW.replace(
            f"          ref: {HEAD_REF}", f"          # ref: {HEAD_REF}"
        ).replace(f"        run: {VALIDATOR_RUN}", f"        # run: {VALIDATOR_RUN}"),
    )
    expect_rejected(
        "wrong-job fixture",
        VALID_WORKFLOW.replace(
            "  animation-pack:", "  animation-pack:\n    steps: []\n  other:"
        ),
    )
    expect_rejected(
        "nested-env fixture",
        VALID_WORKFLOW.replace(
            f"          ref: {HEAD_REF}", f"        env:\n          ref: {HEAD_REF}"
        ).replace(
            f"        run: {VALIDATOR_RUN}", f"        env:\n          run: {VALIDATOR_RUN}"
        ),
    )
    expect_rejected(
        "quoted-defaults fixture",
        VALID_WORKFLOW.replace(
            "    steps:", '    "defaults":\n      run:\n        shell: cat {0}\n    steps:'
        ),
    )
    expect_rejected(
        "workflow-defaults fixture",
        "defaults:\n  run:\n    shell: cat {0}\n" + VALID_WORKFLOW,
    )
    expect_rejected(
        "merge fixture",
        """\
base: &job-controls
  defaults:
    run:
      shell: cat {0}
jobs:
  animation-pack:
    <<: *job-controls
    steps:
      - uses: actions/checkout@v7
        with:
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
      - name: Validate animation-pack skill and published reports
        run: PYTHONDONTWRITEBYTECODE=1 python .agents/skills/evaluate-animation-packs/scripts/test_validators.py
""",
    )
    expect_rejected(
        "duplicate-key fixture",
        VALID_WORKFLOW.replace(
            "  animation-pack:", "  animation-pack:\n    steps: []\n  animation-pack:"
        ),
    )
    expect_rejected(
        "job-if fixture", VALID_WORKFLOW.replace("    steps:", "    if: ${{ false }}\n    steps:")
    )
    expect_rejected(
        "validator-continue-on-error fixture",
        VALID_WORKFLOW.replace(
            f"        run: {VALIDATOR_RUN}",
            f"        continue-on-error: true\n        run: {VALIDATOR_RUN}",
        ),
    )
    expect_rejected(
        "defaults-shell fixture",
        VALID_WORKFLOW.replace(
            "    steps:",
            "    defaults:\n      run:\n        shell: cat {0}\n    steps:",
        ),
    )
    expect_rejected(
        "second-checkout fixture",
        VALID_WORKFLOW.replace(
            f"      - name: {VALIDATOR_NAME}",
            "      - uses: actions/checkout@v6\n"
            "        with:\n"
            "          ref: main\n"
            f"      - name: {VALIDATOR_NAME}",
        ),
    )
    expect_rejected(
        "checkout-env fixture",
        VALID_WORKFLOW.replace(
            f"      - uses: {CHECKOUT}",
            f"      - uses: {CHECKOUT}\n        env:\n          CI: true",
        ),
    )
    expect_rejected(
        "validator-env fixture",
        VALID_WORKFLOW.replace(
            f"        run: {VALIDATOR_RUN}",
            f"        env:\n          CI: true\n        run: {VALIDATOR_RUN}",
        ),
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workflow", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    if args.workflow is not None:
        check_workflow_text(args.workflow.read_text(encoding="utf-8"), str(args.workflow))
    if not args.self_test and args.workflow is None:
        parser.error("one of --workflow or --self-test is required")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, WorkflowContractError, AssertionError) as exc:
        print(f"animation-pack-workflow: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
