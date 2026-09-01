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


# The exact command RELEASING.md documents as the manual recovery dispatch, so
# the automatic and manual paths cannot drift apart.
DISPATCH_COMMAND = "gh workflow run docs-pages.yml --ref main"
# release-plz reports whether the run actually published anything; an
# ungated dispatch would not prove a post-publication trigger.
PUBLICATION_OUTPUT = "releases_created"
# Only the repository token is allowed: the fix must not depend on a personal
# access token that a maintainer has to mint and rotate.
REPOSITORY_TOKENS = ("${{ github.token }}", "${{ secrets.GITHUB_TOKEN }}")
DEPLOY_ACTION = "actions/deploy-pages@"
# Pages deploys only when an eligible published release was found.
DEPLOY_GATE = "release_available == 'true'"


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
    dispatchers = [
        (job_id, job, step)
        for job_id, job in jobs_of(document, source).items()
        for step in steps_of(
            require_mapping(job, f"{source}: {job_id} job"), f"{source}: {job_id} job"
        )
        if DISPATCH_COMMAND in normalized_text(step.get("run", ""))
    ]
    if len(dispatchers) != 1:
        raise WorkflowContractError(
            f"{source}: exactly one step must run {DISPATCH_COMMAND!r}; a release "
            "published with the repository GITHUB_TOKEN never triggers the "
            f"`release: published` event, and {len(dispatchers)} steps do"
        )
    job_id, job, step = dispatchers[0]

    permissions = require_mapping(
        job.get("permissions"), f"{source}: {job_id} permissions"
    )
    if permissions.get("actions") != "write":
        raise WorkflowContractError(
            f"{source}: the {job_id} job must hold `actions: write` to dispatch "
            "a workflow"
        )

    gate = f"{normalized_text(job.get('if', ''))} {normalized_text(step.get('if', ''))}"
    if PUBLICATION_OUTPUT not in gate:
        raise WorkflowContractError(
            f"{source}: the {job_id} dispatch must be gated on the release "
            f"{PUBLICATION_OUTPUT} output"
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
    if DEPLOY_GATE not in normalized_text(job.get("if", "")):
        raise WorkflowContractError(
            f"{source}: the {job_id} job must stay gated on {DEPLOY_GATE} so no "
            "eligible published release means no deployment"
        )


VALID_RELEASE_WORKFLOW = f"""\
jobs:
  release:
    outputs:
      {PUBLICATION_OUTPUT}: ${{{{ steps.release_plz.outputs.{PUBLICATION_OUTPUT} }}}}
    steps:
      - id: release_plz
        run: release-plz release
  release_pages:
    needs: [release]
    if: ${{{{ needs.release.outputs.{PUBLICATION_OUTPUT} == 'true' }}}}
    permissions:
      actions: write
    steps:
      - name: Dispatch the Pages documentation workflow on main
        env:
          GH_TOKEN: ${{{{ github.token }}}}
        run: {DISPATCH_COMMAND} --repo "${{GITHUB_REPOSITORY}}"
"""

VALID_PAGES_WORKFLOW = f"""\
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
      needs.prepare-release-site.outputs.{DEPLOY_GATE}
    steps:
      - uses: {DEPLOY_ACTION}v4
"""


def expect_rejected(
    label: str, check: Callable[[str, str], None], text: str
) -> None:
    try:
        check(text, label)
    except WorkflowContractError:
        return
    raise AssertionError(f"{label}: invalid fixture was accepted")


def self_test() -> None:
    check_release_workflow_text(VALID_RELEASE_WORKFLOW, "valid release fixture")
    check_pages_workflow_text(VALID_PAGES_WORKFLOW, "valid pages fixture")

    dispatch_step = (
        "      - name: Dispatch the Pages documentation workflow on main\n"
        "        env:\n"
        "          GH_TOKEN: ${{ github.token }}\n"
        f'        run: {DISPATCH_COMMAND} --repo "${{GITHUB_REPOSITORY}}"\n'
    )
    expect_rejected(
        # The regression itself: publication leaves Pages to the suppressed
        # `release: published` event.
        "release-event-only fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW.replace(dispatch_step, "      - run: true\n"),
    )
    expect_rejected(
        "wrong-ref fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW.replace(
            "--ref main", "--ref ${{ github.event.release.tag_name }}"
        ),
    )
    expect_rejected(
        "duplicated-dispatch fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW
        + dispatch_step.replace("      - ", "  extra:\n    steps:\n      - ", 1),
    )
    expect_rejected(
        "missing-actions-permission fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW.replace(
            "    permissions:\n      actions: write\n", "    permissions: {}\n"
        ),
    )
    expect_rejected(
        "string-permissions fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW.replace(
            "    permissions:\n      actions: write\n", "    permissions: write-all\n"
        ),
    )
    expect_rejected(
        "ungated-dispatch fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW.replace(
            f"    if: ${{{{ needs.release.outputs.{PUBLICATION_OUTPUT} == 'true' }}}}\n",
            "",
        ),
    )
    expect_rejected(
        "missing-token fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW.replace(
            "        env:\n          GH_TOKEN: ${{ github.token }}\n", ""
        ),
    )
    expect_rejected(
        "personal-token fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW.replace(
            "GH_TOKEN: ${{ github.token }}",
            "GH_TOKEN: ${{ secrets.PAGES_DISPATCH_TOKEN }}",
        ),
    )
    expect_rejected(
        "commented-dispatch fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW.replace(
            f"        run: {DISPATCH_COMMAND}", f"        # run: {DISPATCH_COMMAND}"
        ),
    )
    expect_rejected(
        "duplicate-permissions fixture",
        check_release_workflow_text,
        VALID_RELEASE_WORKFLOW.replace(
            "    permissions:\n      actions: write\n",
            "    permissions:\n      actions: write\n"
            "    permissions:\n      contents: read\n",
        ),
    )

    expect_rejected(
        "undispatchable-pages fixture",
        check_pages_workflow_text,
        VALID_PAGES_WORKFLOW.replace("  workflow_dispatch:\n", ""),
    )
    expect_rejected(
        "quoted-on fixture",
        check_pages_workflow_text,
        VALID_PAGES_WORKFLOW.replace("on:", '"on":').replace(
            "  workflow_dispatch:\n", ""
        ),
    )
    expect_rejected(
        "open-deployment fixture",
        check_pages_workflow_text,
        VALID_PAGES_WORKFLOW.replace(
            f"      needs.prepare-release-site.outputs.{DEPLOY_GATE}\n", "      true\n"
        ),
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
