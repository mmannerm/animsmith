#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

REPO_URL="https://github.com/mmannerm/animsmith"
REPO_BLOB_URL="${REPO_URL}/blob/main/"
REPO_TREE_URL="${REPO_URL}/tree/main/"
SUPPORT_URL="${REPO_BLOB_URL}SUPPORT.md"
SECURITY_URL="${REPO_BLOB_URL}SECURITY.md"
SECURITY_ADVISORY_URL="${REPO_URL}/security/advisories/new"

fail() {
  echo "github-community: $*" >&2
  exit 1
}

require_file() {
  test -f "$1" || fail "$1 is missing"
}

require_match() {
  local path="$1"
  local pattern="$2"
  local description="$3"

  require_file "$path"
  grep -Eq "$pattern" "$path" || fail "$path must include $description"
}

require_literal() {
  local path="$1"
  local literal="$2"
  local description="$3"

  require_file "$path"
  grep -Fq -- "$literal" "$path" || fail "$path must include $description"
}

validate_markdown_links() {
  local path="$1"
  local absolute_only="${2:-false}"
  local url url_no_anchor target local_path

  require_file "$path"
  while IFS= read -r url; do
    case "$url" in
      \#*) continue ;;
    esac

    url_no_anchor="${url%%#*}"
    case "$url" in
      http://*|https://*)
        if [[ "$url_no_anchor" == "$REPO_BLOB_URL"* ]]; then
          target="${url_no_anchor#"$REPO_BLOB_URL"}"
          test -f "$target" || fail "$path links to missing repository file $url"
        elif [[ "$url_no_anchor" == "$REPO_TREE_URL"* ]]; then
          target="${url_no_anchor#"$REPO_TREE_URL"}"
          test -d "$target" || fail "$path links to missing repository directory $url"
        fi
        ;;
      *)
        if [ "$absolute_only" = true ]; then
          fail "$path must use absolute links, found $url"
        fi
        local_path="$(dirname "$path")/$url_no_anchor"
        test -e "$local_path" || fail "$path links to missing local target $url"
        ;;
    esac
  done < <(grep -Eo '\[[^]]+\]\([^)]+\)' "$path" | sed -E 's/^[^()]*\(([^)]*)\)$/\1/' || true)
}

require_order() {
  local path="$1"
  local first="$2"
  local second="$3"
  local first_line second_line

  first_line="$(grep -nF "$first" "$path" | head -1 | cut -d: -f1 || true)"
  second_line="$(grep -nF "$second" "$path" | head -1 | cut -d: -f1 || true)"
  if [ -z "$first_line" ] || [ -z "$second_line" ] || [ "$first_line" -ge "$second_line" ]; then
    fail "$path must route CLI users before contributor docs"
  fi
}

require_issue_template() {
  local path="$1"
  local label="$2"
  local ids duplicate_ids

  require_match "$path" '^name:[[:space:]]*[^[:space:]]' "a name"
  require_match "$path" '^description:[[:space:]]*[^[:space:]]' "a description"
  if grep -Eq '^title:' "$path"; then
    fail "$path should keep taxonomy in labels, not a default title prefix"
  fi
  grep -Fxq "  - $label" "$path" || fail "$path must include $label"
  awk '
    $0 == "body:" { in_body = 1; next }
    in_body && /^[[:space:]]+-[[:space:]]+type:/ { found = 1 }
    END { exit found ? 0 : 1 }
  ' "$path" || fail "$path must define a non-empty body"

  ids="$(sed -nE 's/^[[:space:]]+id:[[:space:]]*([^[:space:]]+).*/\1/p' "$path")"
  duplicate_ids="$(printf '%s\n' "$ids" | sort | uniq -d | tr '\n' ' ')"
  if [ -n "$duplicate_ids" ]; then
    fail "$path must not repeat body ids: $duplicate_ids"
  fi
}

require_workflow_trigger() {
  local path="$1"
  local name="$2"

  require_match "$path" "^[[:space:]]*$name:" "workflow trigger $name"
}

forbid_workflow_trigger() {
  local path="$1"
  local name="$2"

  if grep -Eq "^[[:space:]]*$name:" "$path"; then
    fail "$path must not run on $name"
  fi
}

require_main_push() {
  local path="$1"

  require_workflow_trigger "$path" "push"
  require_literal "$path" "branches: [main]" "pushes only to main"
}

require_workflow_cron() {
  local path="$1"
  local cron="$2"

  require_workflow_trigger "$path" "schedule"
  require_literal "$path" "cron: '$cron'" "schedule $cron"
}

validate_markdown_links README.md true
for path in \
  CONTRIBUTING.md \
  DEVELOPMENT.md \
  SUPPORT.md \
  SECURITY.md \
  AGENTS.md \
  CLAUDE.md \
  .agent-instructions/shared.md \
  .github/PULL_REQUEST_TEMPLATE.md \
  docs/README.md \
  docs/why-animsmith.md \
  docs/game-ready-clips.md \
  docs/pipeline-scenarios.md \
  docs/cli.md \
  docs/embedding.md \
  docs/output.md \
  examples/README.md; do
  validate_markdown_links "$path"
