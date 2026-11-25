use super::{phase_correlation, selection_stop_message};
use dx_exec_core::common::{CODE_COLLECTION_FAILED, emit_built, emit_event};
use dx_exec_core::failure::CODE_INVALID_SCOPE;
use dx_output::{
    FinishedCounts, OutputMode, command_finished, command_started, error_event, nest_phase_event,
    read_event, with_correlation,
};
use dx_process::stdout_io_code;
use serde_json::Value;
use std::io::{self, Write};

pub(super) struct Relay<'a> {
    out: &'a mut dyn Write,
    output: OutputMode,
    command: &'a str,
    mode: &'a str,
    phase: &'a str,
    pending: Vec<u8>,
    held: Vec<Value>,
    started: bool,
    finished: bool,
    readable: bool,
    saw_error: bool,
    failed: Option<i32>,
}

impl<'a> Relay<'a> {
    pub(super) fn new(
        out: &'a mut dyn Write,
        output: OutputMode,
        (command, mode): (&'a str, &'a str),
        phase: &'a str,
    ) -> Self {
        Relay {
            out,
            output,
            command,
            mode,
            phase,
            pending: Vec::new(),
            held: Vec::new(),
            started: false,
            finished: false,
            readable: true,
            saw_error: false,
            failed: None,
        }
    }

    fn nest(
        &self,
        event: Result<Value, dx_output::OutputError>,
    ) -> Result<Value, dx_output::OutputError> {
        let correlation = phase_correlation(self.command, self.phase);
        event.and_then(|event| nest_phase_event(event, self.command, self.phase, &correlation))
    }

    fn line(&mut self, line: &[u8]) -> Result<(), i32> {
        if line.is_empty() {
            return Ok(());
        }
        let nested = self.nest(
            std::str::from_utf8(line)
                .map_err(|error| {
                    tracing::info!(%error, "phase stdout line is not UTF-8");
                    dx_output::OutputError::NotAnEvent
                })
                .and_then(read_event),
        );
        let Ok(event) = nested else {
            self.readable = false;
            return Ok(());
        };
        self.forward(event)
    }

    fn forward(&mut self, event: Value) -> Result<(), i32> {
        let name = event.get("event").and_then(Value::as_str);
        if !self.started && name != Some("phase_started") {
            let started = self.nest(command_started(self.phase, self.mode));
            emit_built(self.out, started)?;
        }
        self.started = true;
        self.saw_error |= name == Some("error");
        let closing = name == Some("phase_finished");
        self.finished |= closing;
        if closing || !self.held.is_empty() {
            self.held.push(event);
            return Ok(());
        }
        emit_event(self.out, &event)
    }

    pub(super) fn finish(
        mut self,
        err: &mut dyn Write,
        code: i32,
        selection_stop: Option<&[u8]>,
    ) -> Result<bool, i32> {
        if let Some(exit) = self.failed {
            return Err(exit);
        }
        if self.output != OutputMode::Json {
            if let Some(phase_err) = selection_stop.filter(|phase_err| phase_err.is_empty()) {
                let message = selection_stop_message(self.phase, code, phase_err);
                dx_output::write_stderr(err, format_args!("dx: {CODE_INVALID_SCOPE}: {message}\n"));
            }
            return Ok(true);
        }
        let rest = std::mem::take(&mut self.pending);
        self.line(&rest)?;
        if let Some(phase_err) = selection_stop.filter(|_| !self.saw_error) {
            let message = selection_stop_message(self.phase, code, phase_err);
            if let Ok(event) = self.nest(error_event(
                CODE_INVALID_SCOPE,
                &message,
                None,
                None,
                Some(self.phase),
            )) {
                let held = std::mem::take(&mut self.held);
                self.forward(event)?;
                self.held = held;
            }
        }
        if !self.started {
            let started = self.nest(command_started(self.phase, self.mode));
            emit_built(self.out, started)?;
        }
        if !self.readable {
            let message = format!("{} wrote an unreadable event", self.phase);
            dx_output::write_stderr(
                err,
                format_args!("dx: {CODE_COLLECTION_FAILED}: {message}\n"),
            );
            let correlation = phase_correlation(self.command, self.phase);
            emit_built(
                self.out,
                error_event(
                    CODE_COLLECTION_FAILED,
                    &message,
                    None,
                    None,
                    Some(self.phase),
                )
                .and_then(|event| with_correlation(event, &correlation)),
            )?;
        }
        for event in std::mem::take(&mut self.held) {
            emit_event(self.out, &event)?;
        }
        if !self.finished {
            let finished = self.nest(Ok(command_finished(code, &FinishedCounts::default())));
            emit_built(self.out, finished)?;
        }
        Ok(self.readable)
    }
}

impl Write for Relay<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.failed.is_some() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if self.output != OutputMode::Json {
            return self.out.write(buf).inspect_err(|error| {
                self.failed = Some(stdout_io_code(error));
            });
        }
        self.pending.extend_from_slice(buf);
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            if let Err(exit) = self.line(line.strip_suffix(b"\n").unwrap_or(&line)) {
                self.failed = Some(exit);
                return Err(io::ErrorKind::BrokenPipe.into());
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

pub(super) struct Tee<'a> {
    err: &'a mut dyn Write,
    pub(super) copy: Vec<u8>,
}

impl<'a> Tee<'a> {
    pub(super) fn new(err: &'a mut dyn Write) -> Self {
        Tee {
            err,
            copy: Vec::new(),
        }
    }
}

impl Write for Tee<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.err.write(buf)?;
        self.copy
            .extend_from_slice(buf.get(..written).unwrap_or_default());
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.err.flush()
    }
}
