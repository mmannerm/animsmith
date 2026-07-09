#!/usr/bin/env bash
# Decide whether release-plz produced a CLI release and, if so, which tag
# to package binaries for. This is the single detection recipe shared by
# the release-plz.yml workflow and the local contract test, so the
# release-present / no-release-skip / missing-CLI-tag branches have one
# source of truth instead of inline shell that only ever runs in CI.
#
# Inputs (environment):
#   RELEASES_CREATED  release-plz `releases_created` output ("true" when a
#                     release was cut this run).
#   RELEASES          release-plz `releases` output (JSON array).
#
# Behaviour:
#   - RELEASES_CREATED != "true": print nothing, exit 0 (skip binaries).
#   - release present with a matching CLI tag: print the tag, exit 0.
#   - release present but no matching CLI tag: error, exit 1.
set -euo pipefail

# The CLI package whose tag drives binary packaging.
cli_package="animsmith"

releases_created="${RELEASES_CREATED:-}"
releases="${RELEASES:-}"

if [[ "${releases_created}" != "true" ]]; then
  # No release cut this run; nothing to package. Emit no tag.
  exit 0
fi

# release-plz can report several releases in one run; take the latest CLI
# tag. `.tag` on a record without a tag field yields the string "null",
# which the guard below rejects rather than emitting as a real tag.
tag="$(
  jq -r --arg pkg "${cli_package}" \
    '.[] | select(.package_name == $pkg) | .tag' <<<"${releases}" \
    | tail -n 1
)"

if [[ -z "${tag}" || "${tag}" == "null" ]]; then
  echo "release-plz reported releases but none for the ${cli_package} CLI package" >&2
  printf '%s\n' "${releases}" >&2
  exit 1
fi

printf '%s\n' "${tag}"
