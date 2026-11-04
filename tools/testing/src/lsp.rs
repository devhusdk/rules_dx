//! A small headless LSP client for tests: framing, a reader thread and deadline-based waits.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Unavailable(String),
    Transport(String),
    Malformed(String),
    Server(i64, String),
    Timeout(String),
    MissingContext(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unavailable(detail) => write!(formatter, "lsp: unavailable server: {detail}"),
            Error::Transport(detail) => write!(formatter, "lsp: transport failed: {detail}"),
            Error::Malformed(detail) => write!(formatter, "lsp: malformed message: {detail}"),
            Error::Server(code, message) => {
                write!(formatter, "lsp: server error {code}: {message}")
            }
            Error::Timeout(what) => write!(formatter, "lsp: timed out waiting for {what}"),
            Error::MissingContext(detail) => {
                write!(formatter, "lsp: missing generated context: {detail}")
            }
        }
    }
}

impl std::error::Error for Error {}

pub const MAX_HEADER_BYTES: usize = 65536;
pub const MAX_BODY_BYTES: usize = 1 << 30;

pub fn encode(message: &serde_json::Value) -> Vec<u8> {
    let body = serde_json::to_vec(message).expect("lsp message serializes");
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(&body);
    out
}

pub fn header_length(head: &[u8]) -> Result<usize, Error> {
    let text = std::str::from_utf8(&head[..head.len() - 4])
        .map_err(|err| Error::Malformed(format!("lsp headers are not UTF-8: {err}")))?;
    for line in text.split("\r\n") {
        let (name, value) = line.split_once(':').unwrap_or((line, ""));
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value.trim().parse::<usize>().map_err(|_| {
                Error::Malformed(format!(
                    "lsp content length is not a number: {}",
                    value.trim()
                ))
            });
        }
    }
    Err(Error::Malformed(
        "lsp headers carry no Content-Length".to_owned(),
    ))
}

pub fn read_message(reader: &mut impl BufRead) -> Result<serde_json::Value, Error> {
    let mut head: Vec<u8> = Vec::new();
    loop {
        if head.len() > MAX_HEADER_BYTES {
            return Err(Error::Malformed("lsp headers exceed 64 KiB".to_owned()));
        }
        let mut byte = [0u8; 1];
        reader
            .read_exact(&mut byte)
            .map_err(|err| Error::Malformed(format!("lsp stream ended inside headers: {err}")))?;
        head.push(byte[0]);
        if head.len() >= 4 && head[head.len() - 4..] == *b"\r\n\r\n" {
            break;
        }
    }
    let length = header_length(&head)?;
    if length > MAX_BODY_BYTES {
        return Err(Error::Malformed(format!(
            "lsp content length {length} exceeds 1 GiB"
        )));
    }
    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|_| Error::Malformed(format!("lsp body truncated: expected {length} bytes")))?;
    serde_json::from_slice(&body)
        .map_err(|err| Error::Malformed(format!("lsp body is not JSON: {err}")))
}

pub fn is_response_for(message: &serde_json::Value, id: i64) -> bool {
    message.get("id") == Some(&serde_json::json!(id)) && message.get("method").is_none()
}

