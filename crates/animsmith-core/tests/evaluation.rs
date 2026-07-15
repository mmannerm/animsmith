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
        Readiness::Skipped(CoverageGap::new(
            "roles_unresolved",
            "arbitrary display message",
        ))
    }

    fn run(&self, _ctx: &CheckCtx, _out: &mut Vec<Finding>) {
        panic!("blocked check must not execute")
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
        assert_eq!(records.len(), 5);

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

        let idle = &records[4];
        assert_eq!(idle.applicability, Applicability::NotApplicable);
        assert_eq!(idle.evaluation, EvaluationState::NotEvaluated);
        assert!(idle.gaps.is_empty());
    });
}

#[test]
fn disabled_and_unselected_are_independent_and_never_execute() {
    let doc = Document::default();
    let roles = ResolvedRoles::default();
    let config = Config {
        checks: BTreeMap::from([(
            "finding".to_string(),
            CheckSettings {
                severity: Some(SeveritySetting::Off),
                ..CheckSettings::default()
            },
        )]),
        ..Config::default()
    };
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let selected = BTreeSet::from(["finding".to_string()]);
    let records = evaluate_checks(&ctx, &catalog(), CheckSelection::Only(&selected));

    assert_eq!(records[0].selection, SelectionState::Unselected);
    assert_eq!(records[0].configuration, ConfigurationState::Enabled);
    assert_eq!(records[1].selection, SelectionState::Selected);
    assert_eq!(records[1].configuration, ConfigurationState::Disabled);
    assert!(records[1].findings.is_empty());
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
