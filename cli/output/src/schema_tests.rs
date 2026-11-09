use crate::*;
use serde_json::{Value, json};

const SCHEMA: &str = include_str!("../schema/events.schema.json");
const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn schema() -> Value {
    serde_json::from_str(SCHEMA).expect("schema is JSON")
}

#[test]
fn schema_defines_exactly_the_registered_events() {
    let schema = schema();
    let defs = schema["$defs"].as_object().expect("$defs");
    let mut defined: Vec<&str> = defs
        .keys()
        .map(String::as_str)
        .filter(|name| !matches!(*name, "schema" | "finished"))
        .collect();
    defined.sort_unstable();
    let mut registered = EVENTS.to_vec();
    registered.sort_unstable();
    assert_eq!(defined, registered);
}

#[test]
fn schema_pins_the_current_version() {
    let version = &schema()["$defs"]["schema"]["properties"];
    assert_eq!(version["major"]["const"], SCHEMA_MAJOR);
    assert_eq!(version["minor"]["const"], SCHEMA_MINOR);
}

fn started() -> Value {
    command_started("check", "default").expect("started")
}

fn finished() -> Value {
    let counts = FinishedCounts {
        results_complete: Some(true),
        run_id: Some("r1".to_owned()),
        diagnostics: Some([1, 2, 3]),
        findings: None,
        changes: Some([1, 2]),
        effects: Some([1, 0]),
        tests: Some(TestCounts::default()),
        actions: Some(ActionCounts::default()),
    };
    command_finished(0, &counts)
}

fn diagnostic() -> DiagnosticEvent {
    let mut d = DiagnosticEvent::new(Severity::Warning, "tool", "message");
    d.path = Some("a.rs".to_owned());
    d.range = Some((1, 2));
    d
}

fn full_diagnostic() -> Value {
    let mut d = diagnostic();
    d.rule = Some("rule".to_owned());
    d.unfixable_reason = Some("why".to_owned());
    d.origins = vec![Origin {
        producer: "p".to_owned(),
        target: "//a:b".to_owned(),
        configuration: None,
    }];
    d.package = Some(PackageRef {
        name: "n".to_owned(),
        version: "1".to_owned(),
        fixed: Some(vec!["2".to_owned()]),
        license: Some("MIT".to_owned()),
    });
    d.related = vec![RelatedEvent {
        role: "fix".to_owned(),
        message: "m".to_owned(),
        path: Some("a.rs".to_owned()),
        range: Some((1, 2)),
        replacement: Some("x".to_owned()),
        applicability: Some("safe".to_owned()),
    }];
    let mut record = FindingRecord::single(d);
    record.span = Some((
        Position { line: 1, column: 1 },
        Position { line: 1, column: 2 },
    ));
    finding_value(&record)
}

fn slot(full: bool) -> SlotEvent {
    SlotEvent {
        action: "add".to_owned(),
        path: "/w".to_owned(),
        slot: full.then_some(1),
        branch: full.then(|| "b".to_owned()),
        holder: full.then(|| "h".to_owned()),
        state: "held".to_owned(),
        reason: full.then(|| "r".to_owned()),
        base: full.then(|| "origin/main".to_owned()),
        server: false,
        disk_bytes: full.then_some(5),
    }
}

fn coverage(full: bool) -> CoverageEvent {
    CoverageEvent {
        cell: "c".to_owned(),
        covered_lines: 1,
        eligible_lines: 2,
        raw_lines: 3,
        excluded_lines: 0,
        eligible_files: 1,
        support_files: 0,
        excluded_files: 0,
        unsupported_files: 0,
        minimum: full.then_some(80),
        passed: true,
        errors: 0,
        dx_version: "1".to_owned(),
        sha: full.then(|| "abc".to_owned()),
        dirty: full,
        collector: full.then(|| "bazel 8".to_owned()),
    }
}

fn outcome(full: bool) -> TestOutcome {
    let mut t = TestOutcome {
        target: "//a:t".to_owned(),
        outcome: "passed".to_owned(),
        evidence_complete: true,
        ..TestOutcome::default()
    };
    if full {
        t.configuration = Some("c".to_owned());
        t.status = Some("PASSED".to_owned());
        t.cached = Some(true);
        t.run = Some(1);
        t.shard = Some(1);
        t.attempt = Some(1);
        t.superseded = true;
        t.duration_millis = Some(5);
        t.cases_total = Some(2);
        t.cases_failed = Some(1);
        t.cases_error = Some(1);
        t.cases_skipped = Some(1);
        t.artifacts_missing = 1;
        t.artifacts_invalid = 1;
        t.cases_omitted = 1;
        t.evidence_complete = false;
        t.log_path = Some("l".to_owned());
        t.log_tail = Some("tail".to_owned());
        t.outputs_path = Some("o".to_owned());
        t.cases = vec![TestCase {
            classname: Some("C".to_owned()),
            name: "n".to_owned(),
            outcome: "failed".to_owned(),
            message: Some("m".to_owned()),
            file: Some("f.rs".to_owned()),
            line: Some(3),
        }];
    }
    t
}

