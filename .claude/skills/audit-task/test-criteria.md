# Testing Review Criteria

## Primary Principle

Tests should primarily validate externally observable behavior through module contracts, public interfaces, and system boundaries.

Favor acceptance-style unit tests that exercise behavior from the consumer's perspective.

## Preferred Test Structure

Highest priority:

* Contract tests.
* Public API tests.
* Module boundary tests.
* Behavioral tests.
* End-to-end flows within the module.

Secondary priority:

* Granular unit tests for complex algorithms.
* Error handling paths.
* Edge cases.
* Coverage-oriented tests for internal branches.

## Review Questions

1. Would the tests still pass after a significant internal refactoring that preserves behavior?
2. Are tests coupled to implementation details, internal methods, private classes, data structures, or call sequences?
3. Do the tests verify outcomes and contracts rather than implementation mechanics?
4. Does the test suite describe the expected behavior of the module?
5. Are internal tests justified by meaningful risk, complexity, or error handling requirements?

## Preferred Characteristics

Prefer:

* Black-box tests over white-box tests.
* Contract verification over implementation verification.
* User-visible behavior over internal interactions.
* Stable tests that survive refactoring.

Avoid:

* Testing private implementation details.
* Mock-heavy tests that mirror implementation structure.
* Assertions on internal call ordering unless it is part of the contract.
* Tests whose primary purpose is preserving current implementation rather than behavior.

A good test suite should enable aggressive refactoring while preserving confidence in correctness.
