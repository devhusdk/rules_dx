use super::{
    Evidence, OutputMode, Path, Sources, TestEvents, TestOutcome, WorkflowVerb, Write,
    collect_lcov, collect_test_xml, emit_outcome, mark_superseded_attempts, summary_statuses,
};
use dx_bep::{OutputLocations, collect_test_events};
use dx_exec_core::common::FsArtifacts;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::{Read, Seek, SeekFrom};
use std::time::{Duration, Instant};

pub const PROGRESS_EVERY: Duration = Duration::from_secs(60);
const READ_EVERY: Duration = Duration::from_secs(1);
const LISTED_TESTS: usize = 20;

type OutcomeKey = (
    String,
    Option<String>,
    Option<u32>,
    Option<u32>,
    Option<u32>,
);

fn outcome_key(outcome: &TestOutcome) -> OutcomeKey {
    (
        outcome.target.clone(),
        outcome.configuration.clone(),
        outcome.run,
        outcome.shard,
        outcome.attempt,
    )
}

#[derive(Debug, Default)]
pub struct Streamed(BTreeSet<OutcomeKey>);

impl Streamed {
    pub(super) fn contains(&self, outcome: &TestOutcome) -> bool {
        self.0.contains(&outcome_key(outcome))
    }
}

pub struct LiveTests<'a> {
    workspace: &'a Path,
    bep: &'a Path,
    verb: WorkflowVerb,
    output: OutputMode,
    started: Instant,
    progress_at: Instant,
    read_at: Option<Instant>,
    events: Vec<u8>,
    scanned: usize,
    tests: BTreeSet<String>,
    finished: BTreeSet<String>,
    failed: BTreeSet<String>,
    unsent: BTreeSet<String>,
    streamed: Streamed,
    closed: bool,
}

impl<'a> LiveTests<'a> {
    #[must_use]
    pub fn new(
        workspace: &'a Path,
        bep: &'a Path,
        verb: WorkflowVerb,
        output: OutputMode,
        now: Instant,
    ) -> Self {
        LiveTests {
            workspace,
            bep,
            verb,
            output,
            started: now,
            progress_at: now,
            read_at: None,
            events: Vec::new(),
            scanned: 0,
            tests: BTreeSet::new(),
            finished: BTreeSet::new(),
            failed: BTreeSet::new(),
            unsent: BTreeSet::new(),
            streamed: Streamed::default(),
            closed: false,
        }
    }

    #[must_use]
    pub fn streamed(self) -> Streamed {
        self.streamed
    }

    pub fn poll(
        &mut self,
        out: &mut dyn Write,
        err: &mut dyn Write,
        now: Instant,
        interrupted: bool,
    ) {
        if self.closed {
            return;
        }
        if !interrupted
            && self
                .read_at
                .is_some_and(|at| now.duration_since(at) < READ_EVERY)
        {
            return;
        }
        self.read_at = Some(now);
        self.read();
        if self.send(out, err).is_err() {
            self.closed = true;
            return;
        }
        if interrupted {
            self.closed = true;
            self.interrupted(out, err, now);
        } else if now.duration_since(self.progress_at) >= PROGRESS_EVERY {
            self.progress_at = now;
            dx_output::write_stderr(err, format_args!("{}\n", self.progress(now)));
        }
    }

    fn read(&mut self) {
        let Ok(mut file) = std::fs::File::open(self.bep) else {
            return;
        };
        let have = u64::try_from(self.events.len()).unwrap_or(u64::MAX);
        if file.metadata().is_ok_and(|meta| meta.len() < have) {
            self.events.clear();
            self.scanned = 0;
            self.tests.clear();
            self.finished.clear();
            self.failed.clear();
            self.unsent.clear();
        }
        let start = u64::try_from(self.events.len()).unwrap_or(u64::MAX);
        if file.seek(SeekFrom::Start(start)).is_err() || file.read_to_end(&mut self.events).is_err()
        {
            return;
        }
        while let Some(end) = self
            .events
            .get(self.scanned..)
            .and_then(|rest| rest.iter().position(|byte| *byte == b'\n'))
        {
            let line = self
                .events
                .get(self.scanned..self.scanned + end)
                .unwrap_or_default();
            let event = serde_json::from_slice::<Value>(line).ok();
            self.scanned += end + 1;
            if let Some(event) = event {
                self.observe(&event);
            }
        }
    }