fn failure(full: bool) -> BuildFailureEvent {
    let related = FailureRelated {
        role: "r".to_owned(),
        message: "m".to_owned(),
        path: Some("a".to_owned()),
        line: Some(1),
        column: Some(1),
        replacement: Some("x".to_owned()),
        applicability: Some("safe".to_owned()),
    };
    BuildFailureEvent {
        target: full.then(|| "//a:b".to_owned()),
        configuration: full.then(|| "c".to_owned()),
        target_kind: "rust_library".to_owned(),
        stage: "build".to_owned(),
        category: "compile".to_owned(),
        mnemonic: full.then(|| "Rustc".to_owned()),
        primary_output: full.then(|| "out".to_owned()),
        exit_code: full.then_some(1),
        tool: full.then(|| "rustc".to_owned()),
        message: full.then(|| "m".to_owned()),
        diagnostics_status: "complete".to_owned(),
        diagnostics_total: 1,
        diagnostics: vec![FailureDiagnostic {
            severity: "error".to_owned(),
            code: full.then(|| "E1".to_owned()),
            message: "m".to_owned(),
            path: full.then(|| "a".to_owned()),
            line: full.then_some(1),
            column: full.then_some(1),
            related: if full { vec![related] } else { Vec::new() },
        }],
        count: if full { 2 } else { 1 },
        targets: if full {
            vec!["//a:b".to_owned()]
        } else {
            Vec::new()
        },
    }
}

fn samples() -> Vec<Value> {
    let facts = FailureFacts {
        cause: "exit",
        status: Some(2),
        excerpt: Some(&Excerpt {
            text: "t".to_owned(),
            truncated: true,
        }),
        next: Some("run it again"),
    };
    let capability = CapabilityEvent {
        name: "check".to_owned(),
        source: "cli-grammar".to_owned(),
        availability: "available".to_owned(),
        detail: "d".to_owned(),
        flags: vec!["--x".to_owned()],
        outputs: vec!["json".to_owned()],
        reports: vec!["sarif".to_owned()],
        scope_policy: "p".to_owned(),
        effect: "e".to_owned(),
        passthrough: false,
    };
    let change = ChangeEvent {
        path: "a.txt".to_owned(),
        kind: ChangeKind::Modify,
        source_digest: Some(DIGEST.to_owned()),
        edits: vec![Edit {
            start: 0,
            end: 1,
            replacement: "x".to_owned(),
        }],
    };
    let gate = ChangedLineCoverageEvent {
        base: "main".to_owned(),
        merge_base: "abc".to_owned(),
        minimum: 80,
        covered_lines: 1,
        eligible_lines: 2,
        uncovered: vec![("a.rs".to_owned(), 3)],
        uninstrumented: vec!["b.rs".to_owned()],
        result: "fail".to_owned(),
        dx_version: "1".to_owned(),
    };
    let mut notice = NoticeEvent::info("code", "message");
    notice.related_command = Some("check".to_owned());
    notice.scope = Some(vec!["//a".to_owned()]);
    notice.path = Some("a".to_owned());
    notice.language = Some("rust".to_owned());
    notice.import = Some("x".to_owned());
    let plan = PlanFindingEvent {
        kind: "k".to_owned(),
        subject: "s".to_owned(),
        before: "b".to_owned(),
        after: "a".to_owned(),
        file: "f".to_owned(),
        line: 3,
        hint: "h".to_owned(),
        strength: "hard".to_owned(),
    };
    let phase = |event: Value| nest_phase_event(event, "check", "lint", "c1").expect("nest");
    vec![
        build_failure_event(&failure(false)).expect("failure"),
        build_failure_event(&failure(true)).expect("failure"),
        capability_event(&capability).expect("capability"),
        change_event(&change, ChangeEffect::Applied, None, true).expect("change"),
        change_event(&change, ChangeEffect::Refused, Some("why"), false).expect("change"),
        changed_line_coverage_event(&gate).expect("gate"),
        finished(),
        command_finished(1, &FinishedCounts::default()),
        started(),
        coverage_event(&coverage(false)).expect("coverage"),
        coverage_event(&coverage(true)).expect("coverage"),
        diagnostic_event(&diagnostic()).expect("diagnostic"),
        full_diagnostic(),
        error_event("code", "m", None, None, None).expect("error"),
        error_event("code", "m", Some("a"), Some("--f"), Some("p")).expect("error"),
        generation_error_event(
            "code",
            "m",
            Some("a"),
            Some("p"),
            &GenerationDetail {
                language: "py",
                kind: Some("unresolved_import"),
                import: Some("x"),
                fix: Some(("a/BUILD.bazel", "d")),
            },
        )
        .expect("error"),
        failure_event("code", "m", Some("p"), &facts).expect("failure"),
        init_plan_event(&plan).expect("plan"),
        notice_event(&NoticeEvent::warning("code", "message")).expect("notice"),
        notice_value(&notice),
        operation_event("check", "lint", None).expect("operation"),
        owned_file_event(&OwnedFileEvent {
            path: ".gitignore".to_owned(),
            owns: "lines".to_owned(),
            state: "current".to_owned(),
        })
        .expect("owned file"),
        operation_event("check", "lint", Some(&["//a".to_owned()])).expect("operation"),
        phase(started()),
        phase(finished()),
        report_event("sarif", "r.sarif", true).expect("report"),
        selection_event(DIGEST, DIGEST, DIGEST).expect("selection"),
        slot_event(&slot(false)).expect("slot"),
        slot_event(&slot(true)).expect("slot"),
        status_event(&StatusEvent {
            name: "n".to_owned(),
            severity: "info".to_owned(),
            detail: "d".to_owned(),
            hint: "h".to_owned(),
        })
        .expect("status"),
        summary_event(&SummaryEvent {
            slots: 1,
            servers: 1,
            servers_rss_bytes: 1,
            disk_bytes: Some(1),
            disk_cache_bytes: Some(1),
        }),
        test_outcome_event(&outcome(false)).expect("outcome"),
        test_outcome_event(&outcome(true)).expect("outcome"),
        tier_event(&TierEvent {
            family: None,
            tier: "strict".to_owned(),
            source: "dx.toml".to_owned(),
        })
        .expect("tier"),
        tier_event(&TierEvent {
            family: Some("python".to_owned()),
            tier: "standard".to_owned(),
            source: "dx.toml".to_owned(),
        })
        .expect("family tier"),
        config_file_event(&ConfigFileEvent {
            tool: "ruff".to_owned(),
            file: "ruff.toml".to_owned(),
            role: "root".to_owned(),
            base: None,
            edited: None,
        })
        .expect("bound config file"),
        family_tools_event(&FamilyToolsEvent {
            family: "python".to_owned(),
            capability: "lint".to_owned(),
            tools: vec!["ruff".to_owned()],
        })
        .expect("family tools"),
        config_file_event(&ConfigFileEvent {
            tool: "hlint".to_owned(),
            file: ".hlint.yaml".to_owned(),
            role: "root".to_owned(),
            base: Some(".dx/config/hlint/base.yaml".to_owned()),
            edited: Some(true),
        })
        .expect("config file"),
        with_correlation(started(), "c1").expect("correlation"),
    ]
}

