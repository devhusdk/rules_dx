//! Drives one Firefox over its Marionette control port to exercise a rendered site.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::fmt::{self, Write as _};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use dx_atomic_fs::path_io::{self, PathIoError};
use serde_json::Value;

const LAUNCH_BUDGET: Duration = Duration::from_secs(90);
const LAUNCH_ATTEMPTS: u32 = 3;
const STEP_TIMEOUT: Duration = Duration::from_secs(45);
const LOG_TAIL_LINES: usize = 40;
const POLL: Duration = Duration::from_millis(50);
const ACTIVE_PORT_FILE: &str = "MarionetteActivePort";

/// Why the browser cannot be started or driven.
#[derive(Debug)]
pub enum BrowserError {
    Profile(PathIoError),
    OpenLog(io::Error),
    Spawn {
        firefox: PathBuf,
        source: io::Error,
    },
    Exited {
        status: ExitStatus,
        report: String,
    },
    NoControlPort {
        last: io::Error,
        tail: String,
    },
    ArmSocket(io::Error),
    Send {
        command: String,
        source: io::Error,
    },
    NoReply {
        command: String,
    },
    ParseReply {
        command: String,
        source: serde_json::Error,
    },
    Failed {
        command: String,
        error: Value,
    },
    ReadReply(io::Error),
    FrameLength {
        digits: String,
        source: ParseIntError,
    },
    FrameByte {
        byte: u8,
    },
    NeverLoaded {
        url: String,
    },
    NeverResized {
        width: u64,
        height: u64,
    },
}

impl fmt::Display for BrowserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(error) => write!(f, "browser: {error}"),
            Self::OpenLog(error) => write!(f, "browser: cannot open the browser log: {error}"),
            Self::Spawn { firefox, source } => {
                write!(f, "browser: cannot run '{}': {source}", firefox.display())
            }
            Self::Exited { status, report } => write!(
                f,
                "browser: Firefox exited with {status} before opening its control port\n{report}"
            ),
            Self::NoControlPort { last, tail } => write!(
                f,
                "browser: Firefox opened no control port ({last})\n{tail}"
            ),
            Self::ArmSocket(error) => {
                write!(f, "browser: cannot arm the control socket: {error}")
            }
            Self::Send { command, source } => write!(f, "browser: cannot send {command}: {source}"),
            Self::NoReply { command } => write!(f, "browser: {command} got no reply"),
            Self::ParseReply { command, source } => {
                write!(f, "browser: cannot parse a reply to {command}: {source}")
            }
            Self::Failed { command, error } => write!(f, "browser: {command} failed: {error}"),
            Self::ReadReply(error) => write!(f, "browser: cannot read a reply: {error}"),
            Self::FrameLength { digits, source } => {
                write!(
                    f,
                    "browser: cannot read a frame length '{digits}': {source}"
                )
            }
            Self::FrameByte { byte } => write!(
                f,
                "browser: frame length holds the byte {:?}",
                char::from(*byte)
            ),
            Self::NeverLoaded { url } => write!(f, "browser: '{url}' never finished loading"),
            Self::NeverResized { width, height } => {
                write!(f, "browser: the window never reached {width}x{height}")
            }
        }
    }
}

impl std::error::Error for BrowserError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Profile(error) => Some(error),
            Self::OpenLog(source)
            | Self::Spawn { source, .. }
            | Self::NoControlPort { last: source, .. }
            | Self::ArmSocket(source)
            | Self::Send { source, .. }
            | Self::ReadReply(source) => Some(source),
            Self::ParseReply { source, .. } => Some(source),
            Self::FrameLength { source, .. } => Some(source),
            Self::Exited { .. }
            | Self::NoReply { .. }
            | Self::Failed { .. }
            | Self::FrameByte { .. }
            | Self::NeverLoaded { .. }
            | Self::NeverResized { .. } => None,
        }
    }
}

/// One headless Firefox driven over its Marionette control port.
pub struct Browser {
    child: Child,
    stream: TcpStream,
    profile: PathBuf,
    next_id: u64,
    port: u16,
}

