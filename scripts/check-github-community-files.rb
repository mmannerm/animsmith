#!/usr/bin/env ruby
# frozen_string_literal: true

require "yaml"

REPO_URL = "https://github.com/mmannerm/animsmith"
REPO_BLOB_URL = "#{REPO_URL}/blob/main/"
REPO_TREE_URL = "#{REPO_URL}/tree/main/"
SUPPORT_URL = "#{REPO_BLOB_URL}SUPPORT.md"
SECURITY_URL = "#{REPO_BLOB_URL}SECURITY.md"
SECURITY_ADVISORY_URL = "#{REPO_URL}/security/advisories/new"

def fail!(message)
  warn message
  exit 1
end

def read(path)
  File.read(path)
rescue Errno::ENOENT
  fail!("#{path} is missing")
end

def load_yaml(path)
  YAML.load_file(path)
rescue Psych::SyntaxError => e
  fail!("#{path} is not valid YAML: #{e.message}")
rescue Errno::ENOENT
  fail!("#{path} is missing")
end

def workflow_triggers(path)
  data = load_yaml(path)
  triggers = data["on"] || data[true]
  fail!("#{path} must define workflow triggers") unless triggers
  triggers
end

def workflow_trigger_config(path, name)
  triggers = workflow_triggers(path)
  fail!("#{path} workflow triggers must be a mapping") unless triggers.is_a?(Hash)

  return triggers[name] || {} if triggers.key?(name)

  nil
end

def require_workflow_trigger(path, name)
  fail!("#{path} must run on #{name}") unless workflow_trigger_config(path, name)
end

def forbid_workflow_trigger(path, name)
  fail!("#{path} must not run on #{name}") if workflow_trigger_config(path, name)
end

def require_main_push(path)
  push = workflow_trigger_config(path, "push")
  fail!("#{path} must run on push") unless push

  branches = Array(push["branches"])
  fail!("#{path} must run only on pushes to main") unless branches == ["main"]
end

def require_workflow_cron(path, cron)
  schedule = workflow_trigger_config(path, "schedule")
  fail!("#{path} must run on schedule") unless schedule

  crons = Array(schedule).map { |entry| entry["cron"] if entry.is_a?(Hash) }.compact
  fail!("#{path} must schedule #{cron}") unless crons.include?(cron)
end

def require_text(path, pattern, description)
  text = read(path)
  fail!("#{path} must #{description}") unless text.match?(pattern)
end

def markdown_links(path)
  read(path).scan(/\[[^\]]+\]\(([^)]+)\)/).flatten
end

def local_target_for(url)
  target = url.split("#", 2).first

  if target.start_with?(REPO_BLOB_URL)
    return [target.delete_prefix(REPO_BLOB_URL), :file]
  end

  return [target.delete_prefix(REPO_TREE_URL), :directory] if target.start_with?(REPO_TREE_URL)

  [target, :any]
end

def validate_markdown_links(path, absolute_only: false)
  markdown_links(path).each do |url|
    next if url.start_with?("#")

    if url.start_with?("http://", "https://")
      target, kind = local_target_for(url)
      case kind
      when :file
        fail!("#{path} links to missing repository file #{url}") unless File.file?(target)
      when :directory
        fail!("#{path} links to missing repository directory #{url}") unless File.directory?(target)
      end

      next
    end

    fail!("#{path} must use absolute links, found #{url}") if absolute_only

    local_path = File.expand_path(url.split("#", 2).first, File.dirname(path))
    fail!("#{path} links to missing local target #{url}") unless File.exist?(local_path)
  end
end

def require_all(path, expectations)
  text = read(path)
  expectations.each do |description, pattern|
    fail!("#{path} must include #{description}") unless text.match?(pattern)
  end
end

readme = read("README.md")
validate_markdown_links("README.md", absolute_only: true)
%w[
  CONTRIBUTING.md
  DEVELOPMENT.md
  SUPPORT.md
  SECURITY.md
  AGENTS.md
  CLAUDE.md
  .agent-instructions/shared.md
  .github/PULL_REQUEST_TEMPLATE.md
].each { |path| validate_markdown_links(path) }

install_index = readme.index("cargo install animsmith")
quickstart_index = readme.index("animsmith lint clip.glb")
contributor_index = readme.index("CONTRIBUTING.md")
fail!("README.md must route CLI users before contributor docs") unless install_index && quickstart_index && contributor_index && install_index < contributor_index && quickstart_index < contributor_index

