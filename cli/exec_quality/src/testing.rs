#![expect(
    clippy::expect_used,
    reason = "test fixtures: a failed setup fails the test"
)]

//! Workspace, file system and `quality_result` fixtures for quality phase tests.

use dx_digest::blake3 as digest;
use quality_result::encode_validated;
use quality_result::proto::{self, Capability, Convergence, FileSnapshot, QualityResult, Stage};
use dx_testing::http::{Reply, serve};
use serde_json::json;
use std::io;
use std::path::{Path, PathBuf};

/// The event Bazel ends every complete build event stream with.
pub const LAST_EVENT: &str = r#"{"id":{"buildMetrics":{}},"lastMessage":true}"#;

/// A scratch dir removed when the guard drops.
#[must_use]
pub fn temp_dir(prefix: &str) -> dx_test_scratch::TempDir {
    dx_test_scratch::scratch(&format!("dx-exec-quality-test-{prefix}-")).expect("scratch dir")
}

/// A real file system whose atomic writes fail for the listed paths, so tests
/// can make one apply write fail without chmod tricks.
pub struct FailingWrites {
    failing: Vec<PathBuf>,
}

impl FailingWrites {
    /// Canonicalizes each path, since writes go to the resolved file.
    #[must_use]
    pub fn new(failing: &[PathBuf]) -> Self {
        Self {
            failing: failing
                .iter()
                .map(|path| std::fs::canonicalize(path).unwrap_or_else(|_| path.clone()))
                .collect(),
        }
    }
}

impl dx_apply::FileSystem for FailingWrites {
    fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>> {
        dx_apply::RealFileSystem.read(path)
    }

    fn write_atomic(&self, path: &Path, content: &[u8]) -> io::Result<()> {
        if self.failing.iter().any(|failing| failing == path) {
            return Err(io::Error::other("injected write failure"));
        }
        dx_apply::RealFileSystem.write_atomic(path, content)
    }
}

/// A scratch workspace and temp dir, both removed on drop.
pub struct QualityWorkspace {
    pub workspace: PathBuf,
    pub temp: PathBuf,
    _workspace_guard: dx_test_scratch::TempDir,
    _temp_guard: dx_test_scratch::TempDir,
}

impl QualityWorkspace {
    #[must_use]
    pub fn new(name: &str) -> Self {
        let workspace_guard = temp_dir(&format!("{name}-ws"));
        let temp_guard = temp_dir(&format!("{name}-tmp"));
        Self {
            workspace: workspace_guard.path().to_path_buf(),
            temp: temp_guard.path().to_path_buf(),
            _workspace_guard: workspace_guard,
            _temp_guard: temp_guard,
        }
    }

    pub fn write_source(&self, path: &str, text: &str) {
        write_source(&self.workspace, path, text);
    }

    #[must_use]
    pub fn valid_result(
        &self,
        initial: Vec<proto::Diagnostic>,
        replacements: Vec<proto::FileEdits>,
    ) -> Vec<u8> {
        valid_result(&self.workspace, initial, replacements)
    }

    #[must_use]
    pub fn replacement(&self, replacement: &[u8]) -> proto::FileEdits {
        replacement_in(&self.workspace, "src/a.py", replacement)
    }

    #[must_use]
    pub fn replacement_in(&self, path: &str, replacement: &[u8]) -> proto::FileEdits {
        replacement_in(&self.workspace, path, replacement)
    }
}

pub fn write_source(workspace: &Path, path: &str, text: &str) {
    let full = workspace.join(path);
    std::fs::create_dir_all(full.parent().expect("parent")).expect("parent dir");
    std::fs::write(full, text).expect("write source");
}

/// A result for `src/a.py` that snapshots its current bytes.
#[must_use]
pub fn valid_result(
    workspace: &Path,
    initial: Vec<proto::Diagnostic>,
    replacements: Vec<proto::FileEdits>,
) -> Vec<u8> {
    let original = std::fs::read(workspace.join("src/a.py")).unwrap_or_default();
    result_full(
        initial,
        vec![],
        replacements,
        vec![FileSnapshot {
            path: "src/a.py".to_owned(),
            digest: digest(&original).to_vec(),
        }],
    )
}

