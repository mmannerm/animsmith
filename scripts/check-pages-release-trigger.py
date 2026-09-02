#!/usr/bin/env python3
"""Validate how a published release reaches the Pages deployment workflow.

GitHub does not create workflow runs for events produced with the repository
GITHUB_TOKEN, so the `release: published` trigger in docs-pages.yml never fires
for a release published by release-plz (issue #652): the Pages root then keeps
serving the previous tag. `workflow_dispatch` is one of the two documented
exceptions to that recursion guard, so a successful publication must dispatch
the Pages workflow on main itself.

This contract therefore requires every part of that path to stay present — the
dispatch in the release workflow, the repository token that authorizes it, and
the `workflow_dispatch` trigger it targets — and keeps the deployment
fail-closed when no eligible published release exists.
"""

from __future__ import annotations

import argparse
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

from workflow_contract import (
    WorkflowContractError,
    load_workflow,
    normalized_text,
    require_mapping,
    workflow_triggers,
)


# The dispatch RELEASING.md documents as the manual recovery command. The job
# runs it with `--repo` because it checks nothing out and `gh` cannot infer the
# repository; from a clone that flag is unnecessary. Both forms are accepted,
# and nothing else may share the step: a `run` that only mentions the command
# (`echo '<command>'`, `false && <command>`) never dispatches anything.
DISPATCH_COMMAND = "gh workflow run docs-pages.yml --ref main"
DISPATCH_RUNS = (DISPATCH_COMMAND, f'{DISPATCH_COMMAND} --repo "${{GITHUB_REPOSITORY}}"')
# release-plz reports whether the run actually published anything. The gate is
# matched whole, so a negated or differently sourced condition cannot pass, and
# the job that produces it is required by name: a gate whose producer is not
# needed is never true, so the dispatch would simply never run.
PUBLICATION_JOB = "release"
PUBLICATION_OUTPUT = "releases_created"
PUBLICATION_GATE = f"needs.{PUBLICATION_JOB}.outputs.{PUBLICATION_OUTPUT} == 'true'"
# Only the repository token is allowed: the fix must not depend on a personal
# access token that a maintainer has to mint and rotate.
REPOSITORY_TOKENS = ("${{ github.token }}", "${{ secrets.GITHUB_TOKEN }}")
# The publication itself. The gate is only as good as the value behind it, so
# the output must forward this action's own report, not a literal or another
# step's result.
RELEASE_ACTION = "release-plz/action@"
DEPLOY_ACTION = "actions/deploy-pages@"
# Pages deploys only when an eligible published release was found. Matched
# whole as well, so `... || true` cannot widen it.
DEPLOY_CONDITION = (
    "github.event_name != 'pull_request' "
    "&& needs.prepare-release-site.outputs.release_available == 'true'"
)


def condition(value: object) -> str:
    """Return an `if:` expression normalized for whole-value comparison.

    GitHub accepts the same condition with or without the `${{ }}` wrapper, so
    the wrapper is stripped; everything else must match the contract exactly.
    """
    text = normalized_text(value)
    if text.startswith("${{") and text.endswith("}}"):
        text = " ".join(text[3:-2].split())
    return text


def jobs_of(document: dict[str, Any], source: str) -> dict[str, Any]:
    return require_mapping(document.get("jobs"), f"{source}: jobs")


def steps_of(job: dict[str, Any], description: str) -> list[dict[str, Any]]:
    """Return a job's steps; a reusable-workflow call legitimately has none."""
    steps = job.get("steps", [])
    if not isinstance(steps, list):
        raise WorkflowContractError(f"{description} steps must be a list")
    return [require_mapping(step, f"{description} step") for step in steps]


