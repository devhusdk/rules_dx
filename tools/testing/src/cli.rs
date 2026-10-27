//! Shared helpers for tests of the dx CLI crates.

/// Copies test words into owned strings, in order.
pub fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

/// Renders one rule as a line of `streamed_jsonproto` query output.
pub fn graph_rule(name: &str, inputs: &[&str], tags: &[&str]) -> String {
    render_graph_rule(name, None, inputs, tags)
}

/// Renders one rule of the given rule class as a line of query output.
pub fn graph_rule_of_kind(name: &str, kind: &str, inputs: &[&str], tags: &[&str]) -> String {
    render_graph_rule(name, Some(kind), inputs, tags)
}

fn render_graph_rule(name: &str, kind: Option<&str>, inputs: &[&str], tags: &[&str]) -> String {
    let mut rule = serde_json::json!({
        "name": name,
        "ruleInput": inputs,
        "attribute": [{"name": "tags", "stringListValue": tags}],
    });
    if let Some(kind) = kind {
        rule["ruleClass"] = serde_json::json!(kind);
    }
    serde_json::json!({"type": "RULE", "rule": rule}).to_string()
}

/// Joins rule lines into the rule graph query output.
pub fn graph_stdout(lines: &[String]) -> String {
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Parses one NDJSON event per line.
pub fn json_events(out: &str) -> Vec<serde_json::Value> {
    out.lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap_or_else(|error| panic!("NDJSON: {error}"))
}

/// Returns the kind of every event, in order.
pub fn event_kinds(events: &[serde_json::Value]) -> Vec<&str> {
    events
        .iter()
        .map(|event| {
            event["event"]
                .as_str()
                .unwrap_or_else(|| panic!("event without a kind: {event}"))
        })
        .collect()
}

/// Returns the first event of the given kind.
pub fn event<'a>(events: &'a [serde_json::Value], kind: &str) -> &'a serde_json::Value {
    events
        .iter()
        .find(|event| event["event"] == serde_json::json!(kind))
        .unwrap_or_else(|| panic!("missing {kind} event"))
}

/// Returns every event of the given kind, in order.
pub fn events_of_kind<'a>(
    events: &'a [serde_json::Value],
    kind: &str,
) -> Vec<&'a serde_json::Value> {
    events
        .iter()
        .filter(|event| event["event"] == serde_json::json!(kind))
        .collect()
}

/// Splits a markdown page at its `## ` headings into (heading, body) pairs; the first heading is empty.
pub fn markdown_sections(page: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut heading = String::new();
    let mut body: Vec<&str> = Vec::new();
    for line in page.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            out.push((std::mem::take(&mut heading), body.join("\n")));
            heading = rest.trim().to_owned();
            body.clear();
            continue;
        }
        body.push(line);
    }
    out.push((heading, body.join("\n")));
    out
}

/// Returns every complete backticked span in `text`, in order.
pub fn backticked(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let tail = &rest[start + 1..];
        let Some(end) = tail.find('`') else {
            break;
        };
        out.push(tail[..end].to_owned());
        rest = &tail[end + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const STREAM: &str = concat!(
        r#"{"event":"command_started","exit_code":0}"#,
        "\n",
        r#"{"event":"notice","detail":"//a:one"}"#,
        "\n",
        r#"{"event":"command_finished","exit_code":0}"#,
        "\n",
    );

    #[test]
    fn strings_owns_every_word_in_order() {
        assert_eq!(
            strings(&["lint", "--check", "lint"]),
            vec!["lint".to_owned(), "--check".to_owned(), "lint".to_owned()]
        );
    }

    #[test]
    fn graph_stdout_holds_one_rule_per_line() {
        let out = graph_stdout(&[
            graph_rule("//a:lib", &["//a:a.txt"], &[]),
            graph_rule("//a:t", &["//a:lib"], &["manual"]),
        ]);
        let rules: Vec<serde_json::Value> = out
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["rule"]["ruleInput"][0], "//a:a.txt");
        assert_eq!(
            rules[1]["rule"]["attribute"][0]["stringListValue"][0],
            "manual"
        );
    }

    #[test]
    fn strings_owns_nothing_from_no_words() {
        assert!(strings(&[]).is_empty());
    }

    #[test]
    fn json_events_reads_one_value_per_line() {
        let events = json_events(STREAM);
        assert_eq!(events.len(), 3);
        assert_eq!(events[1]["detail"], serde_json::json!("//a:one"));
    }

    #[test]
    fn json_events_reads_nothing_from_empty_output() {
        assert!(json_events("").is_empty());
    }

    #[test]
    #[should_panic(expected = "NDJSON")]
    fn json_events_rejects_a_line_that_is_not_json() {
        json_events("{\"event\":\"notice\"}\nnot json\n");
    }

    #[test]
    fn event_kinds_reports_every_kind_in_order() {
        assert_eq!(
            event_kinds(&json_events(STREAM)),
            vec!["command_started", "notice", "command_finished"]
        );
    }

    #[test]
    #[should_panic(expected = "event")]
    fn event_kinds_rejects_an_event_without_a_kind() {
        event_kinds(&json_events(r#"{"detail":"//a:one"}"#));
    }

    #[test]
    fn event_returns_the_first_event_of_a_kind() {
        let events = json_events(STREAM);
        assert_eq!(event(&events, "notice")["detail"], "//a:one");
    }

    #[test]
    fn event_prefers_the_earlier_of_two_events_of_a_kind() {
        let events = json_events(concat!(
            r#"{"event":"notice","detail":"first"}"#,
            "\n",
            r#"{"event":"notice","detail":"second"}"#,
            "\n",
        ));
        assert_eq!(event(&events, "notice")["detail"], "first");
    }

    #[test]
    #[should_panic(expected = "missing error event")]
    fn event_panics_when_the_kind_is_absent() {
        event(&json_events(STREAM), "error");
    }

    #[test]
    fn events_of_kind_returns_every_match_in_order() {
        let events = json_events(concat!(
            r#"{"event":"notice","detail":"first"}"#,
            "\n",
            r#"{"event":"error","detail":"boom"}"#,
            "\n",
            r#"{"event":"notice","detail":"second"}"#,
            "\n",
        ));
        let notices = events_of_kind(&events, "notice");
        assert_eq!(notices.len(), 2);
        assert_eq!(notices[0]["detail"], "first");
        assert_eq!(notices[1]["detail"], "second");
        assert!(events_of_kind(&events, "warning").is_empty());
    }

    #[test]
    fn markdown_sections_split_at_second_level_headings() {
        let got = markdown_sections("intro\n## `dx a`\nbody a\n### deeper\n## b\n");
        assert_eq!(
            got,
            vec![
                (String::new(), "intro".to_owned()),
                ("`dx a`".to_owned(), "body a\n### deeper".to_owned()),
                ("b".to_owned(), String::new()),
            ]
        );
    }

    #[test]
    fn backticked_skips_an_unterminated_span() {
        assert_eq!(backticked("a `x` b `y` c `z"), vec!["x", "y"]);
        assert!(backticked("none").is_empty());
    }
}
