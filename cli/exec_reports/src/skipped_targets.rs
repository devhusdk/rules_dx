use dx_bep::{EventGap, EventLedger};
use dx_exec_core::common::{CODE_TARGETS_SKIPPED, emit_built};
use dx_output::{NoticeEvent, OutputMode, notice_event};
use dx_plan::args::Invocation;
use dx_plan::resolve::{GraphCache, QueryRunner, SelectionContext};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

const UNSUPPORTED_JSON: &str = include_str!("unsupported.json");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipGroup {
    pub count: usize,
    pub language: Option<SkipLanguage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipLanguage {
    pub title: String,
    pub reason: String,
}

pub fn read_skipped(bep: &Path) -> Vec<EventGap> {
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

fn known_languages(json: &str) -> Vec<(String, String, String)> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    map.into_iter()
        .map(|(constraint, entry)| {
            let title = entry
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let reasons: Vec<(&String, &str)> = entry
                .get("platforms")
                .and_then(serde_json::Value::as_object)
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

pub fn language_in_message(message: &str) -> Option<SkipLanguage> {
    if !message.contains("explicitly requested") {
        return None;
    }
    known_languages(UNSUPPORTED_JSON)
        .into_iter()
        .find(|(constraint, _, _)| message.contains(constraint.as_str()))
        .map(|(_, title, reason)| SkipLanguage { title, reason })
}

pub fn group_skipped(
    skipped: &[EventGap],
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn QueryRunner,
) -> Vec<SkipGroup> {
    let selection = SelectionContext::for_test_execution(
        invocation.build_mode(),
        invocation.raw_bazel_options(),
        &invocation.bazel_startup_options,
    );
    let argv = |constraints: &[&str], labels: &[String]| {
        let targets: Vec<String> = constraints
            .iter()
            .map(|constraint| format!("@rules_dx{constraint}"))
            .collect();
        let expr = format!(
            "allpaths({}, {})",
            dx_plan::resolve::set_expression(labels),
            dx_plan::resolve::set_expression(&targets)
        );
        selection
            .selection_argv("cquery", &["--output=label".to_owned()], &expr)
            .ok()
    };
    let cache = |constraints: &[&str], labels: &[String]| {
        argv(constraints, labels)
            .map(|argv| GraphCache::new(workspace, dx_path::state::SKIPPED_TARGETS, &argv))
    };
    group_with(skipped, UNSUPPORTED_JSON, cache, |constraints, labels| {
        let argv = argv(constraints, labels)?;
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
    cache: impl Fn(&[&str], &[String]) -> Option<GraphCache>,
    query: impl Fn(&[&str], &[String]) -> Option<Vec<String>>,
) -> Vec<SkipGroup> {
    let mut labels: Vec<String> = skipped.iter().filter_map(|gap| gap.label.clone()).collect();
    labels.sort();
    labels.dedup();
    let languages = known_languages(json);
    let constraints: Vec<&str> = languages.iter().map(|(c, _, _)| c.as_str()).collect();
    let explained = if labels.is_empty() || constraints.is_empty() {
        Vec::new()
    } else {
        let cache = cache(&constraints, &labels);
        match cache.as_ref().and_then(read_explained) {
            Some(explained) => explained,
            None => {
                let (explained, complete) = explain(&labels, &constraints, &query);
                if let Some(cache) = cache.filter(|_| complete) {
                    write_explained(&cache, &explained);
                }
                explained
            }
        }
    };
    groups_for(skipped.len(), &languages, &explained)
}

fn explain(
    labels: &[String],
    constraints: &[&str],
    query: impl Fn(&[&str], &[String]) -> Option<Vec<String>>,
) -> (Vec<(String, String)>, bool) {
    let mut explained = Vec::new();
    let Some(found) = query(constraints, labels) else {
        return (explained, false);
    };
    let mut remaining: Vec<String> = labels
        .iter()
        .filter(|label| found.contains(label))
        .cloned()
        .collect();
    let mut complete = true;
    for constraint in constraints {
        if remaining.is_empty() {
            break;
        }
        let Some(found) = query(&[constraint], &remaining) else {
            complete = false;
            continue;
        };
        remaining.retain(|label| {
            let hit = found.contains(label);
            if hit {
                explained.push((label.clone(), (*constraint).to_owned()));
            }
            !hit
        });
    }
    (explained, complete)
}

fn read_explained(cache: &GraphCache) -> Option<Vec<(String, String)>> {
    let body = String::from_utf8(cache.read()?).ok()?;
    Some(
        body.lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(label, constraint)| (label.to_owned(), constraint.to_owned()))
            .collect(),
    )
}

fn write_explained(cache: &GraphCache, explained: &[(String, String)]) {
    let body: String = explained
        .iter()
        .map(|(label, constraint)| format!("{label}\t{constraint}\n"))
        .collect();
    cache.write(body.as_bytes());
}

fn groups_for(
    skipped: usize,
    languages: &[(String, String, String)],
    explained: &[(String, String)],
) -> Vec<SkipGroup> {
    let mut groups = Vec::new();
    let mut grouped = 0;
    for (constraint, title, reason) in languages {
        let hits = explained
            .iter()
            .filter(|(_, explained)| explained == constraint)
            .count();
        if hits == 0 {
            continue;
        }
        grouped += hits;
        groups.push(SkipGroup {
            count: hits,
            language: Some(SkipLanguage {
                title: title.clone(),
                reason: reason.clone(),
            }),
        });
    }
    let rest = skipped.saturating_sub(grouped);
    if rest > 0 {
        groups.push(SkipGroup {
            count: rest,
            language: None,
        });
    }
    groups
}

pub fn skipped_notice(
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
                dx_output::write_stderr(err, format_args!("dx: {message}\n"));
            }
            OutputMode::Diff => {}
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
        "//core/platforms:php_unsupported": {"title": "PHP", "platforms": {"windows_arm64": "no PHP build"}},
        "//core/platforms:dart_unsupported": {"title": "Dart", "platforms": {"a": "x", "b": "y"}}
    }"#;

    fn fake_query(constraints: &[&str], _labels: &[String]) -> Option<Vec<String>> {
        constraints
            .contains(&"//core/platforms:php_unsupported")
            .then(|| vec!["//a:x".to_owned(), "//a:y".to_owned()])
    }

    fn uncached(_: &[&str], _: &[String]) -> Option<GraphCache> {
        None
    }

    fn counted(
        calls: &std::cell::RefCell<Vec<usize>>,
    ) -> impl Fn(&[&str], &[String]) -> Option<Vec<String>> + '_ {
        move |constraints, labels| {
            calls.borrow_mut().push(constraints.len());
            fake_query(constraints, labels)
        }
    }

    #[test]
    fn one_query_when_no_language_explains_the_skips() {
        let calls = std::cell::RefCell::new(Vec::new());
        let groups = group_with(
            &[gap("//b:z"), gap("//b:w")],
            JSON,
            uncached,
            counted(&calls),
        );
        assert_eq!(groups, generic(2));
        assert_eq!(calls.into_inner(), vec![2]);
    }

    #[test]
    fn languages_are_queried_only_after_the_union_hits() {
        let calls = std::cell::RefCell::new(Vec::new());
        let groups = group_with(
            &[gap("//a:x"), gap("//b:z")],
            JSON,
            uncached,
            counted(&calls),
        );
        assert_eq!(groups[0].language.as_ref().unwrap().title, "PHP");
        assert_eq!(groups[1], generic(1)[0]);
        assert_eq!(calls.into_inner(), vec![2, 1, 1]);
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
        let groups = group_with(&skipped, JSON, uncached, fake_query);
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
        let groups = group_with(&skipped, JSON, uncached, |_, _| None);
        assert_eq!(
            groups,
            vec![SkipGroup {
                count: 1,
                language: None
            }]
        );
        assert!(
            group_with(&[], JSON, uncached, fake_query).is_empty(),
            "{:?}",
            group_with(&[], JSON, uncached, fake_query)
        );
    }

    fn cached_in(root: &Path) -> impl Fn(&[&str], &[String]) -> Option<GraphCache> + '_ {
        move |constraints, labels| {
            let mut argv: Vec<String> = constraints.iter().map(|c| (*c).to_owned()).collect();
            argv.extend_from_slice(labels);
            Some(GraphCache::new(
                root,
                dx_path::state::SKIPPED_TARGETS,
                &argv,
            ))
        }
    }

    fn answering(
        calls: &std::cell::RefCell<Vec<usize>>,
    ) -> impl Fn(&[&str], &[String]) -> Option<Vec<String>> + '_ {
        move |constraints, labels| {
            calls.borrow_mut().push(constraints.len());
            Some(fake_query(constraints, labels).unwrap_or_default())
        }
    }

    #[test]
    fn a_warm_run_reuses_the_grouping_until_the_graph_or_the_skips_change() {
        let scratch = dx_test_scratch::scratch("dx-skipped-cache-").unwrap();
        let root = scratch.path();
        dx_test_scratch::write(root, "MODULE.bazel", "module(name = \"w\")\n").unwrap();
        dx_test_scratch::write(root, "a/BUILD.bazel", "").unwrap();
        let skipped = [gap("//a:x"), gap("//b:z")];
        let calls = std::cell::RefCell::new(Vec::new());
        let first = group_with(&skipped, JSON, cached_in(root), answering(&calls));
        let warm = group_with(&skipped, JSON, cached_in(root), answering(&calls));
        assert_eq!(warm, first);
        assert_eq!(warm[0].language.as_ref().unwrap().title, "PHP");
        assert_eq!(calls.borrow().len(), 3, "the warm run runs no cquery");

        group_with(&[gap("//b:z")], JSON, cached_in(root), answering(&calls));
        assert_eq!(calls.borrow().len(), 4, "another skip set queries again");

        dx_test_scratch::write(root, "a/BUILD.bazel", "# edit\n").unwrap();
        let edited = group_with(&skipped, JSON, cached_in(root), answering(&calls));
        assert_eq!(edited, first);
        assert_eq!(calls.borrow().len(), 7, "a BUILD edit queries again");
    }

    #[test]
    fn a_failed_language_query_is_not_cached() {
        let scratch = dx_test_scratch::scratch("dx-skipped-cache-").unwrap();
        let root = scratch.path();
        dx_test_scratch::write(root, "MODULE.bazel", "module(name = \"w\")\n").unwrap();
        let calls = std::cell::Cell::new(0);
        let flaky = |constraints: &[&str], labels: &[String]| {
            calls.set(calls.get() + 1);
            (constraints.len() > 1)
                .then(|| fake_query(constraints, labels))
                .flatten()
        };
        let skipped = [gap("//a:x")];
        assert_eq!(
            group_with(&skipped, JSON, cached_in(root), flaky),
            generic(1)
        );
        group_with(&skipped, JSON, cached_in(root), flaky);
        assert_eq!(calls.get(), 6);
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
        assert!(
            !known_languages(UNSUPPORTED_JSON).is_empty(),
            "expected non-empty"
        );
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
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn text_notice_names_the_language_and_reason() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let groups = group_with(&[gap("//a:x")], JSON, uncached, fake_query);
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
        let groups = group_with(&[gap("//a:x")], JSON, uncached, fake_query);
        skipped_notice(OutputMode::Json, &mut out, &mut err, &groups).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\"targets_skipped\""), "{text}");
        assert!(text.contains("\"language\":\"PHP\""), "{text}");
    }
}
