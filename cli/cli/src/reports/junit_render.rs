use super::junit_types::{JunitCase, JunitMessage};

fn junit_display_name(case: &JunitCase, show_run: bool, show_configuration: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    if show_run {
        parts.push(format!("run={}", case.run));
    }
    if show_configuration {
        parts.push(format!(
            "config={}",
            case.configuration.as_deref().unwrap_or_default()
        ));
    }
    if case.shard != 0 || case.attempt != 0 {
        parts.push(format!("shard={},attempt={}", case.shard, case.attempt));
    }
    if parts.is_empty() {
        case.name.clone()
    } else {
        format!("{} [{}]", case.name, parts.join(","))
    }
}

fn junit_duration(time: f64) -> std::time::Duration {
    if time.is_finite() && time >= 0.0 {
        std::time::Duration::try_from_secs_f64(time).unwrap_or(std::time::Duration::ZERO)
    } else {
        std::time::Duration::ZERO
    }
}

fn junit_status(case: &JunitCase) -> quick_junit::TestCaseStatus {
    if let Some(failure) = &case.failure {
        let mut status =
            quick_junit::TestCaseStatus::non_success(quick_junit::NonSuccessKind::Failure);
        if let Some(note) = &failure.message {
            status.set_message(note.as_str());
        }
        if !failure.text.is_empty() {
            status.set_description(failure.text.as_str());
        }
        status
    } else if let Some(error) = &case.error {
        let mut status =
            quick_junit::TestCaseStatus::non_success(quick_junit::NonSuccessKind::Error);
        if let Some(note) = &error.message {
            status.set_message(note.as_str());
        }
        if !error.text.is_empty() {
            status.set_description(error.text.as_str());
        }
        status
    } else if let Some(skipped) = &case.skipped {
        let mut status = quick_junit::TestCaseStatus::skipped();
        if let Some(note) = &skipped.message {
            status.set_message(note.as_str());
        }
        if !skipped.text.is_empty() {
            status.set_description(skipped.text.as_str());
        }
        status
    } else {
        quick_junit::TestCaseStatus::success()
    }
}

pub fn render_junit(suites: &[(String, Vec<JunitCase>)]) -> Result<String, super::ReportError> {
    let mut ordered: Vec<(String, Vec<JunitCase>)> = suites.to_vec();
    for (_, cases) in &mut ordered {
        cases.sort_by(|a, b| {
            a.name
                .as_bytes()
                .cmp(b.name.as_bytes())
                .then(a.configuration.cmp(&b.configuration))
                .then(a.run.cmp(&b.run))
                .then(a.shard.cmp(&b.shard))
                .then(a.attempt.cmp(&b.attempt))
        });
    }
    ordered.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

    let mut report = quick_junit::Report::new("dx");
    let mut total_time = std::time::Duration::ZERO;
    for (label, cases) in &ordered {
        let mut suite = quick_junit::TestSuite::new(label.as_str());
        let mut suite_time = std::time::Duration::ZERO;
        let show_run = cases.iter().any(|case| case.run != cases[0].run);
        let show_configuration = cases
            .iter()
            .any(|case| case.configuration != cases[0].configuration);
        for case in cases {
            let display = junit_display_name(case, show_run, show_configuration);
            let mut test_case = quick_junit::TestCase::new(display, junit_status(case));
            if let Some(classname) = &case.classname {
                test_case.set_classname(classname.as_str());
            }
            let duration = junit_duration(case.time);
            test_case.set_time(duration);
            suite_time += duration;
            if let Some(out) = &case.system_out {
                test_case.set_system_out(out.as_str());
            }
            if let Some(err_text) = &case.system_err {
                test_case.set_system_err(err_text.as_str());
            }
            suite.add_test_case(test_case);
        }
        suite.set_time(suite_time);
        total_time += suite_time;
        report.add_test_suite(suite);
    }
    report.set_time(total_time);
    report
        .to_string()
        .map_err(|err| super::ReportError::JunitRender {
            detail: err.to_string(),
        })
}

