use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

/// One announced or aborted event that never delivered its payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventGap {
    pub kind: String,
    pub label: Option<String>,
    pub configuration: Option<String>,
    pub aspect: Option<String>,
    pub run: Option<u32>,
    pub shard: Option<u32>,
    pub attempt: Option<u32>,
    pub aborted: Option<String>,
}

impl EventGap {
    /// True when Bazel skipped the work on purpose, such as an incompatible target.
    pub fn is_skipped(&self) -> bool {
        self.aborted.as_deref() == Some("SKIPPED")
    }

    /// The machine outcome for this gap.
    pub fn outcome(&self) -> &'static str {
        match self.aborted.as_deref() {
            None => "not_reported",
            Some("SKIPPED") => "skipped",
            Some("USER_INTERRUPTED") => "cancelled",
            Some("TIME_OUT") => "timeout",
            Some("LOADING_FAILURE" | "ANALYSIS_FAILURE") => "failed_to_build",
            Some(_) => "aborted",
        }
    }

    /// Names the gap for a human: its kind, identity, and why it is missing.
    pub fn describe(&self) -> String {
        let mut text = self.kind.clone();
        if let Some(label) = &self.label {
            text.push_str(&format!(" for {label}"));
        }
        if let Some(aspect) = &self.aspect {
            text.push_str(&format!(" ({aspect})"));
        }
        if let (Some(run), Some(shard), Some(attempt)) = (self.run, self.shard, self.attempt) {
            text.push_str(&format!(" (run {run}, shard {shard}, attempt {attempt})"));
        }
        match &self.aborted {
            None => text.push_str(" was announced but never reported"),
            Some(reason) => text.push_str(&format!(" was aborted: {reason}")),
        }
        text
    }
}

/// The announced, delivered, and aborted event ids of one build event stream.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventLedger {
    ended: bool,
    announced: BTreeMap<String, (String, Map<String, Value>)>,
    delivered: BTreeSet<String>,
    aborted: BTreeMap<String, (String, Map<String, Value>, String)>,
}

fn canonical(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let fields: Vec<String> = keys
                .into_iter()
                .map(|key| format!("{}:{}", Value::String(key.clone()), canonical(&map[key])))
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(canonical).collect();
            format!("[{}]", items.join(","))
        }
        other => other.to_string(),
    }
}

/// The kind and body of one event id, when it has exactly one kind.
fn id_parts(id: &Map<String, Value>) -> Option<(String, Map<String, Value>)> {
    let mut entries = id.iter();
    let (kind, body) = entries.next()?;
    if entries.next().is_some() {
        return None;
    }
    Some((kind.clone(), body.as_object().cloned().unwrap_or_default()))
}

fn index_of(body: &Map<String, Value>, field: &str) -> Option<u32> {
    body.get(field)
        .and_then(Value::as_u64)
        .and_then(|raw| u32::try_from(raw).ok())
}

impl EventLedger {
    /// Records one event: its own id, the ids it announces, and whether it ends the stream.
    pub fn observe(&mut self, object: &Map<String, Value>) {
        if object.get("lastMessage").and_then(Value::as_bool) == Some(true) {
            self.ended = true;
        }
        if let Some(children) = object.get("children").and_then(Value::as_array) {
            for child in children {
                if let Some((kind, body)) = child.as_object().and_then(id_parts) {
                    self.announced.insert(canonical(child), (kind, body));
                }
            }
        }
        let Some(id) = object.get("id") else {
            return;
        };
        let key = canonical(id);
        match object.get("aborted") {
            Some(aborted) => {
                let reason = aborted
                    .as_object()
                    .and_then(|aborted| aborted.get("reason"))
                    .and_then(Value::as_str)
                    .unwrap_or("UNKNOWN")
                    .to_owned();
                if let Some((kind, body)) = id.as_object().and_then(id_parts) {
                    self.aborted.insert(key, (kind, body, reason));
                }
            }
            None => {
                self.delivered.insert(key);
            }
        }
    }

    /// True once Bazel's last event arrived.
    pub fn ended(&self) -> bool {
        self.ended
    }

    /// Every announced or aborted event of these kinds that never delivered its payload.
    pub fn gaps(&self, kinds: &[&str]) -> Vec<EventGap> {
        let mut gaps = Vec::new();
        for (key, (kind, body)) in &self.announced {
            if !kinds.contains(&kind.as_str())
                || self.delivered.contains(key)
                || self.aborted.contains_key(key)
            {
                continue;
            }
            gaps.push(gap(kind, body, None));
        }
        for (key, (kind, body, reason)) in &self.aborted {
            if !kinds.contains(&kind.as_str()) || self.delivered.contains(key) {
                continue;
            }
            gaps.push(gap(kind, body, Some(reason.clone())));
        }
        gaps.sort_by(|a, b| {
            (&a.label, &a.kind, &a.aspect, a.run, a.shard, a.attempt)
                .cmp(&(&b.label, &b.kind, &b.aspect, b.run, b.shard, b.attempt))
        });
        gaps
    }
}

