use animsmith_core::all_checks;
use std::collections::BTreeSet;

const README: &str = include_str!("../../../README.md");
const GAME_READY_CLIPS: &str = include_str!("../../../docs/game-ready-clips.md");

const DOCS: &[(&str, &str)] = &[
    ("README.md", README),
    ("docs/game-ready-clips.md", GAME_READY_CLIPS),
];

const EXEMPTED_REGISTERED_CHECK_IDS: &[&str] = &[];

const NON_CHECK_ID_LIKE_TOKENS: &[&str] = &[
    "animsmith-core",
    "animsmith-fbx",
    "animsmith-gltf",
    "animsmith-report",
    "ue-mannequin",
];

#[test]
fn docs_check_ids_match_the_registered_catalog() {
    let catalog = registered_check_ids();

    for (path, markdown) in DOCS {
        let tokens = inline_code_tokens(markdown);
        let unknown_check_ids: Vec<_> = tokens
            .iter()
            .copied()
            .filter(|token| looks_like_check_id(token))
            .filter(|token| !catalog.contains(token))
            .filter(|token| !NON_CHECK_ID_LIKE_TOKENS.contains(token))
            .collect();
        assert!(
            unknown_check_ids.is_empty(),
            "{path} names check-like ids that are not registered: {unknown_check_ids:?}"
        );

        let documented: BTreeSet<_> = tokens
            .iter()
            .copied()
            .filter(|token| catalog.contains(token))
            .collect();
        let missing: Vec<_> = catalog
            .iter()
            .copied()
            .filter(|id| !documented.contains(id))
            .filter(|id| !EXEMPTED_REGISTERED_CHECK_IDS.contains(id))
            .collect();
        assert!(
            missing.is_empty(),
            "{path} does not document registered checks: {missing:?}"
        );
    }
}

fn registered_check_ids() -> BTreeSet<&'static str> {
    let checks = all_checks();
    let ids: Vec<_> = checks.iter().map(|check| check.id()).collect();
    let unique: BTreeSet<_> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len(), "duplicate registered check id");
    unique
}

fn inline_code_tokens(markdown: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut in_fence = false;

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find('`') {
            let after_start = &rest[start + 1..];
            let Some(end) = after_start.find('`') else {
                break;
            };
            tokens.push(&after_start[..end]);
            rest = &after_start[end + 1..];
        }
    }

    tokens
}

fn looks_like_check_id(token: &str) -> bool {
    (token.contains('-') || matches!(token, "fps" | "nan"))
        && !token.starts_with('-')
        && !token.ends_with('-')
        && token.chars().all(|ch| ch.is_ascii_lowercase() || ch == '-')
}
