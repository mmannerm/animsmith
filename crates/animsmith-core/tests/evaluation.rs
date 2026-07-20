use std::collections::{BTreeMap, BTreeSet};

use animsmith_core::check::{Check, CheckCtx, Readiness};
use animsmith_core::config::{CheckSettings, SeveritySetting};
use animsmith_core::{
    Applicability, CheckOutput, CheckSelection, Config, ConfigurationState, CoverageGap, Document,
    EvaluationScope, EvaluationState, Finding, MetricGrids, ResolvedRoles, SelectionState,
    Severity, evaluate_checks,
};

struct Complete;

impl Check for Complete {
    fn id(&self) -> &'static str {
        "complete"
    }

    fn run(&self, _ctx: &CheckCtx, _out: &mut Vec<Finding>) {}
}

struct FindingCheck;

impl Check for FindingCheck {
    fn id(&self) -> &'static str {
        "finding"
    }

    fn run(&self, _ctx: &CheckCtx, out: &mut Vec<Finding>) {
        out.push(Finding::new(
            self.id(),
            Severity::Warning,
            "content warning",
        ));
    }
}

struct Partial;

impl Check for Partial {
    fn id(&self) -> &'static str {
        "partial"
    }

    fn run(&self, _ctx: &CheckCtx, _out: &mut Vec<Finding>) {}

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        CheckOutput {
            findings: vec![Finding::new(self.id(), Severity::Error, "member missing")],
            evaluated_scopes: vec![EvaluationScope::new("member_existence")],
            gaps: vec![
                CoverageGap::new("roles_unresolved", "display text")
                    .scope(EvaluationScope::new("phase_coherence")),
            ],
        }
    }
}

struct Blocked;

impl Check for Blocked {
    fn id(&self) -> &'static str {
        "blocked"
    }

    fn readiness(&self, _ctx: &CheckCtx) -> Readiness {
        Readiness::Skipped("arbitrary display message".to_string())
    }

    fn run(&self, _ctx: &CheckCtx, _out: &mut Vec<Finding>) {
        panic!("blocked check must not execute")
    }
}

struct ReadyButUnevaluated;

impl Check for ReadyButUnevaluated {
    fn id(&self) -> &'static str {
        "ready-but-unevaluated"
    }

    fn run(&self, _ctx: &CheckCtx, _out: &mut Vec<Finding>) {
        panic!("typed evaluation must not fall back to run")
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        CheckOutput {
            findings: Vec::new(),
            evaluated_scopes: Vec::new(),
            gaps: vec![CoverageGap::new("input_unavailable", "nothing evaluated")],
        }
    }
}

struct DiagnosticCheck;

impl Check for DiagnosticCheck {
    fn id(&self) -> &'static str {
        "diagnostic"
    }

    fn run(&self, _ctx: &CheckCtx, out: &mut Vec<Finding>) {
        out.push(
            Finding::new(self.id(), Severity::Note, "prerequisite unavailable").as_diagnostic(),
        );
    }
}

struct TypedDiagnosticCheck;

impl Check for TypedDiagnosticCheck {
    fn id(&self) -> &'static str {
        "typed-diagnostic"
    }

    fn run(&self, _ctx: &CheckCtx, _out: &mut Vec<Finding>) {
        panic!("typed evaluator must not fall back to run")
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        CheckOutput::complete(vec![
            Finding::new(
                self.id(),
                Severity::Warning,
                "typed prerequisite unavailable",
            )
            .as_diagnostic(),
        ])
    }
}

struct MixedDiagnosticCheck;

impl Check for MixedDiagnosticCheck {
    fn id(&self) -> &'static str {
        "mixed-diagnostic"
    }

    fn run(&self, _ctx: &CheckCtx, out: &mut Vec<Finding>) {
        out.push(Finding::new(
            self.id(),
            Severity::Warning,
            "content warning",
        ));
        out.push(Finding::new(self.id(), Severity::Note, "some work unavailable").as_diagnostic());
    }
}

struct PoisonCheck {
    id: &'static str,
    idle: bool,
}

impl Check for PoisonCheck {
    fn id(&self) -> &'static str {
        self.id
    }

    fn readiness(&self, _ctx: &CheckCtx) -> Readiness {
        if self.idle {
            Readiness::Idle
        } else {
            Readiness::Ready
        }
    }

    fn run(&self, _ctx: &CheckCtx, _out: &mut Vec<Finding>) {
        panic!("inactive check {} must not run", self.id)
    }

    fn evaluate(&self, _ctx: &CheckCtx) -> CheckOutput {
        panic!("inactive check {} must not evaluate", self.id)
    }
}

struct Idle;

impl Check for Idle {
    fn id(&self) -> &'static str {
        "idle"
    }

    fn readiness(&self, _ctx: &CheckCtx) -> Readiness {
        Readiness::Idle
    }

    fn run(&self, _ctx: &CheckCtx, _out: &mut Vec<Finding>) {
        panic!("not-applicable check must not execute")
    }
}