impl Browser {
    /// Starts Firefox on a private profile and opens one control session, retrying a failed launch.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn start(firefox: &Path, profile: &Path) -> Result<Self, BrowserError> {
        let deadline = Instant::now() + LAUNCH_BUDGET;
        let mut attempt = 0;
        loop {
            match Browser::attempt(firefox, profile, attempt, deadline) {
                Ok(browser) => return Ok(browser),
                Err(Launch::Fatal(error)) => return Err(error),
                Err(Launch::Retry(error)) => {
                    attempt += 1;
                    if attempt == LAUNCH_ATTEMPTS || Instant::now() >= deadline {
                        return Err(error);
                    }
                }
            }
        }
    }

    fn attempt(
        firefox: &Path,
        profile: &Path,
        attempt: u32,
        deadline: Instant,
    ) -> Result<Self, Launch> {
        let home = profile.join(format!("attempt-{attempt}"));
        dx_atomic_fs::discard_dir(&home);
        let profile_error = |error| Launch::Fatal(BrowserError::Profile(error));
        path_io::create_dir_all(&home).map_err(profile_error)?;
        path_io::write_scratch(
            &home.join("user.js"),
            b"user_pref(\"marionette.port\", 0);\n",
        )
        .map_err(profile_error)?;
        let log = home.join("browser.log");
        let mut child = Command::new(firefox)
            .arg("--headless")
            .arg("--marionette")
            .arg("--no-remote")
            .arg("--profile")
            .arg(&home)
            .arg("about:blank")
            .env("MOZ_HEADLESS", "1")
            .env("MOZ_DISABLE_CONTENT_SANDBOX", "1")
            .env_remove("DISPLAY")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(
                std::fs::File::create(&log)
                    .map_err(|error| Launch::Fatal(BrowserError::OpenLog(error)))?,
            )
            .spawn()
            .map_err(|source| {
                Launch::Fatal(BrowserError::Spawn {
                    firefox: firefox.to_path_buf(),
                    source,
                })
            })?;
        let control = match wait_for_control(&mut child, &home, &log, deadline) {
            Ok(control) => control,
            Err(error) => {
                drop(child.kill());
                drop(child.wait());
                return Err(error);
            }
        };
        let (stream, port) = control;
        let mut browser = Browser {
            child,
            stream,
            profile: profile.to_path_buf(),
            next_id: 0,
            port,
        };
        if let Err(error) = browser.call(
            "WebDriver:NewSession",
            serde_json::json!({ "capabilities": {} }),
        ) {
            drop(browser);
            return Err(Launch::Retry(error));
        }
        Ok(browser)
    }

    /// Returns the loopback port one browser's control connection is bound to.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Sizes the window to one width and height and waits until the page sees that size.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn set_viewport(&mut self, width: u64, height: u64) -> Result<(), BrowserError> {
        let rect = self.call(
            "WebDriver:SetWindowRect",
            serde_json::json!({ "x": 0, "y": 0, "width": width, "height": height }),
        )?;
        let width = rect.get("width").and_then(Value::as_u64).unwrap_or(width);
        let height = rect.get("height").and_then(Value::as_u64).unwrap_or(height);
        let resized =
            format!("return window.outerWidth === {width} && window.outerHeight === {height};");
        if !wait_for(self, &resized, 30)? {
            return Err(BrowserError::NeverResized { width, height });
        }
        Ok(())
    }

    /// Loads one absolute URL and waits until the page finished loading.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn navigate(&mut self, url: &str) -> Result<(), BrowserError> {
        self.call("WebDriver:Navigate", serde_json::json!({ "url": url }))?;
        let loaded = format!(
            "return document.readyState === 'complete' && window.location.href === {};",
            serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_string())
        );
        if !wait_for(self, &loaded, 60)? {
            return Err(BrowserError::NeverLoaded {
                url: url.to_owned(),
            });
        }
        Ok(())
    }

    /// Sends one key press to whatever element holds the focus; wait for its effect with `wait_for`.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn press(&mut self, key: &str) -> Result<(), BrowserError> {
        let actions = serde_json::json!({
            "actions": [{
                "type": "key",
                "id": "kb",
                "actions": [
                    { "type": "keyDown", "value": key },
                    { "type": "keyUp", "value": key },
                ],
            }],
        });
        self.call("WebDriver:PerformActions", actions)?;
        Ok(())
    }

    /// Runs one script in the page and returns its JSON value.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn script(&mut self, source: &str) -> Result<Value, BrowserError> {
        let value = self.call(
            "WebDriver:ExecuteScript",
            serde_json::json!({ "script": source, "args": [] }),
        )?;
        Ok(value.get("value").cloned().unwrap_or(Value::Null))
    }

    /// Runs one script in the page and returns its value as a string.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn text(&mut self, source: &str) -> Result<String, BrowserError> {
        Ok(self
            .script(source)?
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    /// Runs one script in the page and returns its value as a number.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn count(&mut self, source: &str) -> Result<u64, BrowserError> {
        Ok(self.script(source)?.as_u64().unwrap_or_default())
    }

    /// Runs one script in the page and returns whether it produced a truthy value.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn truthy(&mut self, source: &str) -> Result<bool, BrowserError> {
        Ok(match self.script(source)? {
            Value::Null => false,
            Value::Bool(yes) => yes,
            Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
            Value::String(text) => !text.is_empty() && text != "false",
            Value::Array(items) => !items.is_empty(),
            Value::Object(fields) => !fields.is_empty(),
        })
    }

    fn call(&mut self, name: &str, params: Value) -> Result<Value, BrowserError> {
        self.next_id += 1;
        let message =
            Value::Array(vec![0.into(), self.next_id.into(), name.into(), params]).to_string();
        let framed = format!("{}:{message}", message.len());
        self.stream
            .set_read_timeout(Some(STEP_TIMEOUT))
            .map_err(BrowserError::ArmSocket)?;
        self.stream
            .write_all(framed.as_bytes())
            .and_then(|()| self.stream.flush())
            .map_err(|source| BrowserError::Send {
                command: name.to_owned(),
                source,
            })?;
        loop {
            let frame = self.read_frame()?;
            let Some(frame) = frame else {
                return Err(BrowserError::NoReply {
                    command: name.to_owned(),
                });
            };
            let frame: Value =
                serde_json::from_str(&frame).map_err(|source| BrowserError::ParseReply {
                    command: name.to_owned(),
                    source,
                })?;
            let Some(parts) = frame.as_array() else {
                continue;
            };
            let [kind, id, error, result, ..] = parts.as_slice() else {
                continue;
            };
            if kind.as_u64() != Some(1) {
                continue;
            }
            if id.as_u64() != Some(self.next_id) {
                continue;
            }
            let error = error.clone();
            let result = result.clone();
            if !error.is_null() {
                return Err(BrowserError::Failed {
                    command: name.to_owned(),
                    error,
                });
            }
            return Ok(result);
        }
    }

    fn read_frame(&mut self) -> Result<Option<String>, BrowserError> {
        let mut reader = Frame::default();
        loop {
            let mut byte = [0u8; 1];
            let read = self
                .stream
                .read(&mut byte)
                .map_err(BrowserError::ReadReply)?;
            if read == 0 {
                return Ok(None);
            }
            if let Some(frame) = reader.push(byte[0])? {
                return Ok(Some(frame));
            }
        }
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        drop(self.child.kill());
        drop(self.child.wait());
        dx_atomic_fs::discard_dir(&self.profile);
    }
}

