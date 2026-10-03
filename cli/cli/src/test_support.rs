/// Parses one NDJSON event per line.
pub(crate) fn json_events(out: &str) -> Vec<serde_json::Value> {
    out.lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("NDJSON")
}

/// Returns the kind of every event, in order.
pub(crate) fn event_kinds(events: &[serde_json::Value]) -> Vec<&str> {
    events
        .iter()
        .map(|event| event["event"].as_str().expect("event"))
        .collect()
}

/// Returns the first event of the given kind.
pub(crate) fn event<'a>(events: &'a [serde_json::Value], kind: &str) -> &'a serde_json::Value {
    events
        .iter()
        .find(|event| event["event"] == serde_json::json!(kind))
        .unwrap_or_else(|| panic!("missing {kind} event"))
}

/// Returns every event of the given kind, in order.
pub(crate) fn events_of_kind<'a>(
    events: &'a [serde_json::Value],
    kind: &str,
) -> Vec<&'a serde_json::Value> {
    events
        .iter()
        .filter(|event| event["event"] == serde_json::json!(kind))
        .collect()
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
}
