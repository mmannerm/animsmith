use std::collections::{BTreeMap, BTreeSet};

use animsmith_core::check::{Check, CheckCtx};
use animsmith_core::config::{CheckSettings, SeveritySetting};
use animsmith_core::{
    Applicability, CheckOutput, CheckSelection, Config, ConfigurationState, CoverageGap,
    CoverageGapCode, Document, EvaluationError, EvaluationScope, EvaluationScopeCode,
    EvaluationState, Finding, MetricGrids, ResolvedRoles, SelectionState, Severity, Value,
    evaluate_checks,
};

struct Complete;

impl Check for Complete {
    fn id(&self) -> &'static str {
        "complete"
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
    }
}

struct FindingCheck;

impl Check for FindingCheck {
    fn id(&self) -> &'static str {
        "finding"
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        CheckOutput::from_coverage(
            vec![Finding::new(
                self.id(),
                Severity::Warning,
                "content warning",
            )],
            Vec::new(),
            Vec::new(),
        )
    }
}

struct Partial;

impl Check for Partial {
    fn id(&self) -> &'static str {
        "partial"
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        CheckOutput::from_coverage(
            vec![Finding::new(self.id(), Severity::Error, "member missing")],
            vec![EvaluationScope::new(EvaluationScopeCode::MEMBER_EXISTENCE)],
            vec![
                CoverageGap::new(CoverageGapCode::ROLES_UNRESOLVED, "display text")
                    .scope(EvaluationScope::new(EvaluationScopeCode::PHASE_COHERENCE)),
            ],
        )
    }
}

struct Unevaluated;

impl Check for Unevaluated {
    fn id(&self) -> &'static str {
        "unevaluated"
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        CheckOutput::from_coverage(
            Vec::new(),
            Vec::new(),
            vec![CoverageGap::new(
                CoverageGapCode::custom("acme:input_unavailable"),
                "nothing evaluated",
            )],
        )
    }
}

struct PoisonCheck {
    id: &'static str,
    applicable: bool,
}

impl Check for PoisonCheck {
    fn id(&self) -> &'static str {
        self.id
    }

    fn applicability(&self, _ctx: &CheckCtx) -> Applicability {
        if self.applicable {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        panic!("inactive check {} must not evaluate", self.id)
    }
}

struct MismatchedFinding;

impl Check for MismatchedFinding {
    fn id(&self) -> &'static str {
        "parent"
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        CheckOutput::from_coverage(
            vec![Finding::new("other", Severity::Error, "wrong owner")],
            Vec::new(),
            Vec::new(),
        )
    }
}

fn catalog() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(Complete),
        Box::new(FindingCheck),
        Box::new(Partial),
        Box::new(Unevaluated),
    ]
}

