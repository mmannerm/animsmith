#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

REPO_URL="https://github.com/mmannerm/animsmith"

# The two flags that must travel together on any gate command line. Declared on
# one line, and referenced rather than repeated below, so this file passes the
# rule it enforces instead of having to exempt itself from it.
VARIANT_FLAG="--no-default-features"; ISOLATION_FLAG="--target-dir target/no-default-features"

# Cargo.toml's `rust-version` is the single source of truth for the MSRV;
# every prose mention of it is checked against this value below.
MSRV="$(sed -n 's/^rust-version = "\(.*\)"$/\1/p' Cargo.toml | head -1)"
test -n "$MSRV" || { echo "github-community: Cargo.toml has no rust-version" >&2; exit 1; }
REPO_BLOB_URL="${REPO_URL}/blob/main/"
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

# Validate the workflow structurally after YAML decoding; this handles quoted
# keys, aliases, merges, and duplicate mappings that text scans cannot model.
require_animation_pack_workflow() {
  require_file .github/workflows/checks.yml
  python3 scripts/check-animation-pack-workflow.py --workflow .github/workflows/checks.yml
  python3 scripts/check-animation-pack-workflow.py --self-test
}

# `just gates` and CI both build the CLI twice. While a --no-default-features
# build wrote to the conventional target directory, the artifact a gate run
# left at target/release/animsmith was whichever variant ran last -- a binary
# that rejects FBX while `--version` looked identical (#653). Every such
# command, in any file the gate runs, must redirect its own artifacts.
require_feature_isolation() {
  local offenders

  require_file justfile
  require_file .github/workflows/checks.yml

  # Any line naming the flag must isolate on that same line. Not "any cargo
  # line": a folded YAML scalar or a shell continuation could otherwise put
  # `cargo` and `--no-default-features` on separate source lines and slip an
  # unisolated command past a per-command rule. Requiring the two flags to
  # travel together needs no command reconstruction and no YAML decode.
  # Comment lines are skipped, so prose cannot satisfy the rule and commenting
  # a command out cannot hide it.
  offenders="$(
    awk -v variant="$VARIANT_FLAG" -v isolation="$ISOLATION_FLAG" '
      { line = $0; sub(/^[[:space:]]+/, "", line) }
      line ~ /^#/ { next }
      index(line, variant) == 0 { next }
      index(line, isolation) { next }
      { printf "%s:%d: %s\n", FILENAME, FNR, line }
    ' justfile .github/workflows/checks.yml scripts/*.sh
  )"
  test -z "$offenders" || fail "every line naming $VARIANT_FLAG in a gate command must name $ISOLATION_FLAG too, or the build overwrites the default-feature artifact at target/release/animsmith -- $offenders"

  # The probe judges what a run leaves behind, so nothing may run after it and
  # replace the artifact it just attested to.
  awk '
    /^gates:/ { in_recipe = 1; next }
    in_recipe && /^[^[:space:]]/ { exit }
    in_recipe {
      line = $0; sub(/^[[:space:]]+/, "", line)
      if (line == "" || line ~ /^#/) next
      last = line
    }
    END { exit last == "just release-cli" ? 0 : 1 }
  ' justfile || fail "the justfile gates recipe must end with 'just release-cli' so no later recipe can replace the artifact it probed"

  awk '
    /^  [[:alnum:]_-]+:$/ { job = $0; sub(/^  /, "", job); sub(/:$/, "", job) }
    job == "test" && /^[[:space:]]+(- )?(name: .*|run: .*)$/ {
      line = $0; sub(/^[[:space:]]+(- )?/, "", line)
      if (line ~ /^run: /) last = line
    }
    END { exit last == "run: bash scripts/check-release-cli.sh" ? 0 : 1 }
  ' .github/workflows/checks.yml || fail "the checks.yml test job must end with 'run: bash scripts/check-release-cli.sh' so no later step can replace the artifact it probed"

  # The path is spelled in three files; the probe would report a missing binary
  # rather than a drifted contract if this one fell out of step.
  require_literal scripts/check-release-cli.sh "target/no-default-features" \
    "the isolated target directory whose artifact it probes"
}