require_all(
  "README.md",
  "CLI reference link" => %r{#{Regexp.escape(REPO_BLOB_URL)}docs/cli\.md},
  "embedding API link" => %r{#{Regexp.escape(REPO_BLOB_URL)}docs/embedding\.md},
  "contributor guide link" => %r{#{Regexp.escape(REPO_BLOB_URL)}CONTRIBUTING\.md},
  "development setup link" => %r{#{Regexp.escape(REPO_BLOB_URL)}DEVELOPMENT\.md}
)

require_all(
  "CONTRIBUTING.md",
  "PR flow" => /^## Pull Request Flow$/m,
  "Conventional Commits policy" => /^## Conventional Commits$/m,
  "documentation freshness policy" => /^## Documentation Freshness$/m,
  "type:docs follow-up route" => /type:docs/,
  "audit expectations" => /^## Audit Expectations$/m,
  "labels and milestones" => /^## Labels And Milestones$/m,
  "merge policy" => /^## Merge Policy$/m
)

require_all(
  "DEVELOPMENT.md",
  "maintainer release-doc link" => /RELEASING\.md/,
  "architecture-doc link" => /DESIGN\.md/,
  "MSRV" => /MSRV `1\.88`/,
  "tool install command" => /just install-rust-tools/,
  "local gate command" => /just gates/,
  "rustdoc command" => /just doc/,
  "golden test command" => /just golden/,
  "sccache notes" => /sccache/,
  "no-default-features path" => /--no-default-features/,
  "package readiness check" => /just package-inventory/
)

issue_templates = {
  ".github/ISSUE_TEMPLATE/bug_report.yml" => "type:bug",
  ".github/ISSUE_TEMPLATE/documentation_gap.yml" => "type:docs",
  ".github/ISSUE_TEMPLATE/feature_request.yml" => "type:feature",
}

issue_templates.each do |path, label|
  data = load_yaml(path)
  fail!("#{path} must define a name") unless data["name"].is_a?(String) && !data["name"].empty?
  fail!("#{path} must define a description") unless data["description"].is_a?(String) && !data["description"].empty?
  fail!("#{path} should keep taxonomy in labels, not a default title prefix") if data.key?("title")
  fail!("#{path} must include #{label}") unless Array(data["labels"]).include?(label)

  body = data["body"]
  fail!("#{path} must define a non-empty body") unless body.is_a?(Array) && !body.empty?

  ids = body.map { |entry| entry["id"] }.compact
  counts = Hash.new(0)
  ids.each { |id| counts[id] += 1 }
  duplicate_ids = counts.select { |_id, count| count > 1 }.keys
  fail!("#{path} must not repeat body ids: #{duplicate_ids.join(", ")}") unless duplicate_ids.empty?
end

config = load_yaml(".github/ISSUE_TEMPLATE/config.yml")
fail!(".github/ISSUE_TEMPLATE/config.yml must allow blank issues") unless config["blank_issues_enabled"] == true

contact_links = Array(config["contact_links"])
urls = contact_links.map { |entry| entry["url"] }
fail!(".github/ISSUE_TEMPLATE/config.yml must link SUPPORT.md") unless urls.include?(SUPPORT_URL)
fail!(".github/ISSUE_TEMPLATE/config.yml must link SECURITY.md") unless urls.include?(SECURITY_URL)

pr_template = read(".github/PULL_REQUEST_TEMPLATE.md")
fail!(".github/PULL_REQUEST_TEMPLATE.md must include a Documentation Impact section") unless pr_template.include?("## Documentation Impact")
fail!(".github/PULL_REQUEST_TEMPLATE.md must point at CONTRIBUTING.md for docs-impact policy") unless pr_template.include?("CONTRIBUTING.md")
fail!(".github/PULL_REQUEST_TEMPLATE.md must require or route type:docs follow-ups") unless pr_template.include?("type:docs")
fail!(".github/PULL_REQUEST_TEMPLATE.md must include a Verification section") unless pr_template.include?("## Verification")

require_text("SUPPORT.md", /GitHub Discussions are\s+not enabled/m, "route support with Discussions disabled")
require_text("SUPPORT.md", %r{issues/new\?template=documentation_gap\.yml}, "link the documentation-gap issue template")
require_text("SECURITY.md", /#{Regexp.escape(SECURITY_ADVISORY_URL)}/, "point to private vulnerability reporting")

require_main_push(".github/workflows/codeql.yml")
require_workflow_cron(".github/workflows/codeql.yml", "41 5 * * 2")
forbid_workflow_trigger(".github/workflows/codeql.yml", "pull_request")

require_workflow_trigger(".github/workflows/coverage.yml", "pull_request")
require_main_push(".github/workflows/coverage.yml")
require_text(".github/workflows/coverage.yml", /codecov\/codecov-action@/, "upload coverage to Codecov")

puts "GitHub community files are valid"