fn with_ctx(f: impl FnOnce(&CheckCtx<'_>)) {
    let doc = Document::default();
    let roles = ResolvedRoles::default();
    let config = Config::default();
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    f(&ctx);
}

#[test]
fn records_complete_findings_partial_and_not_evaluated() {
    with_ctx(|ctx| {
        let records = evaluate_checks(ctx, &catalog(), CheckSelection::All).unwrap();
        assert_eq!(records.len(), 4);

        assert_eq!(records[0].evaluation(), EvaluationState::Complete);
        assert!(records[0].findings().is_empty());

        assert_eq!(records[1].evaluation(), EvaluationState::Complete);
        assert_eq!(records[1].findings().len(), 1);

        assert_eq!(records[2].evaluation(), EvaluationState::Partial);
        assert_eq!(records[2].findings().len(), 1);
        assert_eq!(records[2].gaps()[0].code, CoverageGapCode::ROLES_UNRESOLVED);
        assert_eq!(
            records[2].evaluated_scopes()[0].code.as_str(),
            "member_existence"
        );

        assert_eq!(records[3].applicability(), Applicability::Applicable);
        assert_eq!(records[3].evaluation(), EvaluationState::NotEvaluated);
        assert_eq!(records[3].gaps()[0].code.as_str(), "acme:input_unavailable");
    });
}

#[test]
fn disabled_unselected_and_not_applicable_are_independent_and_never_execute() {
    let doc = Document::default();
    let roles = ResolvedRoles::default();
    let config = Config {
        checks: BTreeMap::from([(
            "disabled".to_string(),
            CheckSettings {
                severity: Some(SeveritySetting::Off),
                ..CheckSettings::default()
            },
        )]),
        ..Config::default()
    };
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let selected = BTreeSet::from(["disabled".to_string()]);
    let checks: Vec<Box<dyn Check>> = vec![
        Box::new(PoisonCheck {
            id: "unselected-applicable",
            applicable: true,
        }),
        Box::new(PoisonCheck {
            id: "disabled",
            applicable: true,
        }),
        Box::new(PoisonCheck {
            id: "unselected-not-applicable",
            applicable: false,
        }),
    ];
    let records = evaluate_checks(&ctx, &checks, CheckSelection::Only(&selected)).unwrap();

    assert_eq!(records[0].selection(), SelectionState::Unselected);
    assert_eq!(records[0].configuration(), ConfigurationState::Enabled);
    assert_eq!(records[0].applicability(), Applicability::Applicable);
    assert_eq!(records[0].evaluation(), EvaluationState::NotEvaluated);
    assert_eq!(records[1].selection(), SelectionState::Selected);
    assert_eq!(records[1].configuration(), ConfigurationState::Disabled);
    assert_eq!(records[1].applicability(), Applicability::Applicable);
    assert_eq!(records[2].selection(), SelectionState::Unselected);
    assert_eq!(records[2].applicability(), Applicability::NotApplicable);

    for record in &records {
        assert!(
            record.findings().is_empty(),
            "inactive check emitted findings"
        );
        assert!(
            record.evaluated_scopes().is_empty(),
            "inactive check claimed evaluated scopes"
        );
        assert!(
            record.gaps().is_empty(),
            "inactive check emitted coverage gaps"
        );
    }
}

#[test]
fn severity_override_changes_findings_but_not_gap_typing() {
    let doc = Document::default();
    let roles = ResolvedRoles::default();
    let config = Config {
        checks: BTreeMap::from([(
            "partial".to_string(),
            CheckSettings {
                severity: Some(SeveritySetting::Note),
                ..CheckSettings::default()
            },
        )]),
        ..Config::default()
    };
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let records = evaluate_checks(&ctx, &catalog(), CheckSelection::All).unwrap();

    assert_eq!(records[2].findings()[0].severity, Severity::Note);
    assert_eq!(records[2].gaps()[0].code, CoverageGapCode::ROLES_UNRESOLVED);
}

#[test]
fn catalog_and_output_invariants_return_typed_errors() {
    with_ctx(|ctx| {
        let duplicate: Vec<Box<dyn Check>> = vec![Box::new(Complete), Box::new(Complete)];
        assert_eq!(
            evaluate_checks(ctx, &duplicate, CheckSelection::All).unwrap_err(),
            EvaluationError::DuplicateCheckId("complete")
        );

        let selected = BTreeSet::from(["missing".to_string()]);
        assert_eq!(
            evaluate_checks(ctx, &catalog(), CheckSelection::Only(&selected)).unwrap_err(),
            EvaluationError::UnknownSelection("missing".into())
        );

        let mismatch: Vec<Box<dyn Check>> = vec![Box::new(MismatchedFinding)];
        assert_eq!(
            evaluate_checks(ctx, &mismatch, CheckSelection::All).unwrap_err(),
            EvaluationError::FindingCheckIdMismatch {
                check_id: "parent",
                finding_check_id: "other",
            }
        );
    });
}

#[test]
fn malformed_check_output_returns_a_typed_evaluation_error() {
    struct InvalidOutput;

    impl Check for InvalidOutput {
        fn id(&self) -> &'static str {
            "bad-output"
        }

        fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
            CheckOutput::from_coverage(
                vec![Finding::new(
                    "bad-output",
                    Severity::Error,
                    "unsupported judgment",
                )],
                Vec::new(),
                vec![CoverageGap::new(
                    CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                    "no usable evidence",
                )],
            )
        }
    }

    with_ctx(|ctx| {
        let error = evaluate_checks(ctx, &[Box::new(InvalidOutput)], CheckSelection::All)
            .expect_err("malformed output must not panic or serialize");
        assert_eq!(
            error,
            EvaluationError::InvalidCheckOutput {
                check_id: "bad-output",
                reason: "not-evaluated output cannot carry content findings",
            }
        );
    });
}

#[test]
fn finding_omits_non_finite_optional_numbers_even_after_public_field_mutation() {
    let mut finding = Finding::new("finite-json", Severity::Error, "bad number")
        .time(f32::INFINITY)
        .measured(f64::NAN)
        .expected(f64::NEG_INFINITY);
    assert!(finding.time_s.is_none());
    assert!(finding.measured.is_none());
    assert!(finding.expected.is_none());

    finding.time_s = Some(f32::INFINITY);
    finding.measured = Some(Value::Number(f64::NAN));
    finding.expected = Some(Value::Number(f64::INFINITY));
    let json = serde_json::to_value(&finding).expect("finding serializes");
    assert!(json.get("time_s").is_none());
    assert!(json.get("measured").is_none());
    assert!(json.get("expected").is_none());
}