def check_release_workflow_text(text: str, source: str) -> None:
    """The release workflow must dispatch Pages after a real publication."""
    document = load_workflow(text, source)
    jobs = jobs_of(document, source)
    dispatchers = [
        (job_id, job, step)
        for job_id, job in jobs.items()
        for step in steps_of(
            require_mapping(job, f"{source}: {job_id} job"), f"{source}: {job_id} job"
        )
        if normalized_text(step.get("run", "")) in DISPATCH_RUNS
    ]
    if len(dispatchers) != 1:
        raise WorkflowContractError(
            f"{source}: exactly one step must run {DISPATCH_COMMAND!r} as its whole "
            "command; a release published with the repository GITHUB_TOKEN never "
            "triggers the `release: published` event, and "
            f"{len(dispatchers)} steps do"
        )
    job_id, job, step = dispatchers[0]

    permissions = require_mapping(
        job.get("permissions"), f"{source}: {job_id} permissions"
    )
    if permissions != {"actions": "write"}:
        raise WorkflowContractError(
            f"{source}: the {job_id} job must hold `actions: write` and nothing "
            f"else, not {permissions}"
        )

    conditions = {
        gate
        for gate in (condition(job.get("if", "")), condition(step.get("if", "")))
        if gate
    }
    if conditions != {PUBLICATION_GATE}:
        raise WorkflowContractError(
            f"{source}: the {job_id} dispatch must be gated on exactly "
            f"{PUBLICATION_GATE!r}, not {sorted(conditions)}"
        )

    # A gate is only a post-publication trigger while the job it reads is
    # actually waited for and actually publishes that output.
    needs = job.get("needs")
    if needs == PUBLICATION_JOB:
        needs = [PUBLICATION_JOB]
    if needs != [PUBLICATION_JOB]:
        raise WorkflowContractError(
            f"{source}: the {job_id} job must declare needs: [{PUBLICATION_JOB}], so "
            f"it runs after publication and {PUBLICATION_GATE} has a producer, not "
            f"{job.get('needs')!r}"
        )
    if PUBLICATION_JOB not in jobs:
        raise WorkflowContractError(
            f"{source}: jobs must contain the {PUBLICATION_JOB} job the dispatch "
            "waits for"
        )
    producer = require_mapping(
        jobs[PUBLICATION_JOB], f"{source}: {PUBLICATION_JOB} job"
    )
    outputs = producer.get("outputs")
    if not isinstance(outputs, dict) or PUBLICATION_OUTPUT not in outputs:
        raise WorkflowContractError(
            f"{source}: the {PUBLICATION_JOB} job must publish the "
            f"{PUBLICATION_OUTPUT} output the dispatch is gated on"
        )
    forwarded = {
        f"${{{{ steps.{step['id']}.outputs.{PUBLICATION_OUTPUT} }}}}"
        for step in steps_of(producer, f"{source}: {PUBLICATION_JOB} job")
        if isinstance(step.get("uses"), str)
        and step["uses"].startswith(RELEASE_ACTION)
        and isinstance(step.get("id"), str)
    }
    if not forwarded:
        raise WorkflowContractError(
            f"{source}: the {PUBLICATION_JOB} job must run an identified "
            f"{RELEASE_ACTION}* step for its {PUBLICATION_OUTPUT} output to report"
        )
    published = normalized_text(outputs[PUBLICATION_OUTPUT])
    if published not in forwarded:
        raise WorkflowContractError(
            f"{source}: the {PUBLICATION_JOB} job's {PUBLICATION_OUTPUT} output must "
            f"forward the release-plz step, one of {sorted(forwarded)}, not "
            f"{published!r}"
        )

    token = None
    for scope in (step, job, document):
        environment = scope.get("env")
        if isinstance(environment, dict) and "GH_TOKEN" in environment:
            token = normalized_text(environment["GH_TOKEN"])
            break
    if token is None:
        raise WorkflowContractError(
            f"{source}: the {job_id} dispatch must set GH_TOKEN for `gh`"
        )
    if token not in REPOSITORY_TOKENS:
        raise WorkflowContractError(
            f"{source}: the {job_id} dispatch must authenticate with the "
            f"repository token, not {token}"
        )


def check_pages_workflow_text(text: str, source: str) -> None:
    """The Pages workflow must be dispatchable and deploy fail-closed."""
    document = load_workflow(text, source)
    triggers = workflow_triggers(document, source)
    if "workflow_dispatch" not in triggers:
        raise WorkflowContractError(
            f"{source}: workflow_dispatch is the only post-publication trigger "
            "GitHub delivers for a GITHUB_TOKEN-created release, so it must "
            "stay declared"
        )

    deployers = [
        (job_id, job)
        for job_id, job in jobs_of(document, source).items()
        for step in steps_of(
            require_mapping(job, f"{source}: {job_id} job"), f"{source}: {job_id} job"
        )
        if isinstance(step.get("uses"), str) and step["uses"].startswith(DEPLOY_ACTION)
    ]
    if len(deployers) != 1:
        raise WorkflowContractError(
            f"{source}: exactly one job must use {DEPLOY_ACTION}*; found "
            f"{len(deployers)}"
        )
    job_id, job = deployers[0]
    deploy_gate = condition(job.get("if", ""))
    if deploy_gate != DEPLOY_CONDITION:
        raise WorkflowContractError(
            f"{source}: the {job_id} job must stay gated on exactly "
            f"{DEPLOY_CONDITION!r} so no eligible published release means no "
            f"deployment, not {deploy_gate!r}"
        )