    fn observe(&mut self, event: &Value) {
        let id = event.get("id");
        if let Some(label) = id
            .and_then(|id| id.get("targetConfigured"))
            .filter(|configured| configured.get("aspect").is_none())
            .and_then(|configured| configured.get("label"))
            .and_then(Value::as_str)
            && event
                .get("configured")
                .and_then(|body| body.get("testSize"))
                .is_some()
        {
            self.tests.insert(label.to_owned());
        }
        if let Some(label) = id
            .and_then(|id| id.get("testSummary"))
            .and_then(|summary| summary.get("label"))
            .and_then(Value::as_str)
        {
            let status = event
                .get("testSummary")
                .and_then(|summary| summary.get("overallStatus"))
                .and_then(Value::as_str);
            if !matches!(status, Some("PASSED" | "FLAKY")) {
                self.failed.insert(label.to_owned());
            }
            self.finished.insert(label.to_owned());
            self.unsent.insert(label.to_owned());
        }
    }

    fn send(&mut self, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), i32> {
        if self.unsent.is_empty() {
            return Ok(());
        }
        let unsent = std::mem::take(&mut self.unsent);
        let complete = self.events.get(..self.scanned).unwrap_or_default();
        let Ok(all) = collect_test_events(complete, || Some(OutputLocations::new(self.workspace)))
        else {
            return Ok(());
        };
        let events = TestEvents {
            results: all
                .results
                .into_iter()
                .filter(|result| unsent.contains(&result.label))
                .collect(),
            summaries: all
                .summaries
                .into_iter()
                .filter(|summary| unsent.contains(&summary.label))
                .collect(),
            ended: true,
            ..TestEvents::default()
        };
        let summaries = summary_statuses(&events);
        let mut reported = if self.verb == WorkflowVerb::Coverage {
            collect_lcov(
                &events,
                &summaries,
                &FsArtifacts,
                Evidence::default(),
                Sources::default(),
            )
            .reported
        } else {
            collect_test_xml(
                &events,
                &summaries,
                &FsArtifacts,
                Evidence::default(),
                &mut std::io::sink(),
            )
            .reported
        };
        mark_superseded_attempts(&mut reported);
        for outcome in reported.iter().filter(|outcome| outcome.evidence_complete) {
            if self.streamed.0.insert(outcome_key(outcome)) {
                emit_outcome(self.output, out, err, outcome)?;
            }
        }
        out.flush()
            .map_err(|error| dx_process::stdout_io_code(&error))
    }

    fn progress(&self, now: Instant) -> String {
        let total = self.tests.len().max(self.finished.len());
        format!(
            "dx: {}: {}/{total} tests finished, {} failed, {}",
            self.verb.name(),
            self.finished.len(),
            self.failed.len(),
            elapsed(now.duration_since(self.started))
        )
    }

    fn interrupted(&self, out: &mut dyn Write, err: &mut dyn Write, now: Instant) {
        dx_output::write_stderr(err, format_args!("{} (interrupted)\n", self.progress(now)));
        let running: Vec<&str> = self
            .tests
            .difference(&self.finished)
            .map(String::as_str)
            .collect();
        if !running.is_empty() {
            let mut line = running
                .iter()
                .take(LISTED_TESTS)
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            if running.len() > LISTED_TESTS {
                line.push_str(&format!(" and {} more", running.len() - LISTED_TESTS));
            }
            dx_output::write_stderr(
                err,
                format_args!("dx: {}: not finished: {line}\n", self.verb.name()),
            );
        }
        drop(out.flush());
        drop(err.flush());
    }
}

fn elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{}m{:02}s", seconds / 60, seconds % 60)
}
