"""Version-1 machine vocabulary for animation-pack evaluation artifacts."""

SCHEMA = "urn:animsmith:skill:animation-pack-evaluation-manifest:1"
TAXONOMY_VERSION = "1"
PROFILE_SET_VERSION = "1"

PRIMARY_ROLES = (
    "idle-pose",
    "continuous-locomotion",
    "locomotion-transition",
    "airborne",
    "traversal",
    "action-interaction",
    "reaction-death",
    "emote-cinematic",
    "other-unknown",
)
VARIANTS = {
    "in-place",
    "root-motion",
    "rotation-only-root",
    "single",
    "unknown",
}
SET_TYPES = {
    "directional-blend",
    "speed-blend",
    "sync-group",
    "transition-chain",
    "mask-composition",
    "retarget-group",
    "paired-interaction",
    "motion-database",
    "other",
}
CLASSIFICATION_BASES = {
    "user-required",
    "vendor-stated",
    "observed-file",
    "inferred",
}
CONFIDENCE = {"high", "medium", "low"}

PROFILE_ROWS = (
    ("marketplace-intake", "Marketplace intake"),
    ("blended-locomotion", "Blended locomotion"),
    ("root-motion-controller", "Root-motion controller"),
    ("state-machine-transitions", "State-machine transitions"),
    ("layered-upper-body-weapons", "Layered upper body/weapons"),
    ("traversal-environment", "Traversal/environment"),
    ("contact-actions-interactions", "Contact actions/interactions"),
    (
        "retargeted-customizable-characters",
        "Retargeted/customizable characters",
    ),
    ("motion-matching-search", "Motion matching/search"),
    ("networked-movement", "Networked movement"),
    ("runtime-performance", "Runtime performance"),
)
PROFILE_IDS = tuple(identifier for identifier, _label in PROFILE_ROWS)
PROFILE_STATUSES = {"selected", "not-selected", "not-applicable"}
ACTIVATION_BASES = {
    "user-required",
    "vendor-intended",
    "observed-pack-capability",
    "evaluator-selected-generic-scenario",
}

PIPELINE_STAGE_ROWS = (
    ("acquire", "Acquire"),
    ("preserve-raw", "Preserve raw"),
    ("inspect", "Inspect"),
    ("segment", "Segment"),
    ("root-motion", "Root motion"),
    ("conform", "Conform"),
    ("validate", "Validate"),
    ("optimize", "Optimize"),
    ("export", "Export"),
    ("gate-report", "Gate/report"),
)
PIPELINE_STAGES = tuple(identifier for identifier, _label in PIPELINE_STAGE_ROWS)
COVERAGE_STATES = {
    "evaluated-clean",
    "evaluated-finding",
    "partially-evaluated",
    "not-applicable",
    "not-evaluated",
    "unsupported-input",
    "unavailable-evidence",
}

PRIMARY_OWNERS = {
    "engine-config",
    "animsmith-current-safe",
    "animsmith-current-declared",
    "animsmith-future-candidate",
    "artist-author",
    "vendor-license",
    "unknown",
}