done
for path in \
  crates/animsmith-core/README.md \
  crates/animsmith-gltf/README.md \
  crates/animsmith-fbx/README.md \
  crates/animsmith-report/README.md; do
  validate_markdown_links "$path" true
done

require_order README.md "cargo install animsmith" "CONTRIBUTING.md"
require_order README.md "animsmith lint clip.glb" "CONTRIBUTING.md"

require_match README.md "${REPO_BLOB_URL}docs/cli[.]md" "CLI reference link"
require_match README.md "${REPO_BLOB_URL}docs/embedding[.]md" "embedding API link"
require_match README.md "${REPO_BLOB_URL}CONTRIBUTING[.]md" "contributor guide link"
require_match README.md "${REPO_BLOB_URL}DEVELOPMENT[.]md" "development setup link"

require_match CONTRIBUTING.md '^## Pull Request Flow$' "PR flow"
require_match CONTRIBUTING.md '^## Conventional Commits$' "Conventional Commits policy"
require_match CONTRIBUTING.md '^## Documentation Freshness$' "documentation freshness policy"
require_literal CONTRIBUTING.md "type:docs" "type:docs follow-up route"
require_match CONTRIBUTING.md '^## Audit Expectations$' "audit expectations"
require_match CONTRIBUTING.md '^## Labels And Milestones$' "labels and milestones"
require_match CONTRIBUTING.md '^## Merge Policy$' "merge policy"

require_literal DEVELOPMENT.md "RELEASING.md" "maintainer release-doc link"
require_literal DEVELOPMENT.md "DESIGN.md" "architecture-doc link"
require_literal DEVELOPMENT.md 'MSRV `1.88`' "MSRV"
require_literal DEVELOPMENT.md "just install-rust-tools" "tool install command"
require_literal DEVELOPMENT.md "just gates" "local gate command"
require_literal DEVELOPMENT.md "just doc" "rustdoc command"
require_match DEVELOPMENT.md '^## Documentation Builds$' "documentation-builds section"
require_literal DEVELOPMENT.md "just golden" "golden test command"
require_literal DEVELOPMENT.md "sccache" "sccache notes"
require_literal DEVELOPMENT.md "--no-default-features" "no-default-features path"
require_literal DEVELOPMENT.md "just package-inventory" "package readiness check"
require_match DEVELOPMENT.md '^## Package Readiness$' "package-readiness section"

require_match RELEASING.md '^## Published README and docs links$' "published README link policy"
require_literal RELEASING.md "scripts/check-schema-id.sh" "schema check remains separate"

require_issue_template .github/ISSUE_TEMPLATE/bug_report.yml type:bug
require_issue_template .github/ISSUE_TEMPLATE/documentation_gap.yml type:docs
require_issue_template .github/ISSUE_TEMPLATE/feature_request.yml type:feature

grep -Fxq 'blank_issues_enabled: true' .github/ISSUE_TEMPLATE/config.yml \
  || fail ".github/ISSUE_TEMPLATE/config.yml must allow blank issues"
grep -Fxq "    url: $SUPPORT_URL" .github/ISSUE_TEMPLATE/config.yml \
  || fail ".github/ISSUE_TEMPLATE/config.yml must link SUPPORT.md"
grep -Fxq "    url: $SECURITY_URL" .github/ISSUE_TEMPLATE/config.yml \
  || fail ".github/ISSUE_TEMPLATE/config.yml must link SECURITY.md"

require_literal .github/PULL_REQUEST_TEMPLATE.md "## Documentation Impact" "a Documentation Impact section"
require_literal .github/PULL_REQUEST_TEMPLATE.md "CONTRIBUTING.md" "CONTRIBUTING.md for docs-impact policy"
require_literal .github/PULL_REQUEST_TEMPLATE.md "type:docs" "type:docs follow-ups"
require_literal .github/PULL_REQUEST_TEMPLATE.md "Published README/doc-link policy" "published README/doc-link policy"
require_literal .github/PULL_REQUEST_TEMPLATE.md "## Verification" "a Verification section"
require_literal .github/PULL_REQUEST_TEMPLATE.md "just package-inventory" "package/readiness changes route"

require_literal SUPPORT.md "GitHub Discussions are" "support discussion routing"
require_literal SUPPORT.md "not enabled" "support discussion routing"
require_literal SUPPORT.md 'issues/new?template=documentation_gap.yml' "documentation-gap issue template link"
require_literal SECURITY.md "$SECURITY_ADVISORY_URL" "private vulnerability reporting"

require_main_push .github/workflows/codeql.yml
require_workflow_cron .github/workflows/codeql.yml "41 5 * * 2"
forbid_workflow_trigger .github/workflows/codeql.yml "pull_request"

require_workflow_trigger .github/workflows/coverage.yml "pull_request"
require_main_push .github/workflows/coverage.yml
require_literal .github/workflows/coverage.yml "codecov/codecov-action@" "CodeCov upload"

echo "GitHub community files are valid"
