//! The one-line notice for targets Bazel skipped as incompatible with the platform.

use super::common::{CODE_TARGETS_SKIPPED, emit_built};
use dx_bep::{EventGap, EventLedger};
use dx_output::{NoticeEvent, OutputMode, notice_event};
use dx_plan::args::Invocation;
use dx_plan::resolve::{QueryRunner, SelectionContext};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

const UNSUPPORTED_JSON: &str = include_str!("unsupported.json");

/// The skipped targets that share one unsupported language, or the rest when `language` is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkipGroup {
    pub count: usize,
    pub language: Option<SkipLanguage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkipLanguage {
    pub title: String,
    pub reason: String,
}

/// The labels the event file records as skipped, and how many skipped events it holds.
pub(crate) fn read_skipped(bep: &Path) -> Vec<EventGap> {
    let Ok(file) = std::fs::File::open(bep) else {
        return Vec::new();
    };
    let mut ledger = EventLedger::default();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if let Ok(serde_json::Value::Object(object)) = serde_json::from_str(&line) {
            ledger.observe(&object);
        }
    }
    ledger
        .gaps(&["targetCompleted"])
        .into_iter()
        .filter(|gap| gap.is_skipped() && gap.aspect.is_none())
        .collect()
}

/// Entries of `unsupported.json` as (constraint, title, reason).
fn known_languages(json: &str) -> Vec<(String, String, String)> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    map.into_iter()
        .map(|(constraint, entry)| {
            let title = entry["title"].as_str().unwrap_or_default().to_owned();
            let reasons: Vec<(&String, &str)> = entry["platforms"]
                .as_object()
                .map(|platforms| {
                    platforms
                        .iter()
                        .filter_map(|(platform, reason)| Some((platform, reason.as_str()?)))
                        .collect()
                })
                .unwrap_or_default();
            let reason = match reasons.as_slice() {
                [(_, reason)] => (*reason).to_owned(),
                many => many
                    .iter()
                    .map(|(platform, reason)| format!("{platform}: {reason}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            };
            (constraint, title, reason)
        })
        .collect()
}

/// The language named by an explicit-request error, from the unsupported constraint in its dependency chain.
pub(crate) fn language_in_message(message: &str) -> Option<SkipLanguage> {
    if !message.contains("explicitly requested") {
        return None;
    }
    known_languages(UNSUPPORTED_JSON)
        .into_iter()
        .find(|(constraint, _, _)| message.contains(constraint.as_str()))
        .map(|(_, title, reason)| SkipLanguage { title, reason })
}

/// Groups the skipped targets by the language whose unsupported constraint they depend on.
/// One cquery per language that `unsupported.json` lists; a failed query leaves its targets ungrouped.
pub(crate) fn group_skipped(
    skipped: &[EventGap],
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn QueryRunner,
) -> Vec<SkipGroup> {
    group_with(skipped, UNSUPPORTED_JSON, |constraint, labels| {
        let selection = SelectionContext::for_test_execution(
            invocation.build_mode(),
            invocation.raw_bazel_options(),
            &invocation.bazel_startup_options,
        );
        let expr = format!(
            "allpaths({}, @rules_dx{constraint})",
            dx_plan::resolve::set_expression(labels)
        );
        let argv = selection
            .selection_argv("cquery", &["--output=label".to_owned()], &expr)
            .ok()?;
        let result = runner.run_query(&argv, workspace).ok()?;
        if result.code != Some(0) {
            return None;
        }
        Some(
            String::from_utf8_lossy(&result.stdout)
                .lines()
                .filter_map(|line| line.split_whitespace().next())
                .map(str::to_owned)
                .collect(),
        )
    })
}

fn group_with(
    skipped: &[EventGap],
    json: &str,
    query: impl Fn(&str, &[String]) -> Option<Vec<String>>,
) -> Vec<SkipGroup> {
    let mut remaining: Vec<String> = skipped.iter().filter_map(|gap| gap.label.clone()).collect();
    remaining.sort();
    remaining.dedup();
    let mut groups = Vec::new();
    let mut grouped = 0;
    if !remaining.is_empty() {
        for (constraint, title, reason) in known_languages(json) {
            let Some(found) = query(&constraint, &remaining) else {
                continue;
            };
            let hits = remaining
                .iter()
                .filter(|label| found.contains(label))
                .count();
            if hits == 0 {
                continue;
            }
            remaining.retain(|label| !found.contains(label));
            grouped += hits;
            groups.push(SkipGroup {
                count: hits,
                language: Some(SkipLanguage { title, reason }),
            });
            if remaining.is_empty() {
                break;
            }
        }
    }
    let rest = skipped.len().saturating_sub(grouped);
    if rest > 0 {
        groups.push(SkipGroup {
            count: rest,
            language: None,
        });
    }
    groups
}

/// Tells the user how many targets were skipped and why. The exit code is not affected.
pub(crate) fn skipped_notice(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    groups: &[SkipGroup],
) -> Result<(), i32> {
    for group in groups.iter().filter(|group| group.count > 0) {
        let noun = if group.count == 1 {
            "target"
        } else {
            "targets"
        };
        let (message, language) = match &group.language {
            Some(language) => (
                format!(
                    "skipped {} {} {noun}: {} (docs/support-matrix.md)",
                    group.count, language.title, language.reason
                ),
                Some(language.title.clone()),
            ),
            None => (
                format!(
                    "skipped {} {noun} incompatible with this platform (docs/support-matrix.md)",
                    group.count
                ),
                None,
            ),
        };
        match output {
            OutputMode::Json => {
                let mut notice = NoticeEvent::info(CODE_TARGETS_SKIPPED, message);
                notice.language = language;
                emit_built(out, notice_event(&notice))?;
            }
            OutputMode::Text { .. } => {
                let _ = writeln!(err, "dx: {message}");
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skipped_line(label: &str) -> String {
        serde_json::json!({
            "id": {"targetCompleted": {"label": label, "configuration": {"id": "c"}}},
            "aborted": {"reason": "SKIPPED"},
        })
        .to_string()
    }

    fn gap(label: &str) -> EventGap {
        EventGap {
            kind: "targetCompleted".to_owned(),
            label: Some(label.to_owned()),
            configuration: None,
            aspect: None,
            run: None,
            shard: None,
            attempt: None,
            aborted: Some("SKIPPED".to_owned()),
        }
    }

    const JSON: &str = r#"{
        "//php/private/toolchain:unsupported": {"title": "PHP", "platforms": {"windows_arm64": "no PHP build"}},
        "//dart/rules:unsupported": {"title": "Dart", "platforms": {"a": "x", "b": "y"}}
    }"#;

    fn fake_query(constraint: &str, _labels: &[String]) -> Option<Vec<String>> {
        match constraint {
            "//php/private/toolchain:unsupported" => Some(vec!["//a:x".to_owned(), "//a:y".to_owned()]),
            _ => None,
        }
    }

    #[test]
    fn reads_only_skipped_targets() {
        let dir = std::env::temp_dir().join(format!("dx-skipped-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bep = dir.join("bep.json");
        let done = serde_json::json!({
            "id": {"targetCompleted": {"label": "//a:ok", "configuration": {"id": "c"}}},
            "completed": {"success": true},
        });
        let text = format!(
            "{}\n{}\n{}\n",
            skipped_line("//a:x"),
            skipped_line("//a:y"),
            done
        );
        std::fs::write(&bep, text).unwrap();
        assert_eq!(read_skipped(&bep).len(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn groups_by_language_and_keeps_unknown_skips_generic() {
        let skipped = [gap("//a:x"), gap("//a:y"), gap("//b:z")];
        let groups = group_with(&skipped, JSON, fake_query);
        assert_eq!(groups.len(), 2, "{groups:?}");
        assert_eq!(groups[0].count, 2);
        let php = groups[0].language.as_ref().unwrap();
        assert_eq!(
            (php.title.as_str(), php.reason.as_str()),
            ("PHP", "no PHP build")
        );
        assert_eq!(
            groups[1],
            SkipGroup {
                count: 1,
                language: None
            }
        );
    }

    #[test]
    fn failed_queries_leave_the_generic_group() {
        let skipped = [gap("//a:x")];
        let groups = group_with(&skipped, JSON, |_, _| None);
        assert_eq!(
            groups,
            vec![SkipGroup {
                count: 1,
                language: None
            }]
        );
        assert!(group_with(&[], JSON, fake_query).is_empty());
    }

    #[test]
    fn several_platform_reasons_are_listed() {
        let languages = known_languages(JSON);
        assert_eq!(languages[0].2, "a: x; b: y");
    }

    #[test]
    fn explicit_request_errors_name_the_language() {
        let (constraint, title, _) = known_languages(UNSUPPORTED_JSON).remove(0);
        let message = format!(
            "Target //a:x is incompatible and cannot be built, but was explicitly requested.\nDependency chain:\n    //a:x\n    @@rules_dx+{constraint}"
        );
        assert_eq!(language_in_message(&message).unwrap().title, title);
        assert!(language_in_message(&format!("other {constraint}")).is_none());
        assert!(language_in_message("explicitly requested, no constraint").is_none());
    }

    #[test]
    fn committed_unsupported_json_parses() {
        assert!(!known_languages(UNSUPPORTED_JSON).is_empty());
    }

    fn generic(count: usize) -> Vec<SkipGroup> {
        vec![SkipGroup {
            count,
            language: None,
        }]
    }

    #[test]
    fn text_notice_is_one_stderr_line() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let output = OutputMode::Text { quiet: false };
        skipped_notice(output, &mut out, &mut err, &generic(2)).unwrap();
        let err = String::from_utf8(err).unwrap();
        assert_eq!(
            err,
            "dx: skipped 2 targets incompatible with this platform (docs/support-matrix.md)\n"
        );
        assert!(out.is_empty());
    }

    #[test]
    fn text_notice_names_the_language_and_reason() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let groups = group_with(&[gap("//a:x")], JSON, fake_query);
        skipped_notice(
            OutputMode::Text { quiet: false },
            &mut out,
            &mut err,
            &groups,
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "dx: skipped 1 PHP target: no PHP build (docs/support-matrix.md)\n"
        );
    }

    #[test]
    fn json_notice_carries_the_language_and_zero_prints_nothing() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        skipped_notice(OutputMode::Json, &mut out, &mut err, &[]).unwrap();
        assert!(out.is_empty() && err.is_empty());
        let groups = group_with(&[gap("//a:x")], JSON, fake_query);
        skipped_notice(OutputMode::Json, &mut out, &mut err, &groups).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\"targets_skipped\""), "{text}");
        assert!(text.contains("\"language\":\"PHP\""), "{text}");
    }
}
