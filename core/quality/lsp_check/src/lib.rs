#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Runs a language server over stdio, opens files and collects the
//! diagnostics it publishes, for servers that have no batch mode.

pub mod entry;

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub quiet: Duration,
    pub timeout: Duration,
    pub language_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub file: PathBuf,
    pub line: u64,
    pub col: u64,
    pub severity: u64,
    pub code: String,
    pub message: String,
}

impl Diagnostic {
    pub fn to_json(&self) -> Value {
        json!({
            "file": self.file.to_string_lossy(),
            "line": self.line,
            "col": self.col,
            "severity": self.severity,
            "code": self.code,
            "message": self.message,
        })
    }
}

pub fn file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from("file://");
    if !text.starts_with('/') {
        uri.push('/');
    }
    let drive = text.as_bytes().get(1) == Some(&b':');
    for (i, byte) in text.bytes().enumerate() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                uri.push(byte as char)
            }
            b':' if drive && i == 1 => uri.push(':'),
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

pub fn write_message(out: &mut impl Write, message: &Value) -> std::io::Result<()> {
    let body = message.to_string();
    write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    out.flush()
}

pub fn read_message(input: &mut impl BufRead) -> std::io::Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        } else if !line.to_ascii_lowercase().starts_with("content-type:") {
            let mut text = line.to_string();
            for _ in 0..3 {
                let mut more = String::new();
                if input.read_line(&mut more).unwrap_or(0) == 0 {
                    break;
                }
                text.push_str(" | ");
                text.push_str(more.trim_end());
            }
            let mut end = text.len().min(400);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("server wrote non-LSP output: {}", &text[..end]),
            ));
        }
    }
    let Some(length) = length else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "message without Content-Length",
        ));
    };
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn diagnostics_from(file: &Path, params: &Value) -> Vec<Diagnostic> {
    let items = params["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    items
        .iter()
        .map(|item| Diagnostic {
            file: file.to_path_buf(),
            line: item["range"]["start"]["line"].as_u64().unwrap_or(0) + 1,
            col: item["range"]["start"]["character"].as_u64().unwrap_or(0) + 1,
            severity: item["severity"].as_u64().unwrap_or(1),
            code: match &item["code"] {
                Value::String(code) => code.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            },
            message: item["message"].as_str().unwrap_or("").to_string(),
        })
        .collect()
}

/// Drives one server session. `files` must be absolute paths.
/// Fails if the server exits early or does not publish for every file
/// before the timeout.
pub fn check<R, W>(
    server_out: R,
    mut server_in: W,
    files: &[PathBuf],
    options: &Options,
) -> Result<Vec<Diagnostic>, String>
where
    R: Read + Send + 'static,
    W: Write,
{
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(server_out);
        loop {
            match read_message(&mut reader) {
                Ok(Some(message)) => {
                    if sender.send(Ok(message)).is_err() {
                        return;
                    }
                }
                Ok(None) => return,
                Err(error) => {
                    let _ = sender.send(Err(error.to_string()));
                    return;
                }
            }
        }
    });

    let started = Instant::now();
    let send = |out: &mut W, message: Value| {
        write_message(out, &message).map_err(|error| format!("write to server: {error}"))
    };
    let recv = |wait: Duration| -> Result<Option<Value>, String> {
        let left = options.timeout.saturating_sub(started.elapsed());
        match receiver.recv_timeout(wait.min(left)) {
            Ok(Ok(message)) => Ok(Some(message)),
            Ok(Err(error)) => Err(format!("read from server: {error}")),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err("server closed its output".to_string()),
        }
    };
    let timed_out = || started.elapsed() >= options.timeout;

    send(
        &mut server_in,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "processId": std::process::id(),
            "rootUri": Value::Null,
            "capabilities": {
                "textDocument": {"publishDiagnostics": {}},
                "workspace": {"workspaceFolders": true},
            },
        }}),
    )?;
    loop {
        match recv(Duration::from_millis(100))? {
            Some(message) if message["id"] == 1 && message.get("method").is_none() => {
                if message.get("error").is_some() {
                    return Err(format!("initialize failed: {}", message["error"]));
                }
                break;
            }
            Some(message) => answer_request(&mut server_in, &message)?,
            None if timed_out() => return Err("timed out waiting for initialize".to_string()),
            None => {}
        }
    }
    send(
        &mut server_in,
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    )?;

    let mut by_uri = BTreeMap::new();
    for file in files {
        let text = std::fs::read_to_string(file)
            .map_err(|error| format!("read {}: {error}", file.display()))?;
        let uri = file_uri(file);
        by_uri.insert(uri.clone(), file.clone());
        send(
            &mut server_in,
            json!({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": options.language_id,
                    "version": 1,
                    "text": text,
                },
            }}),
        )?;
    }

    let mut latest: BTreeMap<String, Vec<Diagnostic>> = BTreeMap::new();
    let mut last_publish = Instant::now();
    loop {
        let all_published = latest.len() == by_uri.len();
        if all_published && last_publish.elapsed() >= options.quiet {
            break;
        }
        if timed_out() {
            return Err(format!(
                "timed out: diagnostics published for {} of {} files",
                latest.len(),
                by_uri.len()
            ));
        }
        let Some(message) = recv(Duration::from_millis(50))? else {
            continue;
        };
        if message["method"] == "textDocument/publishDiagnostics" {
            let uri = message["params"]["uri"].as_str().unwrap_or("");
            if let Some(file) = by_uri.get(uri) {
                latest.insert(uri.to_string(), diagnostics_from(file, &message["params"]));
                last_publish = Instant::now();
            }
        } else {
            answer_request(&mut server_in, &message)?;
        }
    }

    let _ = send(
        &mut server_in,
        json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown"}),
    );
    let _ = send(&mut server_in, json!({"jsonrpc": "2.0", "method": "exit"}));
    Ok(latest.into_values().flatten().collect())
}

fn answer_request(out: &mut impl Write, message: &Value) -> Result<(), String> {
    if message.get("method").is_some() && message.get("id").is_some() {
        write_message(
            out,
            &json!({"jsonrpc": "2.0", "id": message["id"], "result": Value::Null}),
        )
        .map_err(|error| format!("write to server: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
