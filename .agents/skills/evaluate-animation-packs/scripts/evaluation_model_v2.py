"""V2 current evaluation-model identity and canonical JSON authority."""

from evaluation_model_v1 import *  # noqa: F403 - V2 deliberately retains V1 vocabulary.

SCHEMA = "urn:animsmith:skill:animation-pack-evaluation:2"
SCHEMA_VERSION = 2
COLLECTION_OUTPUT_SCHEMA = "urn:animsmith:schema:collection-output:11"
OUTPUT_SCHEMA = "urn:animsmith:schema:output:19"
MEASUREMENTS_SCHEMA = "urn:animsmith:schema:measurements:18"
MAX_COLLECTION_OUTPUT_BYTES = 256 * 1024 * 1024
SET_TYPES = frozenset({
    "gait-group", "sync-group", "directional-blend", "speed-blend",
    "transition-chain", "mask-composition", "retarget-group",
    "paired-interaction", "motion-database",
})
