use std::collections::{BTreeMap, BTreeSet};

use animsmith_core::check::{Check, CheckCtx};
use animsmith_core::config::{CheckSettings, SeveritySetting};
use animsmith_core::{
    Applicability, BUILTIN_COVERAGE_GAP_CODES, BUILTIN_EVALUATION_SCOPE_CODES, CheckEvaluation,
    CheckOutput, CheckSelection, Config, ConfigurationState, CoverageGap, CoverageGapCode,
    Document, EvaluationError, EvaluationScope, EvaluationScopeCode, EvaluationState, Finding,
    MetricGrids, ResolvedRoles, SelectionState, Severity, Value, evaluate_checks,
    evaluate_checks_v2,
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

struct OptInFindingCheck;

impl Check for OptInFindingCheck {
    fn id(&self) -> &'static str {
        "opt-in-finding"
    }

    fn enabled_by_default(&self) -> bool {
        false
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        CheckOutput::from_coverage(
            vec![Finding::new(
                self.id(),
                Severity::Note,
                "opt-in content signal",
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
            vec![EvaluationScope::new(EvaluationScopeCode::custom(
                "test:member_existence",
            ))],
            vec![
                CoverageGap::new(
                    CoverageGapCode::custom("test:roles_unresolved"),
                    "display text",
                )
                .scope(EvaluationScope::new(EvaluationScopeCode::custom(
                    "test:phase_coherence",
                ))),
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

struct ProtectedCheck {
    applicable: bool,
}

impl Check for ProtectedCheck {
    fn id(&self) -> &'static str {
        "protected"
    }

    fn allows_severity_off(&self) -> bool {
        false
    }

    fn applicability(&self, _ctx: &CheckCtx) -> Applicability {
        if self.applicable {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        panic!("protected check must remain inactive when not applicable or unselected")
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

struct ForeignBuiltin;

impl Check for ForeignBuiltin {
    fn id(&self) -> &'static str {
        "foreign-builtin"
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        CheckOutput::from_coverage(
            Vec::new(),
            Vec::new(),
            vec![CoverageGap::new(
                CoverageGapCode::ROLES_UNRESOLVED,
                "wrong owner",
            )],
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
        assert_eq!(
            records[2].gaps()[0].code,
            CoverageGapCode::custom("test:roles_unresolved")
        );
        assert_eq!(
            records[2].evaluated_scopes()[0].code.as_str(),
            "test:member_existence"
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
fn selected_applicable_protected_check_cannot_use_severity_off_in_both_runners() {
    let doc = Document::default();
    let roles = ResolvedRoles::default();
    let config = Config {
        checks: BTreeMap::from([(
            "protected".to_owned(),
            CheckSettings {
                severity: Some(SeveritySetting::Off),
                ..CheckSettings::default()
            },
        )]),
        ..Config::default()
    };
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let checks: Vec<Box<dyn Check>> = vec![Box::new(ProtectedCheck { applicable: true })];
    let selected = BTreeSet::from(["protected".to_owned()]);

    let v1 = evaluate_checks(&ctx, &checks, CheckSelection::Only(&selected));
    assert!(matches!(
        v1,
        Err(EvaluationError::SeverityOffNotAllowed {
            check_id: "protected"
        })
    ));
    let v2 = evaluate_checks_v2(&ctx, &checks, CheckSelection::Only(&selected));
    assert!(matches!(
        v2,
        Err(EvaluationError::SeverityOffNotAllowed {
            check_id: "protected"
        })
    ));
}

#[test]
fn protected_check_may_be_off_when_unselected_or_not_applicable() {
    let doc = Document::default();
    let roles = ResolvedRoles::default();
    let config = Config {
        checks: BTreeMap::from([(
            "protected".to_owned(),
            CheckSettings {
                severity: Some(SeveritySetting::Off),
                ..CheckSettings::default()
            },
        )]),
        ..Config::default()
    };
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);

    let unselected_checks: Vec<Box<dyn Check>> =
        vec![Box::new(ProtectedCheck { applicable: true })];
    let unselected = evaluate_checks(
        &ctx,
        &unselected_checks,
        CheckSelection::Only(&BTreeSet::new()),
    )
    .unwrap();
    assert_eq!(unselected[0].selection(), SelectionState::Unselected);
    assert_eq!(unselected[0].configuration(), ConfigurationState::Disabled);

    let not_applicable_checks: Vec<Box<dyn Check>> =
        vec![Box::new(ProtectedCheck { applicable: false })];
    let not_applicable = evaluate_checks_v2(
        &ctx,
        &not_applicable_checks,
        CheckSelection::Only(&BTreeSet::from(["protected".to_owned()])),
    )
    .unwrap();
    assert_eq!(
        not_applicable[0].applicability(),
        Applicability::NotApplicable
    );
    assert_eq!(
        not_applicable[0].configuration(),
        ConfigurationState::Disabled
    );
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
    assert_eq!(
        records[2].gaps()[0].code,
        CoverageGapCode::custom("test:roles_unresolved")
    );
}

#[test]
fn opt_in_check_is_disabled_until_an_explicit_severity_enables_it() {
    let doc = Document::default();
    let roles = ResolvedRoles::default();
    let checks: Vec<Box<dyn Check>> = vec![Box::new(OptInFindingCheck)];

    let default_config = Config::default();
    let default_grids = MetricGrids::new(&doc);
    let default_ctx = CheckCtx::new(&default_grids, &roles, &default_config);
    let records = evaluate_checks(&default_ctx, &checks, CheckSelection::All).unwrap();
    assert_eq!(records[0].configuration(), ConfigurationState::Disabled);
    assert_eq!(records[0].evaluation(), EvaluationState::NotEvaluated);
    assert!(records[0].findings().is_empty());

    for (setting, expected) in [
        (SeveritySetting::Note, Severity::Note),
        (SeveritySetting::Warn, Severity::Warning),
        (SeveritySetting::Error, Severity::Error),
    ] {
        let enabled_config = Config {
            checks: BTreeMap::from([(
                "opt-in-finding".to_string(),
                CheckSettings {
                    severity: Some(setting),
                    ..CheckSettings::default()
                },
            )]),
            ..Config::default()
        };
        let enabled_grids = MetricGrids::new(&doc);
        let enabled_ctx = CheckCtx::new(&enabled_grids, &roles, &enabled_config);
        let records = evaluate_checks(&enabled_ctx, &checks, CheckSelection::All).unwrap();
        assert_eq!(records[0].configuration(), ConfigurationState::Enabled);
        assert_eq!(records[0].evaluation(), EvaluationState::Complete);
        assert_eq!(records[0].findings().len(), 1);
        assert_eq!(records[0].findings()[0].severity, expected);
    }
}

#[test]
fn builtin_evidence_codes_reject_undeclared_emitters() {
    const UNDECLARED: &str = "test:undeclared";

    for builtin in BUILTIN_EVALUATION_SCOPE_CODES {
        let code = builtin.clone();
        assert_eq!(
            code,
            builtin.clone(),
            "custom() must preserve built-in scope identity"
        );
        let completed_scope = CheckEvaluation::evaluated(
            UNDECLARED,
            CheckOutput::from_coverage(
                Vec::new(),
                vec![EvaluationScope::new(code.clone())],
                Vec::new(),
            ),
        )
        .expect_err("every built-in completed scope must enforce its declared emitters");
        assert_eq!(
            completed_scope,
            EvaluationError::BuiltinEvaluationScopeEmitterMismatch {
                check_id: UNDECLARED,
                code: code.clone(),
            }
        );

        let gap_scope = CheckEvaluation::evaluated(
            UNDECLARED,
            CheckOutput::from_coverage(
                Vec::new(),
                Vec::new(),
                vec![
                    CoverageGap::new(CoverageGapCode::custom("test:gap"), "gap")
                        .scope(EvaluationScope::new(code.clone())),
                ],
            ),
        )
        .expect_err("every built-in gap scope must enforce its declared emitters");
        assert_eq!(
            gap_scope,
            EvaluationError::BuiltinEvaluationScopeEmitterMismatch {
                check_id: UNDECLARED,
                code,
            }
        );
    }

    for &builtin in BUILTIN_COVERAGE_GAP_CODES {
        let code = CoverageGapCode::custom(builtin.as_str());
        assert_eq!(
            code, builtin,
            "custom() must preserve built-in gap identity"
        );
        let gap = CheckEvaluation::evaluated(
            UNDECLARED,
            CheckOutput::from_coverage(Vec::new(), Vec::new(), vec![CoverageGap::new(code, "gap")]),
        )
        .expect_err("every built-in gap must enforce its declared emitters");
        assert_eq!(
            gap,
            EvaluationError::BuiltinCoverageGapEmitterMismatch {
                check_id: UNDECLARED,
                code,
            }
        );
    }
}

#[test]
fn declared_builtin_emitters_and_namespaced_custom_codes_are_accepted() {
    CheckEvaluation::evaluated(
        "gait-group",
        CheckOutput::from_coverage(
            Vec::new(),
            vec![EvaluationScope::new(EvaluationScopeCode::MEMBER_EXISTENCE)],
            vec![
                CoverageGap::new(CoverageGapCode::ROLES_UNRESOLVED, "missing role")
                    .scope(EvaluationScope::new(EvaluationScopeCode::PHASE_MEASUREMENT)),
            ],
        ),
    )
    .expect("the authority declares gait-group for all emitted built-ins");

    let custom = CheckEvaluation::evaluated(
        "acme-check",
        CheckOutput::from_coverage(
            Vec::new(),
            vec![EvaluationScope::new(EvaluationScopeCode::custom(
                "acme:completed",
            ))],
            vec![
                CoverageGap::new(
                    CoverageGapCode::custom("acme:unavailable"),
                    "custom evidence",
                )
                .scope(EvaluationScope::new(EvaluationScopeCode::custom(
                    "acme:missing",
                ))),
            ],
        ),
    )
    .expect("namespaced custom evidence remains open to embedded checks");

    let json = serde_json::to_value(custom).expect("custom evidence serializes");
    assert_eq!(json["evaluated_scopes"][0]["code"], "acme:completed");
    assert_eq!(json["gaps"][0]["code"], "acme:unavailable");
    assert_eq!(json["gaps"][0]["scope"]["code"], "acme:missing");

    for (check_id, scope_code, gap_code) in [
        ("vendor-check", "vendor:completed", "vendor:unavailable"),
        (
            "reverse-domain-check",
            "org.example:completed",
            "org.example:unavailable",
        ),
    ] {
        CheckEvaluation::evaluated(
            check_id,
            CheckOutput::from_coverage(
                Vec::new(),
                vec![EvaluationScope::new(EvaluationScopeCode::custom(
                    scope_code,
                ))],
                vec![CoverageGap::new(CoverageGapCode::custom(gap_code), "gap")],
            ),
        )
        .expect("custom constructors must remain open to namespaced values");
    }
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

        let foreign_builtin: Vec<Box<dyn Check>> = vec![Box::new(ForeignBuiltin)];
        assert_eq!(
            evaluate_checks(ctx, &foreign_builtin, CheckSelection::All).unwrap_err(),
            EvaluationError::BuiltinCoverageGapEmitterMismatch {
                check_id: "foreign-builtin",
                code: CoverageGapCode::ROLES_UNRESOLVED,
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
                    CoverageGapCode::custom("test:measurement_unavailable"),
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
fn empty_contract_identifiers_return_typed_errors() {
    let empty = CheckEvaluation::evaluated(
        "",
        CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new()),
    )
    .expect_err("an empty parent id must be rejected");
    assert_eq!(empty, EvaluationError::InvalidCheckId(""));

    let cases = [
        (
            CheckOutput::from_coverage(
                Vec::new(),
                vec![EvaluationScope::new(EvaluationScopeCode::custom(""))],
                Vec::new(),
            ),
            "evaluated scope code cannot be empty",
        ),
        (
            CheckOutput::from_coverage(
                Vec::new(),
                Vec::new(),
                vec![CoverageGap::new(CoverageGapCode::custom(""), "gap")],
            ),
            "coverage gap code cannot be empty",
        ),
        (
            CheckOutput::from_coverage(
                Vec::new(),
                Vec::new(),
                vec![
                    CoverageGap::new(CoverageGapCode::custom("test:gap"), "gap")
                        .scope(EvaluationScope::new(EvaluationScopeCode::custom(""))),
                ],
            ),
            "coverage gap scope code cannot be empty",
        ),
    ];
    for (output, reason) in cases {
        assert_eq!(
            CheckEvaluation::evaluated("custom", output)
                .expect_err("empty evidence codes must be rejected"),
            EvaluationError::InvalidCheckOutput {
                check_id: "custom",
                reason,
            }
        );
    }

    struct EmptyId;
    impl Check for EmptyId {
        fn id(&self) -> &'static str {
            ""
        }

        fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
            panic!("invalid catalog ids must fail before evaluation")
        }
    }
    with_ctx(|ctx| {
        assert_eq!(
            evaluate_checks(ctx, &[Box::new(EmptyId)], CheckSelection::All)
                .expect_err("empty catalog id must be rejected"),
            EvaluationError::InvalidCheckId("")
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
