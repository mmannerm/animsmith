#!/usr/bin/env python3
"""Feature-variant builds must never write to the conventional target paths.

`just gates` and the reusable checks workflow build the CLI with default
features and again with `--no-default-features`. While both wrote to
`target/release/animsmith`, the artifact a gate run left behind was whichever
variant ran last -- a binary that rejects FBX while `--version` looked
identical. Every `--no-default-features` cargo command therefore redirects
`CARGO_TARGET_DIR` to an isolated directory, and this gate holds both files to
that.

The workflow half is asserted after YAML decoding rather than over raw text: a
step selects the directory through its `env` mapping, and an inline
`CARGO_TARGET_DIR=... cargo ...` prefix would be a bash-ism that the Windows
runner's default shell cannot parse.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from workflow_contract import (
    WorkflowContractError,
    load_workflow,
    normalized_text,
    require_mapping,
)

ISOLATED_TARGET = "target/no-default-features"
JUSTFILE_VARIABLE = f'no_default_target := "{ISOLATED_TARGET}"'
JUSTFILE_ISOLATION = 'CARGO_TARGET_DIR="{{no_default_target}}"'
TARGET_DIR_VARIABLE = "CARGO_TARGET_DIR"
MINIMAL_RELEASE_BUILD = "cargo build -p animsmith --release --no-default-features"
RETAINED_CLI_CHECK = "bash scripts/check-release-cli.sh"


def is_feature_variant(command: str) -> bool:
    """Whether `command` is a cargo invocation selecting a non-default feature set."""
    return "cargo " in command and "--no-default-features" in command


def check_justfile_text(text: str, source: str) -> None:
    lines = text.splitlines()
    if JUSTFILE_VARIABLE not in [line.strip() for line in lines]:
        raise WorkflowContractError(f"{source}: must define {JUSTFILE_VARIABLE}")

    variant_lines = [line for line in lines if is_feature_variant(line)]
    unisolated = [line.strip() for line in variant_lines if JUSTFILE_ISOLATION not in line]
    if unisolated:
        commands = "; ".join(unisolated)
        raise WorkflowContractError(
            f"{source}: these feature-variant commands can overwrite the "
            f"default-feature artifacts because they omit {JUSTFILE_ISOLATION}: {commands}"
        )

    if not any(
        MINIMAL_RELEASE_BUILD in line and JUSTFILE_ISOLATION in line
        for line in variant_lines
    ):
        raise WorkflowContractError(
            f"{source}: must run `{MINIMAL_RELEASE_BUILD}` into {ISOLATED_TARGET}, "
            "which is the artifact the retained-CLI check probes against"
        )


def check_workflow_text(text: str, source: str) -> None:
    document = load_workflow(text, source)
    jobs = require_mapping(document.get("jobs"), f"{source}: jobs")

    isolated_release_builds = 0
    for name, raw_job in jobs.items():
        job = require_mapping(raw_job, f"{source}: {name} job")
        steps = job.get("steps")
        if not isinstance(steps, list):
            continue
        minimal_release_index: int | None = None
        check_index: int | None = None
        for index, raw_step in enumerate(steps):
            step = require_mapping(raw_step, f"{source}: {name} step")
            command = normalized_text(step.get("run", ""))
            if command == RETAINED_CLI_CHECK and check_index is None:
                check_index = index
            if not is_feature_variant(command):
                continue
            if f"{TARGET_DIR_VARIABLE}=" in command:
                raise WorkflowContractError(
                    f"{source}: {name} step {index} sets {TARGET_DIR_VARIABLE} inside its "
                    "run command; the Windows runner's default shell cannot parse an "
                    "inline environment prefix, so use the step's env mapping"
                )
            selected = require_mapping(
                step.get("env", {}), f"{source}: {name} step {index} env"
            ).get(TARGET_DIR_VARIABLE)
            if selected != ISOLATED_TARGET:
                raise WorkflowContractError(
                    f"{source}: {name} step {index} runs `{command}` with "
                    f"{TARGET_DIR_VARIABLE}={selected!r}; it must be "
                    f"{ISOLATED_TARGET!r} or it can overwrite the default-feature artifacts"
                )
            if MINIMAL_RELEASE_BUILD in command:
                isolated_release_builds += 1
                minimal_release_index = index
        if check_index is not None:
            if minimal_release_index is None:
                raise WorkflowContractError(
                    f"{source}: {name} runs `{RETAINED_CLI_CHECK}` without building "
                    f"`{MINIMAL_RELEASE_BUILD}` in the same job"
                )
            if minimal_release_index >= check_index:
                raise WorkflowContractError(
                    f"{source}: {name} must build the minimal release CLI before "
                    f"running `{RETAINED_CLI_CHECK}`"
                )

    if isolated_release_builds == 0:
        raise WorkflowContractError(
            f"{source}: must run `{MINIMAL_RELEASE_BUILD}` into {ISOLATED_TARGET}, "
            "which is the artifact the retained-CLI check probes against"
        )
    if not any(
        normalized_text(step.get("run", "")) == RETAINED_CLI_CHECK
        for raw_job in jobs.values()
        if isinstance(raw_job, dict) and isinstance(raw_job.get("steps"), list)
        for step in raw_job["steps"]
        if isinstance(step, dict)
    ):
        raise WorkflowContractError(
            f"{source}: must run `{RETAINED_CLI_CHECK}` so CI proves the retained "
            "release CLI the same way the local gate does"
        )


VALID_JUSTFILE = f"""\
{JUSTFILE_VARIABLE}