# A release published with the repository GITHUB_TOKEN creates no workflow run,
# so the Pages root only follows the new tag while release-plz.yml dispatches
# docs-pages.yml itself. Validate that path structurally, after YAML decoding.
require_pages_release_trigger() {
  require_file .github/workflows/release-plz.yml
  require_file .github/workflows/docs-pages.yml
  python3 scripts/check-pages-release-trigger.py \
    --release-workflow .github/workflows/release-plz.yml \
    --pages-workflow .github/workflows/docs-pages.yml
  python3 scripts/check-pages-release-trigger.py --self-test
}

# Dependabot treats a version-shaped action ref as a tag to bump, so
# `dtolnay/rust-toolchain@1.88` gets silently rewritten to whatever number
# looks newest -- it once proposed `@1.100`, a nightly number, for the MSRV
# job. Branch refs (`@stable`, `@nightly`) and SHA pins carrying an explicit
# `toolchain:` input are immune, so those are the only forms allowed.
require_no_versioned_rust_toolchain_ref() {
  local offenders

  offenders="$(grep -rnE 'uses: *dtolnay/rust-toolchain@[0-9]+\.' .github/workflows || true)"
  test -z "$offenders" || fail "workflows must not pin a Rust version through the action ref, which Dependabot rewrites; use a branch ref or a SHA pin with an explicit toolchain: input -- $offenders"
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

# Markdown link validation (required README routes, target existence,
# #anchor resolution, and the absolute-only policy for published
# READMEs) lives in the markdown-parser-backed workspace test
# crates/animsmith/tests/docs_links.rs (pulldown-cmark), which runs
# under `cargo test --workspace`; its sibling docs_index.rs keeps the
# Document-index completeness gate. This script keeps the assertions
# that are not Markdown-link-shaped: required literals, ordering,
# issue-form, and workflow contracts.

require_order README.md "cargo install animsmith" "CONTRIBUTING.md"
require_order README.md "animsmith lint clip.glb" "CONTRIBUTING.md"

require_match CONTRIBUTING.md '^## Pull Request Flow$' "PR flow"
require_match CONTRIBUTING.md '^## Conventional Commits$' "Conventional Commits policy"
require_match CONTRIBUTING.md '^## Documentation Freshness$' "documentation freshness policy"
require_literal CONTRIBUTING.md "type:docs" "type:docs follow-up route"
require_match CONTRIBUTING.md '^## Audit Expectations$' "audit expectations"
require_match CONTRIBUTING.md '^## Labels And Milestones$' "labels and milestones"
require_match CONTRIBUTING.md '^## Merge Policy$' "merge policy"

require_literal DEVELOPMENT.md "RELEASING.md" "maintainer release-doc link"
require_literal DEVELOPMENT.md "DESIGN.md" "architecture-doc link"
require_literal DEVELOPMENT.md "MSRV \`$MSRV\`" "MSRV"
require_no_versioned_rust_toolchain_ref
require_literal DEVELOPMENT.md "just install-rust-tools" "tool install command"
require_literal DEVELOPMENT.md "just gates" "local gate command"
require_literal DEVELOPMENT.md "just doc" "rustdoc command"
require_match DEVELOPMENT.md '^## Documentation Builds$' "documentation-builds section"
require_literal DEVELOPMENT.md "just golden" "golden test command"
require_literal DEVELOPMENT.md "sccache" "sccache notes"
require_literal DEVELOPMENT.md "$VARIANT_FLAG" "no-default-features path"
require_literal DEVELOPMENT.md "just release-cli" "retained release CLI proof"
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

# The animation-pack validator is a separate single-run PR check rather than
# another leg of the platform test matrix. Keep its invocation anchored in the
# reusable workflow so removal cannot silently erase exact-head evidence.
require_animation_pack_workflow

# `just gates` green locally must mean PR CI green, so the local recipe and the
# reusable workflow are held to the same feature-variant isolation (#653).
require_feature_isolation

# The Pages root tracks the latest published release and /dev/ tracks main, so
# a successful publication must reach the Pages workflow (#652).
require_pages_release_trigger

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