VALID_RELEASE_WORKFLOW = """\
jobs:
  release:
    outputs:
      releases_created: ${{ steps.release_plz.outputs.releases_created }}
    steps:
      - name: Run release-plz
        id: release_plz
        uses: release-plz/action@v0.5.131
        with:
          command: release
  release_pages:
    needs: [release]
    if: ${{ needs.release.outputs.releases_created == 'true' }}
    permissions:
      actions: write
    steps:
      - name: Dispatch the Pages documentation workflow on main
        env:
          GH_TOKEN: ${{ github.token }}
        run: gh workflow run docs-pages.yml --ref main --repo "${GITHUB_REPOSITORY}"
"""

VALID_PAGES_WORKFLOW = """\
on:
  pull_request:
  push:
    branches: [main]
  release:
    types: [published]
  workflow_dispatch:
jobs:
  prepare-release-site:
    steps:
      - run: scripts/check-pages-release-eligibility.sh "$tag"
  deploy:
    needs: prepare-release-site
    if: >-
      github.event_name != 'pull_request' &&
      needs.prepare-release-site.outputs.release_available == 'true'
    steps:
      - uses: actions/deploy-pages@v4
"""

DISPATCH_STEP = """\
      - name: Dispatch the Pages documentation workflow on main
        env:
          GH_TOKEN: ${{ github.token }}
        run: gh workflow run docs-pages.yml --ref main --repo "${GITHUB_REPOSITORY}"
"""

NEEDS_LINE = "    needs: [release]\n"
RELEASE_OUTPUT_LINE = (
    "      releases_created: ${{ steps.release_plz.outputs.releases_created }}\n"
)
RELEASE_ACTION_LINE = "        uses: release-plz/action@v0.5.131\n"
RELEASE_OUTPUTS_BLOCK = (
    "    outputs:\n"
    "      releases_created: ${{ steps.release_plz.outputs.releases_created }}\n"
)
PUBLICATION_GATE_LINE = (
    "    if: ${{ needs.release.outputs.releases_created == 'true' }}\n"
)
PERMISSIONS_BLOCK = "    permissions:\n      actions: write\n"
DISPATCH_RUN_LINE = (
    "        run: gh workflow run docs-pages.yml --ref main"
    ' --repo "${GITHUB_REPOSITORY}"\n'
)
DEPLOY_GATE_LINE = (
    "      needs.prepare-release-site.outputs.release_available == 'true'\n"
)


def expect_rejected(
    label: str, check: Callable[[str, str], None], text: str
) -> None:
    try:
        check(text, label)
    except WorkflowContractError:
        return
    raise AssertionError(f"{label}: invalid fixture was accepted")


def released(label: str, original: str, replacement: str) -> None:
    """Reject one mutation of the valid release-workflow fixture."""
    assert original in VALID_RELEASE_WORKFLOW, f"{label}: stale mutation target"
    expect_rejected(
        label,
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW.replace(original, replacement, 1),
    )


def paged(label: str, original: str, replacement: str) -> None:
    """Reject one mutation of the valid Pages-workflow fixture."""
    assert original in VALID_PAGES_WORKFLOW, f"{label}: stale mutation target"
    expect_rejected(
        label,
        check_pages_workflow_text,
        VALID_PAGES_WORKFLOW.replace(original, replacement, 1),
    )


