use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
enum Error {
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

const MAX_HEADER_BYTES: usize = 65536;
const MAX_BODY_BYTES: usize = 1 << 30;

fn encode(message: &serde_json::Value) -> Vec<u8> {
    let body = serde_json::to_vec(message).expect("lsp message serializes");
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(&body);
    out
}

fn header_length(head: &[u8]) -> Result<usize, Error> {
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

fn read_message(reader: &mut impl BufRead) -> Result<serde_json::Value, Error> {
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

fn is_response_for(message: &serde_json::Value, id: i64) -> bool {
    message.get("id") == Some(&serde_json::json!(id)) && message.get("method").is_none()
}

fn ok_result(message: &serde_json::Value) -> Result<serde_json::Value, Error> {
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

fn diagnostics_for(message: &serde_json::Value, uri: &str) -> Option<serde_json::Value> {
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

struct Session {
    stdin: std::process::ChildStdin,
    events: Receiver<Result<serde_json::Value, Error>>,
    next_id: i64,
    child: Child,
}

impl Session {
    fn spawn(binary: &Path, args: &[&str], vars: &[(String, String)]) -> Result<Session, Error> {
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

    fn send(&mut self, message: &serde_json::Value) -> Result<(), Error> {
        let bytes = encode(message);
        self.stdin
            .write_all(&bytes)
            .map_err(|err| Error::Transport(format!("cannot write lsp message: {err}")))?;
        self.stdin
            .flush()
            .map_err(|err| Error::Transport(format!("cannot flush lsp message: {err}")))
    }

    fn request(&mut self, method: &str, params: serde_json::Value) -> Result<i64, Error> {
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

    fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<(), Error> {
        self.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
    }

    fn next_event(&self, what: &str, deadline: Instant) -> Result<serde_json::Value, Error> {
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

    fn wait_response(
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

    fn wait_diagnostics(
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

    fn initialize(&mut self, root_uri: &str, what: &str, deadline: Instant) -> Result<(), Error> {
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

    fn shutdown(mut self) -> Result<(), Error> {
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

fn tool_path(variable: &str) -> PathBuf {
    let rel =
        std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must name the managed tool"));
    dx_testing::resolve_runfiles(&rel)
}

fn toolchain_pick(variable: &str, suffix: &str) -> PathBuf {
    let rels = std::env::var(variable)
        .unwrap_or_else(|_| panic!("{variable} must list managed toolchain files"));
    let mut found = Vec::new();
    for rel in rels.split_whitespace() {
        let path = dx_testing::resolve_runfiles(rel);
        if path.ends_with(suffix) {
            found.push(path);
        }
    }
    assert!(
        found.len() == 1,
        "{variable} must carry exactly one {suffix}, saw {}",
        found.len()
    );
    found.pop().expect("exactly one match")
}

fn require_context(directory: &Path, name: &str) -> Result<PathBuf, Error> {
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

fn write_text(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|_| panic!("create {}", parent.display()));
    }
    std::fs::write(path, text).unwrap_or_else(|_| panic!("write {}", path.display()));
}

fn file_uri(path: &Path) -> String {
    assert!(path.is_absolute(), "lsp uri needs an absolute path");
    format!("file://{}", path.display())
}

fn offset_to_position(text: &str, offset: usize) -> (u32, u32) {
    let line = text[..offset].matches('\n').count() as u32;
    let start = text[..offset]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    (line, (offset - start) as u32)
}

fn scoped_env(root: &Path) -> Vec<(String, String)> {
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

const CPP_HEADER: &str = "inline int generated_helper(int value) {\n    return value * 2;\n}\n";

const CPP_MAIN: &str = "#include \"probe_generated.h\"\n\nint main() {\n    int value = generated_helper(21);\n    int broken = value + unknown_identifier;\n    return broken;\n}\n";

struct CppFixture {
    root: PathBuf,
    directory: PathBuf,
    main_uri: String,
    header_uri: String,
    main_text: String,
}

fn stage_cpp(tag: &str, clang: &Path) -> CppFixture {
    let root = dx_testing::mkscratch(tag).expect("scratch dir builds");
    let gen = root.join("gen");
    let header = gen.join("probe_generated.h");
    write_text(&header, CPP_HEADER);
    let main = root.join("main.cc");
    write_text(&main, CPP_MAIN);
    let entry = cc_context::Entry {
        directory: root.to_string_lossy().into_owned(),
        file: main.to_string_lossy().into_owned(),
        arguments: vec![
            clang.to_string_lossy().into_owned(),
            "-c".to_owned(),
            main.to_string_lossy().into_owned(),
            "-o".to_owned(),
            root.join("main.o").to_string_lossy().into_owned(),
            "-I".to_owned(),
            gen.to_string_lossy().into_owned(),
            "-std=c++17".to_owned(),
        ],
    };
    let database = serde_json::to_string_pretty(&vec![entry]).expect("compile commands serialize");
    write_text(
        &root.join(cc_context::DATABASE_FILE),
        &format!("{database}\n"),
    );
    CppFixture {
        main_uri: file_uri(&main),
        header_uri: file_uri(&header),
        main_text: CPP_MAIN.to_owned(),
        directory: root.clone(),
        root,
    }
}

const RUST_LIB: &str = "mod typed;\n\n#[cfg(feature = \"dx_probe\")]\npub fn feature_helper(value: i32) -> i32 {\n    value * 2\n}\n\npub fn base_helper(value: i32) -> i32 {\n    value + 1\n}\n\npub fn describe(value: i32) -> i32 {\n    let stepped = base_helper(value);\n    #[cfg(feature = \"dx_probe\")]\n    let stepped = feature_helper(stepped);\n    stepped\n}\n";

const RUST_TYPED: &str =
    "pub fn typed() -> i32 {\n    let mismatch: i32 = \"not an integer\";\n    mismatch\n}\n";

const RUST_BROKEN: &str = "fn broken( {\n    let x =\n}\n";

struct RustFixture {
    root: PathBuf,
    lib_uri: String,
    lib_text: String,
    typed_uri: String,
    broken_uri: String,
}

fn stage_rust(tag: &str, feature: bool) -> RustFixture {
    let root = dx_testing::mkscratch(tag).expect("scratch dir builds");
    let src = root.join("src");
    let lib = src.join("lib.rs");
    write_text(&lib, RUST_LIB);
    let typed = src.join("typed.rs");
    write_text(&typed, RUST_TYPED);
    let broken = src.join("broken.rs");
    write_text(&broken, RUST_BROKEN);
    let cfg: Vec<&str> = if feature {
        vec!["feature=\"dx_probe\""]
    } else {
        Vec::new()
    };
    let project = serde_json::json!({
        "crates": [{
            "display_name": "dx_probe",
            "root_module": lib.to_string_lossy(),
            "edition": "2021",
            "deps": [],
            "cfg": cfg,
            "is_workspace_member": true,
        }],
    });
    let rendered = serde_json::to_string_pretty(&project).expect("rust project serializes");
    write_text(&root.join("rust-project.json"), &format!("{rendered}\n"));
    RustFixture {
        lib_uri: file_uri(&lib),
        lib_text: RUST_LIB.to_owned(),
        typed_uri: file_uri(&typed),
        broken_uri: file_uri(&broken),
        root,
    }
}

fn definition_uri(
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

fn poll_definition(
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

fn first_location(result: &serde_json::Value, what: &str) -> serde_json::Value {
    result
        .as_array()
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or_else(|| panic!("{what} resolves to a location, saw {result}"))
}

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
fn lsp_unavailable_server_reports_unavailable() {
    let missing = PathBuf::from("/nonexistent/dx-lsp-probe-server");
    let err = match Session::spawn(&missing, &[], &[]) {
        Err(err) => err,
        Ok(_) => panic!("missing server must not start"),
    };
    assert!(matches!(err, Error::Unavailable(_)), "unexpected {err}");
}

#[test]
fn clangd_reports_diagnostic_and_definition_for_generated_sources() {
    let clang = tool_path("DX_LSP_CLANG");
    let clangd = tool_path("DX_LSP_CLANGD");
    let fixture = stage_cpp("lsp-clangd", &clang);
    let vars = scoped_env(&fixture.root);
    require_context(&fixture.directory, cc_context::DATABASE_FILE)
        .expect("generated context exists");
    let dir_arg = format!("--compile-commands-dir={}", fixture.directory.display());
    let mut session = Session::spawn(&clangd, &[dir_arg.as_str()], &vars).expect("clangd starts");
    let deadline = Instant::now() + Duration::from_secs(90);
    session
        .initialize(&file_uri(&fixture.directory), "clangd initialize", deadline)
        .expect("clangd initializes");
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": fixture.main_uri,
                    "languageId": "cpp",
                    "version": 1,
                    "text": fixture.main_text,
                },
            }),
        )
        .expect("clangd opens the fixture");
    let diagnostics = session
        .wait_diagnostics(&fixture.main_uri, "clangd diagnostics", deadline)
        .expect("clangd publishes the semantic diagnostic");
    let items = diagnostics.as_array().expect("diagnostics array");
    let offset = fixture
        .main_text
        .find("unknown_identifier")
        .expect("fixture holds the error");
    let (error_line, _) = offset_to_position(&fixture.main_text, offset);
    let hit = items
        .iter()
        .find(|item| {
            item.get("code") == Some(&serde_json::json!("undeclared_var_use"))
                && item
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .map(|message| message.contains("unknown_identifier"))
                    .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("undeclared identifier diagnostic, saw {diagnostics}"));
    assert_eq!(
        hit.pointer("/range/start/line"),
        Some(&serde_json::json!(error_line)),
        "diagnostic lands on the error line"
    );
    let result = definition_uri(
        &mut session,
        &fixture.main_uri,
        &fixture.main_text,
        "generated_helper(21)",
        "clangd definition",
        deadline,
    )
    .expect("clangd answers definition");
    let target = first_location(&result, "generated helper definition");
    assert_eq!(
        target.get("uri").and_then(serde_json::Value::as_str),
        Some(fixture.header_uri.as_str()),
        "definition lands in the generated header"
    );
    session.shutdown().expect("clangd exits cleanly");
}

#[test]
fn clangd_missing_context_is_explicit() {
    let empty = dx_testing::mkscratch("lsp-clangd-empty").expect("scratch dir builds");
    let err =
        require_context(&empty, cc_context::DATABASE_FILE).expect_err("missing context must fail");
    assert!(matches!(err, Error::MissingContext(_)), "unexpected {err}");
    let clang = tool_path("DX_LSP_CLANG");
    let staged = stage_cpp("lsp-clangd-bare", &clang);
    let bare = dx_testing::mkscratch("lsp-clangd-bare-tree").expect("scratch dir builds");
    let main = bare.join("main.cc");
    write_text(&main, &staged.main_text);
    let main_uri = file_uri(&main);
    let vars = scoped_env(&bare);
    let clangd = tool_path("DX_LSP_CLANGD");
    let dir_arg = format!("--compile-commands-dir={}", empty.display());
    let mut session = Session::spawn(&clangd, &[dir_arg.as_str()], &vars).expect("clangd starts");
    let deadline = Instant::now() + Duration::from_secs(90);
    session
        .initialize(&file_uri(&bare), "clangd initialize", deadline)
        .expect("clangd initializes");
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "cpp",
                    "version": 1,
                    "text": staged.main_text,
                },
            }),
        )
        .expect("clangd opens the fixture");
    let result = definition_uri(
        &mut session,
        &main_uri,
        &staged.main_text,
        "generated_helper(21)",
        "clangd definition without context",
        deadline,
    )
    .expect("clangd answers definition");
    assert_eq!(
        result,
        serde_json::json!([]),
        "without the generated context the cross-file fact vanishes"
    );
    session.shutdown().expect("clangd exits cleanly");
}

#[test]
fn clangd_unsupported_request_reports_method_not_found() {
    let root = dx_testing::mkscratch("lsp-clangd-errors").expect("scratch dir builds");
    let vars = scoped_env(&root);
    let clangd = tool_path("DX_LSP_CLANGD");
    let mut session = Session::spawn(&clangd, &[], &vars).expect("clangd starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    session
        .initialize(&file_uri(&root), "clangd initialize", deadline)
        .expect("clangd initializes");
    let id = session
        .request("workspace/dxProbeDoesNotExist", serde_json::json!({}))
        .expect("request sends");
    let response = session
        .wait_response(id, "unknown method answer", deadline)
        .expect("server answers unknown method");
    let err = ok_result(&response).expect_err("unknown method must fail");
    assert!(matches!(err, Error::Server(-32601, _)), "unexpected {err}");
    session.shutdown().expect("clangd exits cleanly");
}

#[test]
fn clangd_wrong_target_reports_server_error() {
    let root = dx_testing::mkscratch("lsp-clangd-target").expect("scratch dir builds");
    let vars = scoped_env(&root);
    let clangd = tool_path("DX_LSP_CLANGD");
    let mut session = Session::spawn(&clangd, &[], &vars).expect("clangd starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    session
        .initialize(&file_uri(&root), "clangd initialize", deadline)
        .expect("clangd initializes");
    let id = session
        .request(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": {"uri": "file:///never-opened-dx-probe.cc"},
                "position": {"line": 0, "character": 0},
            }),
        )
        .expect("request sends");
    let response = session
        .wait_response(id, "unopened file answer", deadline)
        .expect("server answers wrong target");
    let err = ok_result(&response).expect_err("unopened file must fail");
    assert!(matches!(err, Error::Server(_, _)), "unexpected {err}");
    session.shutdown().expect("clangd exits cleanly");
}

#[test]
fn lsp_wait_timeout_reports_cancellation() {
    let root = dx_testing::mkscratch("lsp-quiet").expect("scratch dir builds");
    let vars = scoped_env(&root);
    let clangd = tool_path("DX_LSP_CLANGD");
    let mut session = Session::spawn(&clangd, &[], &vars).expect("clangd starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    session
        .initialize(&file_uri(&root), "clangd initialize", deadline)
        .expect("clangd initializes");
    let quiet = Instant::now() + Duration::from_millis(200);
    let err = session
        .wait_diagnostics(
            "file:///never-opened-dx-probe.cc",
            "quiet diagnostics",
            quiet,
        )
        .expect_err("quiet server must time out");
    assert!(matches!(err, Error::Timeout(_)), "unexpected {err}");
    session.shutdown().expect("clangd exits cleanly");
}

#[test]
fn rust_project_definition_resolves_through_selected_features() {
    let analyzer = toolchain_pick("DX_LSP_TOOLCHAIN_FILES", "bin/rust-analyzer");
    let fixture = stage_rust("lsp-rust-feature", true);
    let vars = scoped_env(&fixture.root);
    require_context(&fixture.root, "rust-project.json").expect("generated context exists");
    let mut session = Session::spawn(&analyzer, &[], &vars).expect("rust-analyzer starts");
    let deadline = Instant::now() + Duration::from_secs(240);
    session
        .initialize(
            &file_uri(&fixture.root),
            "rust-analyzer initialize",
            deadline,
        )
        .expect("rust-analyzer initializes");
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": fixture.lib_uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": fixture.lib_text,
                },
            }),
        )
        .expect("rust-analyzer opens the fixture");
    let result = poll_definition(
        &mut session,
        &fixture.lib_uri,
        &fixture.lib_text,
        "base_helper(value)",
        "rust-analyzer definition",
        deadline,
    )
    .expect("rust-analyzer answers definition");
    let target = first_location(&result, "base helper definition");
    assert_eq!(
        target.get("uri").and_then(serde_json::Value::as_str),
        Some(fixture.lib_uri.as_str()),
        "definition resolves through the project context"
    );
    let offset = fixture
        .lib_text
        .find("feature_helper(stepped)")
        .expect("fixture holds the feature use");
    let (line, character) = offset_to_position(&fixture.lib_text, offset + 2);
    let id = session
        .request(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": {"uri": fixture.lib_uri},
                "position": {"line": line, "character": character},
            }),
        )
        .expect("hover sends");
    let response = session
        .wait_response(id, "rust-analyzer hover", deadline)
        .expect("rust-analyzer answers hover");
    let result = ok_result(&response).expect("hover answers");
    let text = result
        .pointer("/contents/value")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("hover text, saw {result}"));
    assert!(
        text.contains("dx_probe") && text.contains("feature_helper"),
        "hover carries the project identity: {text}"
    );
    session.shutdown().expect("rust-analyzer exits cleanly");
}

#[test]
fn rust_project_hides_feature_gated_items_without_the_feature() {
    let analyzer = toolchain_pick("DX_LSP_TOOLCHAIN_FILES", "bin/rust-analyzer");
    let fixture = stage_rust("lsp-rust-no-feature", false);
    let vars = scoped_env(&fixture.root);
    require_context(&fixture.root, "rust-project.json").expect("generated context exists");
    let mut session = Session::spawn(&analyzer, &[], &vars).expect("rust-analyzer starts");
    let deadline = Instant::now() + Duration::from_secs(240);
    session
        .initialize(
            &file_uri(&fixture.root),
            "rust-analyzer initialize",
            deadline,
        )
        .expect("rust-analyzer initializes");
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": fixture.lib_uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": fixture.lib_text,
                },
            }),
        )
        .expect("rust-analyzer opens the fixture");
    let result = poll_definition(
        &mut session,
        &fixture.lib_uri,
        &fixture.lib_text,
        "base_helper(value)",
        "rust-analyzer definition",
        deadline,
    )
    .expect("rust-analyzer answers definition");
    let target = first_location(&result, "base helper definition");
    assert_eq!(
        target.get("uri").and_then(serde_json::Value::as_str),
        Some(fixture.lib_uri.as_str()),
        "ungated items still resolve"
    );
    let result = definition_uri(
        &mut session,
        &fixture.lib_uri,
        &fixture.lib_text,
        "feature_helper(stepped)",
        "rust-analyzer gated definition",
        deadline,
    )
    .expect("rust-analyzer answers gated definition");
    assert!(
        result == serde_json::Value::Null || result == serde_json::json!([]),
        "without the feature the gated item hides: {result}"
    );
    session.shutdown().expect("rust-analyzer exits cleanly");
}

