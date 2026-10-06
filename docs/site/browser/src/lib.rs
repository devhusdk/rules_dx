//! Drives one Firefox over its Marionette control port to exercise a rendered site.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

const CONTROL_PORT: u16 = 2828;
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(180);
const STEP_TIMEOUT: Duration = Duration::from_secs(120);
const SETTLE: Duration = Duration::from_millis(600);

/// One headless Firefox driven over its Marionette control port.
pub struct Browser {
    child: Child,
    stream: TcpStream,
    profile: PathBuf,
    next_id: u64,
    port: u16,
}

impl Browser {
    /// Starts Firefox on a private profile and opens one control session.
    ///
    /// Two browsers started at the same moment can pick the same control port,
    /// so a lost race is retried on a fresh port.
    pub fn start(firefox: &Path, profile: &Path) -> Result<Self, String> {
        let mut last = String::new();
        for attempt in 0..3 {
            match Browser::attempt(firefox, profile, attempt) {
                Ok(browser) => return Ok(browser),
                Err(error) => last = error,
            }
        }
        Err(last)
    }

    fn attempt(firefox: &Path, profile: &Path, attempt: u32) -> Result<Self, String> {
        let port = control_port();
        let home = profile.join(format!("attempt-{attempt}"));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).map_err(|error| {
            format!(
                "browser: cannot create the profile '{}': {error}",
                home.display()
            )
        })?;
        std::fs::write(
            home.join("user.js"),
            format!("user_pref(\"marionette.port\", {port});\n"),
        )
        .map_err(|error| format!("browser: cannot write the profile prefs: {error}"))?;
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
                std::fs::File::create(home.join("browser.log"))
                    .map_err(|error| format!("browser: cannot open the browser log: {error}"))?,
            )
            .spawn()
            .map_err(|error| format!("browser: cannot run '{}': {error}", firefox.display()))?;
        let control = match wait_for_control(port) {
            Ok(control) => control,
            Err(error) => {
                let log = std::fs::read_to_string(home.join("browser.log")).unwrap_or_default();
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{error}\n{log}"));
            }
        };
        let mut browser = Browser {
            child,
            stream: control,
            profile: profile.to_path_buf(),
            next_id: 0,
            port,
        };
        if let Err(error) = browser.call(
            "WebDriver:NewSession",
            serde_json::json!({ "capabilities": {} }),
        ) {
            drop(browser);
            return Err(error);
        }
        Ok(browser)
    }

    /// Returns the loopback port one browser's control connection is bound to.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Sizes the viewport to one CSS pixel width and height.
    pub fn set_viewport(&mut self, width: u64, height: u64) -> Result<(), String> {
        self.call(
            "WebDriver:SetWindowRect",
            serde_json::json!({ "x": 0, "y": 0, "width": width, "height": height }),
        )?;
        settle()
    }

    /// Loads one absolute URL and waits until the page finished loading.
    pub fn navigate(&mut self, url: &str) -> Result<(), String> {
        self.call("WebDriver:Navigate", serde_json::json!({ "url": url }))?;
        settle()?;
        let loaded = format!(
            "return document.readyState === 'complete' && window.location.href === {};",
            serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_string())
        );
        if !wait_for(self, &loaded, 60)? {
            return Err(format!("browser: '{url}' never finished loading"));
        }
        settle()
    }

    /// Sends one key press to whatever element holds the focus.
    pub fn press(&mut self, key: &str) -> Result<(), String> {
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
        settle()
    }

    /// Runs one script in the page and returns its JSON value.
    pub fn script(&mut self, source: &str) -> Result<Value, String> {
        let value = self.call(
            "WebDriver:ExecuteScript",
            serde_json::json!({ "script": source, "args": [] }),
        )?;
        Ok(value.get("value").cloned().unwrap_or(Value::Null))
    }

    /// Runs one script in the page and returns its value as a string.
    pub fn text(&mut self, source: &str) -> Result<String, String> {
        Ok(self
            .script(source)?
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    /// Runs one script in the page and returns its value as a number.
    pub fn count(&mut self, source: &str) -> Result<u64, String> {
        Ok(self.script(source)?.as_u64().unwrap_or_default())
    }

    /// Runs one script in the page and returns whether it produced a truthy value.
    pub fn truthy(&mut self, source: &str) -> Result<bool, String> {
        Ok(match self.script(source)? {
            Value::Null => false,
            Value::Bool(yes) => yes,
            Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
            Value::String(text) => !text.is_empty() && text != "false",
            Value::Array(items) => !items.is_empty(),
            Value::Object(fields) => !fields.is_empty(),
        })
    }

    fn call(&mut self, name: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let message = serde_json::json!([0, self.next_id, name, params]).to_string();
        let framed = format!("{}:{message}", message.len());
        self.stream
            .set_read_timeout(Some(STEP_TIMEOUT))
            .map_err(|error| format!("browser: cannot arm the control socket: {error}"))?;
        self.stream
            .write_all(framed.as_bytes())
            .and_then(|()| self.stream.flush())
            .map_err(|error| format!("browser: cannot send {name}: {error}"))?;
        loop {
            let frame = self.read_frame()?;
            let Some(frame) = frame else {
                return Err(format!("browser: {name} got no reply"));
            };
            let frame: Value = serde_json::from_str(&frame)
                .map_err(|error| format!("browser: cannot parse a reply to {name}: {error}"))?;
            let Some(parts) = frame.as_array() else {
                continue;
            };
            if parts.len() < 4 || parts[0].as_u64() != Some(1) {
                continue;
            }
            if parts[1].as_u64() != Some(self.next_id) {
                continue;
            }
            let error = parts[2].clone();
            let result = parts[3].clone();
            if !error.is_null() {
                return Err(format!("browser: {name} failed: {error}"));
            }
            return Ok(result);
        }
    }

    fn read_frame(&mut self) -> Result<Option<String>, String> {
        let mut reader = Frame::default();
        loop {
            let mut byte = [0u8; 1];
            let read = self
                .stream
                .read(&mut byte)
                .map_err(|error| format!("browser: cannot read a reply: {error}"))?;
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
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

/// Assembles one length-prefixed Marionette frame one byte at a time.
#[derive(Default)]
struct Frame {
    length: Option<usize>,
    digits: String,
    body: String,
}

impl Frame {
    fn push(&mut self, byte: u8) -> Result<Option<String>, String> {
        match self.length {
            None => {
                if byte == b':' {
                    self.length = Some(self.digits.parse::<usize>().map_err(|error| {
                        format!(
                            "browser: cannot read a frame length '{}': {error}",
                            self.digits
                        )
                    })?);
                    self.digits.clear();
                    if self.length == Some(0) {
                        self.length = None;
                        return Ok(Some(String::new()));
                    }
                } else if byte.is_ascii_digit() {
                    self.digits.push(byte as char);
                } else {
                    return Err(format!(
                        "browser: frame length holds the byte {:?}",
                        byte as char
                    ));
                }
            }
            Some(_) => {
                self.body.push(byte as char);
                if self.body.len() == self.length.unwrap_or_default() {
                    let body = self.body.clone();
                    self.length = None;
                    self.body.clear();
                    return Ok(Some(body));
                }
            }
        }
        Ok(None)
    }
}

/// Returns the loopback port the next browser should bind its control socket to.
pub fn control_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .map(|listener| listener.local_addr().map(|addr| addr.port()).unwrap_or(0))
        .unwrap_or(CONTROL_PORT)
}

/// Returns the layout of one acquired Firefox tree, or "" when it holds no browser.
pub fn firefox_executable(root: &Path) -> String {
    let browser = root.join("extracted/firefox/firefox");
    if browser.is_file() {
        return browser.to_string_lossy().into_owned();
    }
    String::new()
}

fn wait_for_control(port: u16) -> Result<TcpStream, String> {
    let deadline = Instant::now() + LAUNCH_TIMEOUT;
    let mut last = String::from("no attempt");
    while Instant::now() < deadline {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Err(error) => {
                last = error.to_string();
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
    Err(format!(
        "browser: Firefox opened no control port on {port} ({last})"
    ))
}

fn settle() -> Result<(), String> {
    thread::sleep(SETTLE);
    Ok(())
}

/// Polls one script until it reads as yes, or reports how long it waited.
pub fn wait_for(browser: &mut Browser, source: &str, seconds: u64) -> Result<bool, String> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if browser.truthy(source)? {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