fn gap(kind: &str, body: &Map<String, Value>, aborted: Option<String>) -> EventGap {
    EventGap {
        kind: kind.to_owned(),
        label: body.get("label").and_then(Value::as_str).map(str::to_owned),
        configuration: body
            .get("configuration")
            .and_then(Value::as_object)
            .and_then(|configuration| configuration.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        aspect: body
            .get("aspect")
            .and_then(Value::as_str)
            .map(str::to_owned),
        run: index_of(body, "run"),
        shard: index_of(body, "shard"),
        attempt: index_of(body, "attempt"),
        aborted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger(lines: &[&str]) -> EventLedger {
        let mut ledger = EventLedger::default();
        for line in lines {
            let value: Value = serde_json::from_str(line).expect("json");
            ledger.observe(value.as_object().expect("object"));
        }
        ledger
    }

    #[test]
    fn a_stream_without_its_last_event_has_not_ended() {
        let open = ledger(&[r#"{"id":{"started":{}}}"#]);
        assert!(!open.ended());
        let closed = ledger(&[
            r#"{"id":{"started":{}}}"#,
            r#"{"id":{"buildMetrics":{}},"lastMessage":true}"#,
        ]);
        assert!(closed.ended());
    }

    #[test]
    fn announced_events_match_regardless_of_key_order() {
        let found = ledger(&[
            r#"{"id":{"targetCompleted":{"label":"//a:t","configuration":{"id":"c"}}},"children":[{"testResult":{"label":"//a:t","run":1,"shard":1,"attempt":1,"configuration":{"id":"c"}}}]}"#,
            r#"{"id":{"testResult":{"configuration":{"id":"c"},"attempt":1,"shard":1,"run":1,"label":"//a:t"}},"testResult":{"status":"PASSED"}}"#,
        ]);
        assert!(found.gaps(&["testResult"]).is_empty());
    }

    #[test]
    fn an_announced_event_that_never_arrives_is_a_gap() {
        let found = ledger(&[
            r#"{"id":{"targetCompleted":{"label":"//a:t"}},"children":[{"testResult":{"label":"//a:t","run":1,"shard":2,"attempt":1}},{"testSummary":{"label":"//a:t"}}]}"#,
        ]);
        let gaps = found.gaps(&["testResult"]);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].label.as_deref(), Some("//a:t"));
        assert_eq!(gaps[0].shard, Some(2));
        assert_eq!(gaps[0].aborted, None);
        assert_eq!(gaps[0].outcome(), "not_reported");
        assert_eq!(
            gaps[0].describe(),
            "testResult for //a:t (run 1, shard 2, attempt 1) was announced but never reported"
        );
        assert_eq!(found.gaps(&["testResult", "testSummary"]).len(), 2);
    }

    #[test]
    fn an_aborted_event_names_its_reason_and_outcome() {
        let found = ledger(&[
            r#"{"id":{"testResult":{"label":"//a:t","run":1,"shard":1,"attempt":1}},"aborted":{"reason":"USER_INTERRUPTED"}}"#,
            r#"{"id":{"targetCompleted":{"label":"//a:i","aspect":"//q.bzl%a"}},"aborted":{"reason":"SKIPPED"}}"#,
            r#"{"id":{"targetCompleted":{"label":"//a:b"}},"aborted":{}}"#,
            r#"{"id":{"targetConfigured":{"label":"//a:x"}},"aborted":{"reason":"ANALYSIS_FAILURE"}}"#,
            r#"{"id":{"targetCompleted":{"label":"//a:s"}},"aborted":{"reason":"TIME_OUT"}}"#,
        ]);
        let results = found.gaps(&["testResult"]);
        assert_eq!(results[0].outcome(), "cancelled");
        assert!(results[0]
            .describe()
            .ends_with("was aborted: USER_INTERRUPTED"));
        let targets = found.gaps(&["targetCompleted"]);
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0].outcome(), "aborted");
        assert_eq!(targets[0].aborted.as_deref(), Some("UNKNOWN"));
        assert!(targets[1].is_skipped());
        assert_eq!(targets[1].outcome(), "skipped");
        assert!(targets[1].describe().contains("(//q.bzl%a)"));
        assert_eq!(targets[2].outcome(), "timeout");
        assert_eq!(
            found.gaps(&["targetConfigured"])[0].outcome(),
            "failed_to_build"
        );
    }

    #[test]
    fn ids_with_several_kinds_are_not_tracked() {
        let found = ledger(&[
            r#"{"id":{"a":{},"b":{}},"aborted":{}}"#,
            r#"{"id":{"started":{}},"children":[{"a":{},"b":{}}]}"#,
        ]);
        assert!(found.gaps(&["a", "b"]).is_empty());
    }

    #[test]
    fn canonical_ids_sort_nested_keys() {
        let a: Value = serde_json::from_str(r#"{"x":{"b":1,"a":[{"d":2,"c":3}]}}"#).expect("a");
        let b: Value = serde_json::from_str(r#"{"x":{"a":[{"c":3,"d":2}],"b":1}}"#).expect("b");
        assert_eq!(canonical(&a), canonical(&b));
    }
}
