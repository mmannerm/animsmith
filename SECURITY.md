# Security Policy

animsmith is a pre-1.0 Rust CLI and library workspace. Security fixes
target the current development line and the latest published crates once
the project is on crates.io.

| Version | Supported |
|---|---|
| current `main` | yes |
| latest crates.io release | yes, after first publish |
| older pre-1.0 releases | no, unless a maintainer explicitly says otherwise |

## Reporting A Vulnerability

Please report suspected vulnerabilities through GitHub private
vulnerability reporting:

https://github.com/mmannerm/animsmith/security/advisories/new

Do not open a public issue for security-sensitive reports. Include:

- Affected crate or CLI command.
- A minimal reproducer or input description.
- Expected impact.
- Whether the report depends on untrusted animation input, build-time
  behavior, or generated HTML reports.

Maintainers will triage the report, discuss remediation privately, and
publish an advisory if the issue has user impact.

## Scope

Security-sensitive areas include parser hardening for glTF/GLB and FBX
input, generated report output, dependency advisories, and behavior that
could crash or hang CI on untrusted assets.

Correctness bugs, false positives, and missing animation checks should be
filed as normal GitHub issues unless they expose a security impact.