pub fn ok_result(message: &serde_json::Value) -> Result<serde_json::Value, Error> {
    if let Some(error) = message.get("error") {
        let code = error
            .get("code")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(-1);
        let text = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown error");
        return Err(Error::Server(code, text.to_owned()));
    }
    Ok(message
        .get("result")
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}

pub fn diagnostics_for(message: &serde_json::Value, uri: &str) -> Option<serde_json::Value> {
    if message.get("method")?.as_str()? != "textDocument/publishDiagnostics" {
        return None;
    }
    if message.pointer("/params/uri")?.as_str()? != uri {
        return None;
    }
    let diagnostics = message.pointer("/params/diagnostics")?;
    if diagnostics
        .as_array()
        .map(|items| items.is_empty())
        .unwrap_or(true)
    {
        return None;
    }
    Some(diagnostics.clone())
}

pub struct Session {
    stdin: std::process::ChildStdin,
    events: Receiver<Result<serde_json::Value, Error>>,
    next_id: i64,
    child: Child,
}

impl Session {
    pub fn spawn(
        binary: &Path,
        args: &[&str],
        vars: &[(String, String)],
    ) -> Result<Session, Error> {
        let mut command = Command::new(binary);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear();
        for (key, value) in vars {
            command.env(key, value);
        }
        let mut child = command.spawn().map_err(|err| {
            Error::Unavailable(format!("cannot start {}: {err}", binary.display()))
        })?;
        let stdin = child.stdin.take().expect("lsp child keeps stdin");
        let stdout = child.stdout.take().expect("lsp child keeps stdout");
        let mut stderr = child.stderr.take().expect("lsp child keeps stderr");
        thread::spawn(move || {
            let _ = std::io::copy(&mut stderr, &mut std::io::sink());
        });
        let (sender, events) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_message(&mut reader) {
                    Ok(message) => {
                        if sender.send(Ok(message)).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = sender.send(Err(err));
                        break;
                    }
                }
            }
        });
        Ok(Session {
            stdin,
            events,
            next_id: 0,
            child,
        })
    }

    pub fn send(&mut self, message: &serde_json::Value) -> Result<(), Error> {
        let bytes = encode(message);
        self.stdin
            .write_all(&bytes)
            .map_err(|err| Error::Transport(format!("cannot write lsp message: {err}")))?;
        self.stdin
            .flush()
            .map_err(|err| Error::Transport(format!("cannot flush lsp message: {err}")))
    }

    pub fn request(&mut self, method: &str, params: serde_json::Value) -> Result<i64, Error> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))?;
        Ok(id)
    }

    pub fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<(), Error> {
        self.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    pub fn next_event(&self, what: &str, deadline: Instant) -> Result<serde_json::Value, Error> {
        match self
            .events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(Ok(message)) => Ok(message),
            Ok(Err(err)) => Err(err),
            Err(RecvTimeoutError::Timeout) => Err(Error::Timeout(what.to_owned())),
            Err(RecvTimeoutError::Disconnected) => {
                Err(Error::Transport("lsp reader stopped".to_owned()))
            }
        }
    }

    pub fn wait_response(
        &self,
        id: i64,
        what: &str,
        deadline: Instant,
    ) -> Result<serde_json::Value, Error> {
        loop {
            let message = self.next_event(what, deadline)?;
            if is_response_for(&message, id) {
                return Ok(message);
            }
        }
    }

    pub fn wait_diagnostics(
        &self,
        uri: &str,
        what: &str,
        deadline: Instant,
    ) -> Result<serde_json::Value, Error> {
        loop {
            let message = self.next_event(what, deadline)?;
            if let Some(diagnostics) = diagnostics_for(&message, uri) {
                return Ok(diagnostics);
            }
        }
    }

    pub fn initialize(
        &mut self,
        root_uri: &str,
        what: &str,
        deadline: Instant,
    ) -> Result<(), Error> {
        let id = self.request(
            "initialize",
            serde_json::json!({
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {},
                "initializationOptions": null,
            }),
        )?;
        let response = self.wait_response(id, what, deadline)?;
        ok_result(&response)?;
        self.notify("initialized", serde_json::json!({}))
    }

    pub fn shutdown(mut self) -> Result<(), Error> {
        let id = self.request("shutdown", serde_json::Value::Null)?;
        let response =
            self.wait_response(id, "shutdown", Instant::now() + Duration::from_secs(15))?;
        ok_result(&response)?;
        self.notify("exit", serde_json::Value::Null)?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match self
                .child
                .try_wait()
                .map_err(|err| Error::Transport(format!("cannot reap lsp server: {err}")))?
            {
                Some(_) => return Ok(()),
                None if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
                None => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Err(Error::Timeout("lsp server exit".to_owned()));
                }
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

pub fn require_context(directory: &Path, name: &str) -> Result<PathBuf, Error> {
    let path = directory.join(name);
    if path.is_file() {
        Ok(path)
    } else {
        Err(Error::MissingContext(format!(
            "{} has no {name}",
            directory.display()
        )))
    }
}

pub fn write_text(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|_| panic!("create {}", parent.display()));
    }
    std::fs::write(path, text).unwrap_or_else(|_| panic!("write {}", path.display()));
}

pub fn file_uri(path: &Path) -> String {
    assert!(path.is_absolute(), "lsp uri needs an absolute path");
    format!("file://{}", path.display())
}

pub fn offset_to_position(text: &str, offset: usize) -> (u32, u32) {
    let line = text[..offset].matches('\n').count() as u32;
    let start = text[..offset]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    (line, (offset - start) as u32)
}

pub fn scoped_env(root: &Path) -> Vec<(String, String)> {
    let home = root.join("home");
    let tmp = root.join("tmp");
    std::fs::create_dir_all(&home).expect("home scratch exists");
    std::fs::create_dir_all(&tmp).expect("tmp scratch exists");
    vec![
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ("HOME".to_owned(), home.to_string_lossy().into_owned()),
        ("TMPDIR".to_owned(), tmp.to_string_lossy().into_owned()),
    ]
}