#[derive(Default)]
struct Frame {
    length: Option<usize>,
    digits: String,
    body: Vec<u8>,
}

impl Frame {
    fn push(&mut self, byte: u8) -> Result<Option<String>, BrowserError> {
        if self.length.is_none() {
            if byte == b':' {
                self.length = Some(self.digits.parse::<usize>().map_err(|source| {
                    BrowserError::FrameLength {
                        digits: self.digits.clone(),
                        source,
                    }
                })?);
                self.digits.clear();
                if self.length == Some(0) {
                    self.length = None;
                    return Ok(Some(String::new()));
                }
            } else if byte.is_ascii_digit() {
                self.digits.push(byte as char);
            } else {
                return Err(BrowserError::FrameByte { byte });
            }
        } else {
            self.body.push(byte);
            if self.body.len() == self.length.unwrap_or_default() {
                let body = String::from_utf8_lossy(&self.body).into_owned();
                self.length = None;
                self.body.clear();
                return Ok(Some(body));
            }
        }
        Ok(None)
    }
}

/// Returns the layout of one acquired Firefox tree, or "" when it holds no browser.
#[must_use]
pub fn firefox_executable(root: &Path) -> String {
    let browser = root.join("extracted/firefox/firefox");
    if browser.is_file() {
        return browser.to_string_lossy().into_owned();
    }
    String::new()
}

enum Launch {
    Fatal(BrowserError),
    Retry(BrowserError),
}

fn wait_for_control(
    child: &mut Child,
    home: &Path,
    log: &Path,
    deadline: Instant,
) -> Result<(TcpStream, u16), Launch> {
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(Launch::Fatal(BrowserError::Exited {
                status,
                report: format!("{}{}", missing_libraries(log), log_tail(log)),
            }));
        }
        let last = match active_port(home) {
            Ok(port) => match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => {
                    drop(stream.set_nodelay(true));
                    return Ok((stream, port));
                }
                Err(error) => error,
            },
            Err(error) => error,
        };
        if Instant::now() >= deadline {
            return Err(Launch::Retry(BrowserError::NoControlPort {
                last,
                tail: log_tail(log),
            }));
        }
        thread::sleep(POLL);
    }
}

fn active_port(home: &Path) -> io::Result<u16> {
    let text = std::fs::read_to_string(home.join(ACTIVE_PORT_FILE))?;
    text.trim()
        .parse()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

const MISSING_LIBRARY_MARK: &str = ": cannot open shared object file";

fn missing_libraries(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let mut libraries: Vec<&str> = Vec::new();
    for line in text.lines() {
        let Some((end, _)) = line.split_once(MISSING_LIBRARY_MARK) else {
            continue;
        };
        let name = end.rsplit(": ").next().unwrap_or("").trim();
        if !name.is_empty() && !name.contains(char::is_whitespace) && !libraries.contains(&name) {
            libraries.push(name);
        }
    }
    let mut report = String::new();
    for name in libraries {
        writeln!(
            report,
            "browser: missing shared library {name} (install the system package that provides it)"
        )
        .unwrap_or_default();
    }
    report
}

fn log_tail(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .skip(lines.len().saturating_sub(LOG_TAIL_LINES))
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Polls one script until it reads as yes, or reports how long it waited.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn wait_for(browser: &mut Browser, source: &str, seconds: u64) -> Result<bool, BrowserError> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if browser.truthy(source)? {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(POLL);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