doc:
    RUSTDOCFLAGS="-D warnings" {JUSTFILE_ISOLATION} cargo doc -p animsmith --no-default-features --no-deps

release-cli:
    cargo build -p animsmith --release
    {JUSTFILE_ISOLATION} {MINIMAL_RELEASE_BUILD}
    {RETAINED_CLI_CHECK}
"""

VALID_WORKFLOW = f"""\
jobs:
  test:
    steps:
      - name: Build release binary
        run: cargo build -p animsmith --release
      - name: Build release binary without default features
        env:
          {TARGET_DIR_VARIABLE}: {ISOLATED_TARGET}
        run: {MINIMAL_RELEASE_BUILD}
      - name: Prove the retained release CLI is the default-feature build
        run: {RETAINED_CLI_CHECK}
"""


def expect_rejected(label: str, check, text: str) -> None:
    try:
        check(text, label)
    except WorkflowContractError:
        return
    raise AssertionError(f"{label}: an invalid fixture was accepted")


def self_test() -> None:
    check_justfile_text(VALID_JUSTFILE, "valid justfile fixture")
    check_workflow_text(VALID_WORKFLOW, "valid workflow fixture")

    expect_rejected(
        "justfile without the isolated target variable",
        check_justfile_text,
        VALID_JUSTFILE.replace(JUSTFILE_VARIABLE, 'no_default_target := "target"'),
    )
    expect_rejected(
        "justfile whose rustdoc pass shares the default target directory",
        check_justfile_text,
        VALID_JUSTFILE.replace(f'{JUSTFILE_ISOLATION} cargo doc', "cargo doc"),
    )
    expect_rejected(
        "justfile that stopped building the minimal release CLI",
        check_justfile_text,
        VALID_JUSTFILE.replace(f"    {JUSTFILE_ISOLATION} {MINIMAL_RELEASE_BUILD}\n", ""),
    )

    expect_rejected(
        "workflow step with no env mapping",
        check_workflow_text,
        VALID_WORKFLOW.replace(
            f"        env:\n          {TARGET_DIR_VARIABLE}: {ISOLATED_TARGET}\n", ""
        ),
    )
    expect_rejected(
        "workflow step naming another target directory",
        check_workflow_text,
        VALID_WORKFLOW.replace(
            f"{TARGET_DIR_VARIABLE}: {ISOLATED_TARGET}", f"{TARGET_DIR_VARIABLE}: target"
        ),
    )
    expect_rejected(
        "workflow step with a shell-specific inline environment prefix",
        check_workflow_text,
        VALID_WORKFLOW.replace(
            f"        env:\n          {TARGET_DIR_VARIABLE}: {ISOLATED_TARGET}\n"
            f"        run: {MINIMAL_RELEASE_BUILD}",
            f"        run: {TARGET_DIR_VARIABLE}={ISOLATED_TARGET} {MINIMAL_RELEASE_BUILD}",
        ),
    )
    expect_rejected(
        "workflow that stopped building the minimal release CLI",
        check_workflow_text,
        VALID_WORKFLOW.replace(
            f"      - name: Build release binary without default features\n"
            f"        env:\n          {TARGET_DIR_VARIABLE}: {ISOLATED_TARGET}\n"
            f"        run: {MINIMAL_RELEASE_BUILD}\n",
            "",
        ),
    )
    expect_rejected(
        "workflow that probes the retained CLI before building the minimal one",
        check_workflow_text,
        f"""\
jobs:
  test:
    steps:
      - name: Prove the retained release CLI is the default-feature build
        run: {RETAINED_CLI_CHECK}
      - name: Build release binary without default features
        env:
          {TARGET_DIR_VARIABLE}: {ISOLATED_TARGET}
        run: {MINIMAL_RELEASE_BUILD}
""",
    )
    expect_rejected(
        "workflow that stopped proving the retained CLI",
        check_workflow_text,
        VALID_WORKFLOW.replace(
            f"      - name: Prove the retained release CLI is the default-feature build\n"
            f"        run: {RETAINED_CLI_CHECK}\n",
            "",
        ),
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--justfile", type=Path)
    parser.add_argument("--workflow", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    if args.justfile is not None:
        check_justfile_text(
            args.justfile.read_text(encoding="utf-8"), str(args.justfile)
        )
    if args.workflow is not None:
        check_workflow_text(
            args.workflow.read_text(encoding="utf-8"), str(args.workflow)
        )
    if not args.self_test and args.justfile is None and args.workflow is None:
        parser.error("one of --justfile, --workflow or --self-test is required")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, WorkflowContractError, AssertionError) as exc:
        print(f"feature-isolation: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