pub fn definition_uri(
    session: &mut Session,
    file_uri: &str,
    text: &str,
    needle: &str,
    what: &str,
    deadline: Instant,
) -> Result<serde_json::Value, Error> {
    let offset = text
        .find(needle)
        .unwrap_or_else(|| panic!("fixture holds {needle}"));
    let (line, character) = offset_to_position(text, offset + 2);
    let id = session.request(
        "textDocument/definition",
        serde_json::json!({
            "textDocument": {"uri": file_uri},
            "position": {"line": line, "character": character},
        }),
    )?;
    let response = session.wait_response(id, what, deadline)?;
    ok_result(&response)
}

pub fn poll_definition(
    session: &mut Session,
    file_uri: &str,
    text: &str,
    needle: &str,
    what: &str,
    deadline: Instant,
) -> Result<serde_json::Value, Error> {
    let mut last = serde_json::Value::Null;
    loop {
        match definition_uri(
            session,
            file_uri,
            text,
            needle,
            what,
            Instant::now() + Duration::from_secs(15),
        ) {
            Ok(result) if result != serde_json::Value::Null && result != serde_json::json!([]) => {
                return Ok(result)
            }
            Ok(result) => last = result,
            Err(Error::Timeout(_)) => {}
            Err(err) => return Err(err),
        }
        if Instant::now() >= deadline {
            return Ok(last);
        }
        thread::sleep(Duration::from_secs(5));
    }
}

pub fn first_location(result: &serde_json::Value, what: &str) -> serde_json::Value {
    result
        .as_array()
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or_else(|| panic!("{what} resolves to a location, saw {result}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_frame_round_trip() {
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "initialize",
            "params": {"rootUri": "file:///x"},
        });
        let bytes = encode(&message);
        let back = read_message(&mut &bytes[..]).expect("round trip decodes");
        assert_eq!(back, message);
    }

    #[test]
    fn lsp_frame_rejects_missing_length() {
        let bytes = b"Content-Type: application/json\r\n\r\n{}";
        let err = read_message(&mut &bytes[..]).expect_err("missing length must fail");
        assert!(matches!(err, Error::Malformed(_)), "unexpected {err}");
    }

    #[test]
    fn lsp_frame_rejects_truncated_body() {
        let bytes = b"Content-Length: 42\r\n\r\n{\"jsonrpc\":";
        let err = read_message(&mut &bytes[..]).expect_err("truncated body must fail");
        assert!(matches!(err, Error::Malformed(_)), "unexpected {err}");
    }

    #[test]
    fn lsp_frame_rejects_invalid_json() {
        let bytes = b"Content-Length: 7\r\n\r\nno json";
        let err = read_message(&mut &bytes[..]).expect_err("invalid json must fail");
        assert!(matches!(err, Error::Malformed(_)), "unexpected {err}");
    }

    #[test]
    fn lsp_frames_read_back_to_back() {
        let first = serde_json::json!({"id": 1});
        let second = serde_json::json!({"id": 2});
        let mut bytes = encode(&first);
        bytes.extend(encode(&second));
        let mut reader = &bytes[..];
        assert_eq!(read_message(&mut reader).expect("first"), first);
        assert_eq!(read_message(&mut reader).expect("second"), second);
    }

    #[test]
    fn lsp_frame_accepts_lowercase_header() {
        let bytes = b"content-length: 2\r\n\r\n{}";
        let back = read_message(&mut &bytes[..]).expect("lowercase header decodes");
        assert_eq!(back, serde_json::json!({}));
    }

    #[test]
    fn lsp_frame_rejects_non_numeric_length() {
        let bytes = b"Content-Length: abc\r\n\r\n{}";
        let err = read_message(&mut &bytes[..]).expect_err("bad length must fail");
        assert!(matches!(err, Error::Malformed(_)), "unexpected {err}");
    }

    #[test]
    fn lsp_frame_rejects_oversized_header() {
        let mut bytes = b"X-Pad: ".to_vec();
        bytes.extend(std::iter::repeat(b'a').take(70 * 1024));
        bytes.extend_from_slice(b"\r\n\r\n");
        let err = read_message(&mut &bytes[..]).expect_err("oversized header must fail");
        assert!(matches!(err, Error::Malformed(_)), "unexpected {err}");
    }

    #[test]
    fn lsp_unavailable_server_reports_unavailable() {
        let missing = PathBuf::from("/nonexistent/dx-lsp-probe-server");
        let err = match Session::spawn(&missing, &[], &[]) {
            Err(err) => err,
            Ok(_) => panic!("missing server must not start"),
        };
        assert!(matches!(err, Error::Unavailable(_)), "unexpected {err}");
    }
}