/// A result with one fixable warning and snapshot per path, changes in the order given.
#[must_use]
pub fn paths_result(workspace: &Path, paths: &[&str], changes: Vec<proto::FileEdits>) -> Vec<u8> {
    let initial = paths
        .iter()
        .map(|path| {
            diagnostic_with(
                proto::Severity::Warning as i32,
                "lint-tool",
                path,
                "unused",
                true,
            )
        })
        .collect();
    let snapshots = paths
        .iter()
        .map(|path| FileSnapshot {
            path: (*path).to_owned(),
            digest: digest(&std::fs::read(workspace.join(path)).expect("source")).to_vec(),
        })
        .collect();
    result_full(initial, vec![], changes, snapshots)
}

#[must_use]
pub fn result_full(
    initial: Vec<proto::Diagnostic>,
    terminal: Vec<proto::Diagnostic>,
    replacements: Vec<proto::FileEdits>,
    snapshots: Vec<FileSnapshot>,
) -> Vec<u8> {
    let result = QualityResult {
        producer: "//test:corpus".to_owned(),
        capability: Capability::Lint as i32,
        stages: vec![Stage {
            tool_id: "lint-tool".to_owned(),
            class_ids: vec!["python".to_owned()],
            source_paths: vec!["src/a.py".to_owned()],
        }],
        completed_rounds: 1,
        convergence: Convergence::Stable as i32,
        original_snapshot: snapshots.clone(),
        terminal_snapshot: snapshots,
        initial_diagnostics: initial,
        terminal_diagnostics: terminal,
        replacements,
    };
    encode_validated(&result).expect("encode")
}

#[must_use]
pub fn diagnostic(message: &str, fixable: bool) -> proto::Diagnostic {
    diagnostic_with(
        proto::Severity::Warning as i32,
        "lint-tool",
        "src/a.py",
        message,
        fixable,
    )
}

#[must_use]
pub fn diagnostic_with(
    severity: i32,
    tool: &str,
    path: &str,
    message: &str,
    fixable: bool,
) -> proto::Diagnostic {
    proto::Diagnostic {
        severity,
        message: message.to_owned(),
        tool_id: tool.to_owned(),
        rule_id: format!("{tool}/rule"),
        path: path.to_owned(),
        start_byte: Some(0),
        end_byte: Some(1),
        fixable,
        unfixable_reason: String::new(),
        build_dep: None,
        related: Vec::new(),
    }
}

/// A 0..1 edit of `path` against its current digest.
#[must_use]
pub fn replacement_in(workspace: &Path, path: &str, replacement: &[u8]) -> proto::FileEdits {
    let original = std::fs::read(workspace.join(path)).expect("source");
    replacement_at(
        path,
        digest(&original).to_vec(),
        vec![proto::Edit {
            start_byte: 0,
            end_byte: 1,
            replacement: replacement.to_vec(),
        }],
    )
}

#[must_use]
pub fn replacement_at(
    path: &str,
    original_digest: Vec<u8>,
    edits: Vec<proto::Edit>,
) -> proto::FileEdits {
    proto::FileEdits {
        path: path.to_owned(),
        original_digest,
        edits,
    }
}

/// A loopback link host: `/ok` answers 200, every other target 404. Returns the base URL.
#[must_use]
pub fn serve_links() -> String {
    let (base, _) = serve(|target| {
        (target == "/ok").then(|| Reply {
            content_type: "text/plain",
            body: Vec::new(),
        })
    })
    .expect("link server");
    base
}

/// A runner request for `docs/README.md` whose lychee stage uses `lychee`.
#[must_use]
pub fn online_link_request(lychee: &Path) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": 1,
        "producer": "//docs:readme",
        "capability": "lint",
        "stages": [
            {"tool": "lychee", "classes": ["markdown"], "sources": ["docs/README.md"]},
            {"tool": "typos", "classes": ["markdown"], "sources": ["docs/README.md"]},
        ],
        "sources": [{"workspace": "docs/README.md", "exec": "docs/README.md"}],
        "tools": {
            "lychee": {"binary": lychee.to_string_lossy()},
            "typos": {"binary": "external/typos+/typos"},
        },
    }))
    .expect("encode link request")
}