#[test]
fn rust_project_syntax_diagnostic_publishes_without_check_runner() {
    let analyzer = toolchain_pick("DX_LSP_TOOLCHAIN_FILES", "bin/rust-analyzer");
    let fixture = stage_rust("lsp-rust-syntax", true);
    let vars = scoped_env(&fixture.root);
    require_context(&fixture.root, "rust-project.json").expect("generated context exists");
    let mut session = Session::spawn(&analyzer, &[], &vars).expect("rust-analyzer starts");
    let deadline = Instant::now() + Duration::from_secs(240);
    session
        .initialize(
            &file_uri(&fixture.root),
            "rust-analyzer initialize",
            deadline,
        )
        .expect("rust-analyzer initializes");
    let broken = std::fs::read_to_string(fixture.root.join("src").join("broken.rs"))
        .expect("broken fixture reads");
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": fixture.broken_uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": broken,
                },
            }),
        )
        .expect("rust-analyzer opens the broken file");
    let diagnostics = session
        .wait_diagnostics(&fixture.broken_uri, "rust-analyzer syntax", deadline)
        .expect("rust-analyzer publishes syntax diagnostics");
    let items = diagnostics.as_array().expect("diagnostics array");
    assert!(
        items
            .iter()
            .any(|item| { item.get("code") == Some(&serde_json::json!("syntax-error")) }),
        "syntax error diagnostic, saw {diagnostics}"
    );
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": fixture.typed_uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": RUST_TYPED,
                },
            }),
        )
        .expect("rust-analyzer opens the typed file");
    let quiet = Instant::now() + Duration::from_secs(60);
    let err = session
        .wait_diagnostics(&fixture.typed_uri, "native type diagnostics", quiet)
        .expect_err("without a check runner there are no native type diagnostics");
    assert!(matches!(err, Error::Timeout(_)), "unexpected {err}");
    session.shutdown().expect("rust-analyzer exits cleanly");
}
