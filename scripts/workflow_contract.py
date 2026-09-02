"""Shared structural decoding for GitHub workflow contract checks.

Workflow contracts are asserted after YAML decoding rather than over raw
text, so quoted keys, aliases, merges, and duplicate mappings cannot hide a
missing guard from a text scan.
"""

from __future__ import annotations

from typing import Any

import yaml


class WorkflowContractError(ValueError):
    """A workflow failed a structural contract check."""


class UniqueSafeLoader(yaml.SafeLoader):
    """SafeLoader variant that rejects duplicate keys after merge expansion."""


def construct_unique_mapping(
    loader: UniqueSafeLoader, node: yaml.MappingNode, deep: bool = False
) -> dict[Any, Any]:
    loader.flatten_mapping(node)
    mapping: dict[Any, Any] = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        try:
            duplicate = key in mapping
        except TypeError as exc:
            raise yaml.constructor.ConstructorError(
                "while constructing a mapping",
                node.start_mark,
                "unhashable key",
                key_node.start_mark,
            ) from exc
        if duplicate:
            raise yaml.constructor.ConstructorError(
                "while constructing a mapping",
                node.start_mark,
                f"found duplicate key {key!r}",
                key_node.start_mark,
            )
        mapping[key] = loader.construct_object(value_node, deep=deep)
    return mapping


UniqueSafeLoader.add_constructor(
    yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, construct_unique_mapping
)


def load_workflow(text: str, source: str) -> dict[str, Any]:
    try:
        document = yaml.load(text, Loader=UniqueSafeLoader)
    except yaml.YAMLError as exc:
        raise WorkflowContractError(f"{source}: invalid YAML: {exc}") from exc
    if not isinstance(document, dict):
        raise WorkflowContractError(f"{source}: workflow root must be a mapping")
    return document


def require_mapping(value: object, description: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise WorkflowContractError(f"{description} must be a mapping")
    return value


def workflow_triggers(document: dict[str, Any], source: str) -> dict[str, Any]:
    """Return the workflow's triggers keyed by event name.

    YAML 1.1 decodes an unquoted ``on`` key as the boolean ``True``, so the
    trigger block is looked up under both spellings. GitHub also accepts a
    bare event name or a list of them; both are normalized to a mapping.
    """
    for key in (True, "on"):
        if key not in document:
            continue
        triggers = document[key]
        if isinstance(triggers, str):
            return {triggers: None}
        if isinstance(triggers, list):
            return {str(event): None for event in triggers}
        return require_mapping(triggers, f"{source}: on")
    raise WorkflowContractError(f"{source}: workflow must define triggers")


def normalized_text(value: object) -> str:
    """Collapse a YAML scalar to single-spaced text for command matching.

    Shell line continuations and YAML block folding are normalized away, so a
    contract can match the command a maintainer would type.
    """
    return " ".join(str(value).replace("\\\n", " ").split())
