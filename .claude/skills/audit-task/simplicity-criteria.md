# Simplicity-First Review Criteria

## Primary Principle

Evaluate the proposed change against the simplest resulting system, not the smallest modification to the current implementation.

Do not assume that preserving existing structure is desirable. Consider how the system would be designed if all currently supported features and the proposed feature were implemented from scratch today.

If a simpler architecture emerges from that exercise, the review should recommend refactoring toward that architecture even if it requires more changes in the current PR.

## Review Questions

1. If the system were implemented from scratch with the current feature set plus the proposed feature, would this design still be chosen?
2. Does the change introduce additional abstractions, special cases, configuration, indirection, deployment artifacts, services, data models, or code paths that would not exist in a clean-slate design?
3. Is the implementation simplifying the overall system, or merely minimizing changes to existing code?
4. Can the new feature be absorbed by generalizing existing behavior rather than introducing feature-specific logic?
5. Does the change remove complexity elsewhere, or only add complexity?
6. Are there obsolete patterns, abstractions, or workarounds that should be eliminated as part of this change?

## Preferred Outcomes

Prefer:

* Fewer concepts.
* Fewer code paths.
* Fewer deployment units.
* Fewer APIs and contracts.
* More general solutions over special cases.
* Refactoring toward a cleaner model when justified.
* Changes that reduce total system complexity.

Avoid:

* Preserving accidental complexity for compatibility with existing implementation details.
* Adding new layers solely to minimize change size.
* Feature-specific branching when a simpler generalization exists.
* Incremental complexity that accumulates over time.

The goal is the simplest maintainable end-state, not the smallest diff.

## Meta-rule

Treat "Would I design it this way from scratch?" as a stronger signal than "Does this fit the existing structure?" Existing structure is evidence, not a constraint.
