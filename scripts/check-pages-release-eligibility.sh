#!/usr/bin/env bash
# Print the exact GitHub-output assignment consumed by docs-pages.yml.
# A release predating the Pages foundation is not a substitute source for the
# release root: it is intentionally ineligible until a tagged revision carries
# the pinned mdBook contract.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <release-tag>" >&2
  exit 2
fi

if git cat-file -e "$1:.mdbook-version" 2>/dev/null; then
  echo "available=true"
else
  echo "available=false"
fi