def self_test() -> None:
    check_release_workflow_text(VALID_RELEASE_WORKFLOW, "valid release fixture")
    check_pages_workflow_text(VALID_PAGES_WORKFLOW, "valid pages fixture")
    # The `--repo` flag is the job's, not the documented recovery command's.
    check_release_workflow_text(
        VALID_RELEASE_WORKFLOW.replace(' --repo "${GITHUB_REPOSITORY}"', "", 1),
        "bare-dispatch fixture",
    )
    # GitHub accepts a lone dependency as a scalar; so does the contract.
    check_release_workflow_text(
        VALID_RELEASE_WORKFLOW.replace("needs: [release]", "needs: release", 1),
        "scalar-needs fixture",
    )

    # The regression itself: publication leaves Pages to the suppressed
    # `release: published` event.
    released("release-event-only fixture", DISPATCH_STEP, "      - run: true\n")
    released(
        "wrong-ref fixture",
        "--ref main",
        "--ref ${{ github.event.release.tag_name }}",
    )
    released(
        "commented-dispatch fixture",
        "        run: gh workflow run",
        "        # run: gh workflow run",
    )
    # A step that only mentions the command dispatches nothing.
    released(
        "echoed-dispatch fixture",
        DISPATCH_RUN_LINE,
        "        run: echo 'gh workflow run docs-pages.yml --ref main'\n",
    )
    released(
        "short-circuit-dispatch fixture",
        "        run: gh workflow run",
        "        run: false && gh workflow run",
    )
    released(
        "trailing-command fixture",
        DISPATCH_RUN_LINE,
        DISPATCH_RUN_LINE.rstrip("\n") + " && git push --force\n",
    )
    expect_rejected(
        "duplicated-dispatch fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW
        + DISPATCH_STEP.replace("      - ", "  extra:\n    steps:\n      - ", 1),
    )
    released(
        "missing-actions-permission fixture", PERMISSIONS_BLOCK, "    permissions: {}\n"
    )
    released(
        "string-permissions fixture", PERMISSIONS_BLOCK, "    permissions: write-all\n"
    )
    # `actions: write` is the whole grant, not a floor.
    released(
        "extra-permission fixture",
        PERMISSIONS_BLOCK,
        PERMISSIONS_BLOCK + "      contents: write\n",
    )
    released(
        "duplicate-permissions fixture",
        PERMISSIONS_BLOCK,
        PERMISSIONS_BLOCK + "    permissions:\n      contents: read\n",
    )
    released("ungated-dispatch fixture", PUBLICATION_GATE_LINE, "")
    # Without the scheduling edge the gate has no producer and is never true.
    released("unscheduled-dispatch fixture", NEEDS_LINE, "")
    released("misdirected-needs fixture", NEEDS_LINE, "    needs: [checks]\n")
    released(
        "widened-needs fixture", NEEDS_LINE, "    needs: [release, release_binaries]\n"
    )
    released("unproduced-gate fixture", RELEASE_OUTPUTS_BLOCK, "")
    # An output that does not report the publication is not evidence of one.
    released(
        "hardcoded-output fixture",
        RELEASE_OUTPUT_LINE,
        "      releases_created: true\n",
    )
    released(
        "miswired-output fixture",
        RELEASE_OUTPUT_LINE,
        "      releases_created: ${{ steps.release_metadata.outputs"
        ".releases_created }}\n",
    )
    released(
        "non-action-step fixture",
        RELEASE_ACTION_LINE,
        "        run: release-plz release\n",
    )
    released("unidentified-action fixture", "        id: release_plz\n", "")
    # A gate that passes when nothing was published is not a release trigger.
    released(
        "negated-gate fixture",
        "releases_created == 'true'",
        "releases_created != 'true'",
    )
    released(
        "dispatch-input-gate fixture",
        "needs.release.outputs.releases_created",
        "github.event.inputs.releases_created",
    )
    released(
        "widened-gate fixture",
        PUBLICATION_GATE_LINE,
        PUBLICATION_GATE_LINE.rstrip("\n").rstrip("}") + " || true }}\n",
    )
    released(
        "disabled-step fixture",
        "      - name: Dispatch",
        "      - if: false\n        name: Dispatch",
    )
    released(
        "missing-token fixture",
        "        env:\n          GH_TOKEN: ${{ github.token }}\n",
        "",
    )
    released(
        "personal-token fixture",
        "GH_TOKEN: ${{ github.token }}",
        "GH_TOKEN: ${{ secrets.PAGES_DISPATCH_TOKEN }}",
    )

    paged("undispatchable-pages fixture", "  workflow_dispatch:\n", "")
    expect_rejected(
        "quoted-on fixture",
        check_pages_workflow_text,
        VALID_PAGES_WORKFLOW.replace("on:", '"on":', 1).replace(
            "  workflow_dispatch:\n", "", 1
        ),
    )
    paged("open-deployment fixture", DEPLOY_GATE_LINE, "      true\n")
    # An `|| true` disjunct deploys a site with no eligible release.
    paged(
        "weakened-deploy-gate fixture",
        DEPLOY_GATE_LINE,
        DEPLOY_GATE_LINE.rstrip("\n") + " || true\n",
    )
    paged(
        "preview-deployment fixture",
        "      github.event_name != 'pull_request' &&\n",
        "",
    )
    expect_rejected(
        "triggerless fixture",
        check_pages_workflow_text,
        VALID_PAGES_WORKFLOW[VALID_PAGES_WORKFLOW.index("jobs:") :],
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--release-workflow", type=Path)
    parser.add_argument("--pages-workflow", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    for workflow, check in (
        (args.release_workflow, check_release_workflow_text),
        (args.pages_workflow, check_pages_workflow_text),
    ):
        if workflow is not None:
            check(workflow.read_text(encoding="utf-8"), str(workflow))
    if not (args.self_test or args.release_workflow or args.pages_workflow):
        parser.error(
            "one of --release-workflow, --pages-workflow or --self-test is required"
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, WorkflowContractError, AssertionError) as exc:
        print(f"pages-release-trigger: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
