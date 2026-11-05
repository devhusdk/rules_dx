use super::*;
use std::io::{BufReader, Cursor};
use std::sync::mpsc::Sender;

struct ChannelReader {
    receiver: mpsc::Receiver<Vec<u8>>,
    pending: Cursor<Vec<u8>>,
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let n = self.pending.read(buf)?;
            if n > 0 {
                return Ok(n);
            }
            match self.receiver.recv() {
                Ok(bytes) => self.pending = Cursor::new(bytes),
                Err(_) => return Ok(0),
            }
        }
    }
}

struct ChannelWriter(Sender<Vec<u8>>);

impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = self.0.send(buf.to_vec());
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn pipe() -> (ChannelWriter, ChannelReader) {
    let (sender, receiver) = mpsc::channel();
    (
        ChannelWriter(sender),
        ChannelReader {
            receiver,
            pending: Cursor::new(Vec::new()),
        },
    )
}

fn options() -> Options {
    Options {
        quiet: Duration::from_millis(100),
        timeout: Duration::from_secs(5),
        language_id: "pkl".to_string(),
    }
}

/// Fake server: answers initialize, then publishes `publish(uri)` for each
/// didOpen. Returns the methods it saw.
fn fake_server(
    publish: fn(&str) -> Option<Value>,
    crash_after_initialize: bool,
) -> (
    ChannelWriter,
    ChannelReader,
    std::thread::JoinHandle<Vec<String>>,
) {
    let (to_server, from_client) = pipe();
    let (to_client, from_server) = pipe();
    let handle = std::thread::spawn(move || {
        let mut input = BufReader::new(from_client);
        let mut out = to_client;
        let mut seen = Vec::new();
        while let Ok(Some(message)) = read_message(&mut input) {
            let method = message["method"].as_str().unwrap_or("").to_string();
            seen.push(method.clone());
            if method == "initialize" {
                let _ = write_message(
                    &mut out,
                    &json!({"jsonrpc": "2.0", "id": message["id"], "result": {"capabilities": {}}}),
                );
                let _ = write_message(
                    &mut out,
                    &json!({"jsonrpc": "2.0", "id": 99, "method": "client/registerCapability", "params": {}}),
                );
                if crash_after_initialize {
                    break;
                }
            }
            if method == "textDocument/didOpen" {
                let uri = message["params"]["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("");
                if let Some(note) = publish(uri) {
                    let _ = write_message(&mut out, &note);
                }
            }
            if method == "exit" {
                break;
            }
        }
        seen
    });
    (to_server, from_server, handle)
}

fn temp_file(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lsp_check_{}_{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("a b.pkl");
    std::fs::write(&path, text).unwrap();
    path
}

fn note(uri: &str, diagnostics: Value) -> Option<Value> {
    Some(
        json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
        "params": {"uri": uri, "diagnostics": diagnostics}}),
    )
}

#[test]
fn collects_diagnostics_and_shuts_down() {
    let file = temp_file("collect", "x = 1\n");
    let (writer, reader, handle) = fake_server(
        |uri| {
            note(
                uri,
                json!([{"range": {"start": {"line": 2, "character": 4}},
                    "severity": 2, "code": "unused", "message": "unused local"}]),
            )
        },
        false,
    );
    let found = check(reader, writer, std::slice::from_ref(&file), &options()).unwrap();
    assert_eq!(
        found,
        vec![Diagnostic {
            file: file.clone(),
            line: 3,
            col: 5,
            severity: 2,
            code: "unused".to_string(),
            message: "unused local".to_string(),
        }]
    );
    let seen = handle.join().unwrap();
    assert_eq!(
        seen,
        [
            "initialize",
            "initialized",
            "textDocument/didOpen",
            "",
            "shutdown",
            "exit"
        ]
    );
}

#[test]
fn clean_file_has_no_diagnostics() {
    let file = temp_file("clean", "x = 1\n");
    let (writer, reader, _handle) = fake_server(|uri| note(uri, json!([])), false);
    assert_eq!(check(reader, writer, &[file], &options()).unwrap(), vec![]);
}

#[test]
fn later_publish_replaces_earlier_one() {
    let file = temp_file("replace", "x = 1\n");
    let (writer, reader, _handle) = fake_server(
        |uri| note(uri, json!([{"range": {"start": {}}, "message": "m"}])),
        false,
    );
    let found = check(reader, writer, &[file], &options()).unwrap();
    assert_eq!((found.len(), found[0].line, found[0].severity), (1, 1, 1));
}

#[test]
fn silent_server_fails_closed_on_timeout() {
    let file = temp_file("silent", "x = 1\n");
    let (writer, reader, _handle) = fake_server(|_| None, false);
    let mut short = options();
    short.timeout = Duration::from_millis(300);
    let error = check(reader, writer, &[file], &short).unwrap_err();
    assert!(error.contains("timed out"), "{error}");
}

#[test]
fn crashed_server_fails_closed() {
    let file = temp_file("crash", "x = 1\n");
    let (writer, reader, _handle) = fake_server(|_| None, true);
    let error = check(reader, writer, &[file], &options()).unwrap_err();
    assert!(error.contains("closed"), "{error}");
}

#[test]
fn uri_for_windows_path_has_an_empty_authority() {
    assert_eq!(
        file_uri(Path::new("C:\\a b\\c.pkl")),
        "file:///C:/a%20b/c.pkl"
    );
}

#[test]
fn uri_escapes_spaces() {
    assert_eq!(file_uri(Path::new("/a b/c.pkl")), "file:///a%20b/c.pkl");
}

#[test]
fn message_without_length_is_an_error() {
    let mut input = Cursor::new(b"X: 1\r\n\r\n".to_vec());
    assert!(read_message(&mut input).is_err());
}

#[test]
fn stray_header_line_is_reported() {
    let mut input = Cursor::new(b"Picked up JAVA_TOOL_OPTIONS\r\n\r\n".to_vec());
    let error = read_message(&mut input).unwrap_err();
    assert!(error.to_string().contains("Picked up JAVA_TOOL_OPTIONS"));
}

#[test]
fn content_type_header_is_accepted() {
    let body = b"{}";
    let mut raw = format!("content-type: x\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
    raw.extend_from_slice(body);
    assert!(read_message(&mut Cursor::new(raw)).unwrap().is_some());
}

#[test]
fn vm_init_error_on_stdout_is_reported() {
    let mut out = Vec::new();
    let input = Cursor::new(b"Error occurred during initialization of VM\n".to_vec());
    let error = check(input, &mut out, &[PathBuf::from("/a.pkl")], &options()).unwrap_err();
    assert!(error
        .to_string()
        .contains("Error occurred during initialization of VM"));
}