#[test]
fn every_event_sample_matches_the_schema() {
    let validator = jsonschema::draft202012::new(&schema()).expect("schema compiles");
    let mut seen = Vec::new();
    for sample in samples() {
        let name = sample["event"].as_str().expect("event").to_owned();
        let errors: Vec<String> = validator
            .iter_errors(&sample)
            .map(|error| format!("{}: {error}", error.instance_path()))
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:?}\n{sample}");
        seen.push(name);
    }
    for name in EVENTS {
        assert!(seen.iter().any(|seen| seen == name), "no sample for {name}");
    }
}

#[test]
fn an_unknown_field_fails_the_schema() {
    let validator = jsonschema::draft202012::new(&schema()).expect("schema compiles");
    let mut event = started();
    event["extra"] = json!(1);
    assert!(!validator.is_valid(&event));
}

#[test]
fn an_unknown_field_on_a_finished_event_fails_the_schema() {
    let validator = jsonschema::draft202012::new(&schema()).expect("schema compiles");
    let phase = nest_phase_event(finished(), "check", "resolve", "c1").expect("phase_finished");
    assert_eq!(phase["event"], "phase_finished");
    assert!(validator.is_valid(&phase), "{phase}");
    let mut extra = phase;
    extra["extra"] = json!(1);
    assert!(!validator.is_valid(&extra));
    let mut command = finished();
    command["extra"] = json!(1);
    assert!(!validator.is_valid(&command));
}

#[test]
fn command_finished_requires_a_status() {
    let validator = jsonschema::draft202012::new(&schema()).expect("schema compiles");
    let mut event = finished();
    assert_eq!(event["status"], "ok");
    assert!(validator.is_valid(&event));
    event["status"] = json!("done");
    assert!(!validator.is_valid(&event));
    event.as_object_mut().expect("object").remove("status");
    assert!(!validator.is_valid(&event));
}