pub fn junit_infrastructure_case(detail: &str) -> (String, Vec<JunitCase>) {
    (
        "dx.infrastructure".to_owned(),
        vec![JunitCase {
            name: "incomplete_results".to_owned(),
            classname: Some("dx.infrastructure".to_owned()),
            time: 0.0,
            failure: None,
            error: Some(JunitMessage {
                message: Some("incomplete_results".to_owned()),
                text: detail.to_owned(),
            }),
            skipped: None,
            system_out: None,
            system_err: None,
            shard: 0,
            attempt: 0,
            run: 0,
            configuration: None,
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::super::junit_parse::parse_test_xml;
    use super::*;

    fn junit_case(name: &str) -> JunitCase {
        JunitCase {
            name: name.to_owned(),
            classname: None,
            time: 0.0,
            failure: None,
            error: None,
            skipped: None,
            system_out: None,
            system_err: None,
            shard: 0,
            attempt: 0,
            run: 0,
            configuration: None,
        }
    }

    #[test]
    fn junit_helpers_cover_escapes_times_and_names() {
        assert_eq!(junit_duration(f64::NAN), std::time::Duration::ZERO);
        assert_eq!(junit_duration(-1.0), std::time::Duration::ZERO);
        assert_eq!(junit_duration(f64::INFINITY), std::time::Duration::ZERO);
        assert_eq!(
            junit_duration(1.23456),
            std::time::Duration::try_from_secs_f64(1.23456).expect("duration")
        );
        assert_eq!(junit_display_name(&junit_case("a"), false, false), "a");
        let retried = JunitCase {
            shard: 1,
            attempt: 2,
            ..junit_case("a")
        };
        assert_eq!(
            junit_display_name(&retried, false, false),
            "a [shard=1,attempt=2]"
        );
        let full = JunitCase {
            run: 2,
            configuration: Some("cfg1".to_owned()),
            ..retried
        };
        assert_eq!(
            junit_display_name(&full, true, true),
            "a [run=2,config=cfg1,shard=1,attempt=2]"
        );
        let doc = render_junit(&[(
            "a&<b>\"'".to_owned(),
            vec![JunitCase {
                name: "n&<m>\"'".to_owned(),
                classname: Some("c&<>".to_owned()),
                time: 1.0,
                failure: Some(JunitMessage {
                    message: Some("msg&<>".to_owned()),
                    text: "body&<>".to_owned(),
                }),
                error: None,
                skipped: None,
                system_out: None,
                system_err: None,
                shard: 0,
                attempt: 0,
                run: 0,
                configuration: None,
            }],
        )])
        .expect("render junit");
        assert!(doc.contains("&amp;"), "{doc}");
        assert!(doc.contains("&lt;"), "{doc}");
        let doc = render_junit(&[(
            "s".to_owned(),
            vec![JunitCase {
                failure: Some(JunitMessage {
                    message: Some("f".to_owned()),
                    text: "ft".to_owned(),
                }),
                error: Some(JunitMessage {
                    message: Some("e".to_owned()),
                    text: "et".to_owned(),
                }),
                ..junit_case("both")
            }],
        )])
        .expect("render junit");
        assert!(doc.contains("<failure"), "{doc}");
        let (_label, cases) = junit_infrastructure_case("detail");
        assert_eq!(cases[0].name, "incomplete_results");
    }

    #[test]
    fn junit_render_covers_all_child_kinds() {
        let suites = vec![
            ("//b:t".to_owned(), vec![junit_case("b")]),
            (
                "//a:t".to_owned(),
                vec![
                    JunitCase {
                        failure: Some(JunitMessage {
                            message: Some("f".to_owned()),
                            text: "ft".to_owned(),
                        }),
                        ..junit_case("a1")
                    },
                    JunitCase {
                        error: Some(JunitMessage {
                            message: None,
                            text: String::new(),
                        }),
                        system_out: Some("out".to_owned()),
                        system_err: Some("err".to_owned()),
                        shard: 1,
                        attempt: 2,
                        ..junit_case("a2")
                    },
                    JunitCase {
                        skipped: Some(JunitMessage {
                            message: None,
                            text: String::new(),
                        }),
                        ..junit_case("a3")
                    },
                ],
            ),
        ];
        let doc = render_junit(&suites).expect("render junit");
        insta::assert_snapshot!(doc, @r#"
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="dx" tests="4" skipped="1" failures="1" errors="1" time="0.000">
    <testsuite name="//a:t" tests="3" skipped="1" errors="1" failures="1" time="0.000">
        <testcase name="a1" time="0.000">
            <failure message="f">ft</failure>
        </testcase>
        <testcase name="a2 [shard=1,attempt=2]" time="0.000">
            <error/>
            <system-out>out</system-out>
            <system-err>err</system-err>
        </testcase>
        <testcase name="a3" time="0.000">
            <skipped/>
        </testcase>
    </testsuite>
    <testsuite name="//b:t" tests="1" skipped="0" errors="0" failures="0" time="0.000">
        <testcase name="b" time="0.000"/>
    </testsuite>
</testsuites>"#);
    }

    #[test]
    fn runs_and_configurations_get_distinct_case_names() {
        let doc = render_junit(&[
            (
                "//runs:t".to_owned(),
                vec![
                    JunitCase {
                        run: 1,
                        ..junit_case("a")
                    },
                    junit_case("a"),
                ],
            ),
            (
                "//configs:t".to_owned(),
                vec![
                    JunitCase {
                        configuration: Some("cfg2".to_owned()),
                        ..junit_case("b")
                    },
                    JunitCase {
                        configuration: Some("cfg1".to_owned()),
                        ..junit_case("b")
                    },
                ],
            ),
            ("//single:t".to_owned(), vec![junit_case("c")]),
        ])
        .expect("render junit");
        assert!(doc.contains(r#"<testcase name="a [run=0]""#), "{doc}");
        assert!(doc.contains(r#"<testcase name="a [run=1]""#), "{doc}");
        assert!(doc.contains(r#"<testcase name="b [config=cfg1]""#), "{doc}");
        assert!(doc.contains(r#"<testcase name="b [config=cfg2]""#), "{doc}");
        assert!(doc.contains(r#"<testcase name="c""#), "{doc}");
        assert!(
            doc.find("a [run=0]") < doc.find("a [run=1]"),
            "runs sort in order: {doc}"
        );
    }

    #[test]
    fn junit_writer_escapes_specials_and_strips_controls() {
        let tricky = "a&<>\"'\u{0}\u{1}\u{8}\u{b}\u{c}\u{e}b";
        let doc = render_junit(&[(
            format!("suite&<>\"'{tricky}"),
            vec![JunitCase {
                name: tricky.to_owned(),
                classname: Some(format!("cls&<>\"'{tricky}")),
                time: 0.5,
                failure: Some(JunitMessage {
                    message: Some(format!("msg&<>\"'{tricky}")),
                    text: format!("body&<>\"'{tricky}"),
                }),
                error: None,
                skipped: None,
                system_out: Some(format!("out&<>{tricky}")),
                system_err: Some(format!("err&<>{tricky}")),
                shard: 0,
                attempt: 0,
                run: 0,
                configuration: None,
            }],
        )])
        .expect("render junit");
        assert!(doc.contains("&amp;"), "{doc}");
        assert!(doc.contains("&lt;"), "{doc}");
        for raw in ['\u{0}', '\u{1}', '\u{8}', '\u{b}', '\u{c}', '\u{e}'] {
            assert!(!doc.contains(raw), "{doc}");
        }
        let reparsed = parse_test_xml(doc.as_bytes(), 0, 0).expect("reparse");
        assert_eq!(reparsed.len(), 1);
        assert!(reparsed[0].failure.is_some());
    }
}