fn catalog() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(Complete),
        Box::new(FindingCheck),
        Box::new(Partial),
        Box::new(Blocked),
        Box::new(Idle),
        Box::new(ReadyButUnevaluated),
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
fn records_complete_findings_partial_blocked_and_not_applicable_without_messages() {
    with_ctx(|ctx| {
        let records = evaluate_checks(ctx, &catalog(), CheckSelection::All);
        assert_eq!(records.len(), 6);

        let complete = &records[0];
        assert_eq!(complete.evaluation, EvaluationState::Complete);
        assert!(complete.findings.is_empty());

        let finding = &records[1];
        assert_eq!(finding.evaluation, EvaluationState::Complete);
        assert_eq!(finding.findings.len(), 1);

        let partial = &records[2];
        assert_eq!(partial.evaluation, EvaluationState::Partial);
        assert_eq!(partial.findings.len(), 1);
        assert_eq!(partial.gaps[0].code, "roles_unresolved");
        assert_eq!(partial.evaluated_scopes[0].code, "member_existence");

        let blocked = &records[3];
        assert_eq!(blocked.applicability, Applicability::Applicable);
        assert_eq!(blocked.evaluation, EvaluationState::NotEvaluated);
        assert_eq!(blocked.gaps[0].code, "roles_unresolved");
        assert_eq!(blocked.gaps[0].message, "arbitrary display message");

        let idle = &records[4];
        assert_eq!(idle.applicability, Applicability::NotApplicable);
        assert_eq!(idle.evaluation, EvaluationState::NotEvaluated);
        assert!(idle.gaps.is_empty());

        let ready_but_unevaluated = &records[5];
        assert_eq!(
            ready_but_unevaluated.applicability,
            Applicability::Applicable
        );
        assert_eq!(
            ready_but_unevaluated.evaluation,
            EvaluationState::NotEvaluated
        );
        assert_eq!(ready_but_unevaluated.gaps[0].code, "input_unavailable");
    });
}

#[test]
fn disabled_and_unselected_are_independent_and_never_execute() {
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
    let poison_catalog: Vec<Box<dyn Check>> = vec![
        Box::new(PoisonCheck {
            id: "unselected-ready",
            idle: false,
        }),
        Box::new(PoisonCheck {
            id: "disabled",
            idle: false,
        }),
        Box::new(PoisonCheck {
            id: "unselected-idle",
            idle: true,
        }),
    ];
    let records = evaluate_checks(&ctx, &poison_catalog, CheckSelection::Only(&selected));

    assert_eq!(records[0].selection, SelectionState::Unselected);
    assert_eq!(records[0].configuration, ConfigurationState::Enabled);
    assert_eq!(records[0].applicability, Applicability::Applicable);
    assert_eq!(records[0].evaluation, EvaluationState::NotEvaluated);
    assert_eq!(records[1].selection, SelectionState::Selected);
    assert_eq!(records[1].configuration, ConfigurationState::Disabled);
    assert_eq!(records[1].applicability, Applicability::Applicable);
    assert!(records[1].findings.is_empty());
    assert_eq!(records[2].selection, SelectionState::Unselected);
    assert_eq!(records[2].configuration, ConfigurationState::Enabled);
    assert_eq!(records[2].applicability, Applicability::NotApplicable);
    assert_eq!(records[2].evaluation, EvaluationState::NotEvaluated);
}

#[test]
fn legacy_diagnostics_never_become_v2_content_findings() {
    let doc = Document::default();
    let roles = ResolvedRoles::default();
    let config = Config {
        checks: BTreeMap::from([
            (
                "diagnostic".to_string(),
                CheckSettings {
                    severity: Some(SeveritySetting::Error),
                    ..CheckSettings::default()
                },
            ),
            (
                "typed-diagnostic".to_string(),
                CheckSettings {
                    severity: Some(SeveritySetting::Error),
                    ..CheckSettings::default()
                },
            ),
            (
                "mixed-diagnostic".to_string(),
                CheckSettings {
                    severity: Some(SeveritySetting::Error),
                    ..CheckSettings::default()
                },
            ),
        ]),
        ..Config::default()
    };
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let checks: Vec<Box<dyn Check>> = vec![
        Box::new(DiagnosticCheck),
        Box::new(TypedDiagnosticCheck),
        Box::new(MixedDiagnosticCheck),
    ];
    let records = evaluate_checks(&ctx, &checks, CheckSelection::All);

    for (record, expected_message) in records[..2]
        .iter()
        .zip(["prerequisite unavailable", "typed prerequisite unavailable"])
    {
        assert_eq!(record.evaluation, EvaluationState::NotEvaluated);
        assert!(record.findings.is_empty());
        assert_eq!(record.gaps.len(), 1);
        assert_eq!(record.gaps[0].code, "legacy_diagnostic");
        assert_eq!(record.gaps[0].message, expected_message);
    }
    let mixed = &records[2];
    assert_eq!(mixed.evaluation, EvaluationState::Partial);
    assert_eq!(mixed.findings.len(), 1);
    assert_eq!(mixed.findings[0].severity, Severity::Error);
    assert_eq!(mixed.findings[0].message, "content warning");
    assert_eq!(mixed.gaps[0].code, "legacy_diagnostic");
    assert_eq!(mixed.gaps[0].message, "some work unavailable");
}

#[test]
fn severity_override_changes_content_finding_but_not_gap_typing() {
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
    let records = evaluate_checks(&ctx, &catalog(), CheckSelection::All);
    let partial = &records[2];

    assert_eq!(partial.findings[0].severity, Severity::Note);
    assert_eq!(partial.gaps[0].code, "roles_unresolved");
    assert_eq!(partial.evaluation, EvaluationState::Partial);
}
