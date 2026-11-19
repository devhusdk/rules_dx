//! Release candidate identity, source archive, local registry and tag checks.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::io::{self, BufRead};
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::string::FromUtf8Error;

use dx_atomic_fs::path_io::{self, PathIoError};
use dx_process::capture::{
    self, Captured, DEFAULT_TIMEOUT, RunError, run_captured, run_captured_with_input,
};

use dx_fingerprint::FingerprintError;

use crate::{
    BCR_WANT_MODULE, BcrError, GitState, RELEASE_REMOTE, RELEASE_REPOSITORY, bcr_candidate_error,
    bcr_candidate_version_error, bcr_integrity_of_digest, bcr_source_url, bcr_strip_prefix,
    modules::{ModuleError, local_override_error, module_version_error},
    pretty_json, render_bcr_source_for_digest, write_bcr_source_for_archive,
};

/// One release candidate: a `SemVer` version at one full commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    version: String,
    commit: String,
}

impl Candidate {
    /// Returns a candidate for a publishable version and a full lowercase commit.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn new(version: &str, commit: &str) -> Result<Self, CandidateError> {
        bcr_candidate_version_error(version)?;
        if !dx_digest::is_lower_hex(commit, 20) {
            return Err(CandidateError::InvalidCommit {
                commit: commit.to_owned(),
            });
        }
        Ok(Self {
            version: version.to_owned(),
            commit: commit.to_owned(),
        })
    }

    /// Returns the `SemVer` version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns the full commit SHA.
    #[must_use]
    pub fn commit(&self) -> &str {
        &self.commit
    }

    /// Returns the release tag.
    #[must_use]
    pub fn tag(&self) -> String {
        format!("v{}", self.version)
    }

    /// Returns the full tag ref.
    #[must_use]
    pub fn reference(&self) -> String {
        format!("refs/tags/{}", self.tag())
    }

    /// Returns the ls-remote patterns for the tag and its peeled commit.
    #[must_use]
    pub fn ls_remote_patterns(&self) -> [String; 2] {
        let reference = self.reference();
        let peeled = format!("{reference}^{{}}");
        [reference, peeled]
    }

    /// Returns the archive strip prefix.
    #[must_use]
    pub fn strip_prefix(&self) -> String {
        bcr_strip_prefix(BCR_WANT_MODULE, &self.version)
    }

    /// Returns the release asset URL.
    #[must_use]
    pub fn url(&self) -> String {
        bcr_source_url(BCR_WANT_MODULE, &self.version)
    }

    /// Returns the canonical release remote.
    #[must_use]
    pub fn remote(&self) -> &'static str {
        RELEASE_REMOTE
    }
}

/// Digest and SRI integrity of one built source archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceArchive {
    pub digest: String,
    pub integrity: String,
}

/// A failed git command.
#[derive(Debug)]
pub enum GitError {
    /// git could not be run to completion.
    Run { command: String, source: RunError },
    /// git exited with a failure.
    Exit { command: String, detail: String },
    /// git printed output that is not UTF-8.
    NotUtf8 {
        command: String,
        source: FromUtf8Error,
    },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Run { command, source } => write!(f, "{command} failed: {source}"),
            Self::Exit { command, detail } => write!(f, "{command} failed: {detail}"),
            Self::NotUtf8 { command, .. } => write!(f, "{command} printed non-UTF-8 output"),
        }
    }
}

impl std::error::Error for GitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Run { source, .. } => Some(source),
            Self::NotUtf8 { source, .. } => Some(source),
            Self::Exit { .. } => None,
        }
    }
}

/// A failed candidate identity, archive, registry or tag check.
#[derive(Debug)]
pub enum CandidateError {
    /// A file operation failed.
    Io(PathIoError),
    /// A git command failed.
    Git(GitError),
    /// A git command reading the local release state failed.
    GitState(GitError),
    /// A file could not be hashed.
    Hash {
        path: PathBuf,
        source: io::Error,
    },
    /// A BCR source metadata check failed.
    Bcr(BcrError),
    /// A `MODULE.bazel` release check failed.
    Module(ModuleError),
    /// A registry document could not be rendered.
    Render(FingerprintError),
    InvalidCommit {
        commit: String,
    },
    CommitMissing {
        commit: String,
        repo: PathBuf,
        source: Box<GitError>,
    },
    CommitResolves {
        commit: String,
        resolved: String,
        repo: PathBuf,
    },
    MalformedTreeRecord {
        record: String,
    },
    NotRegularFile {
        path: String,
        kind: String,
        mode: String,
    },
    NewlineInPath {
        path: String,
    },
    EmptyTree {
        commit: String,
    },
    ScratchDir(io::Error),
    BatchRead(io::Error),
    BlobMissing {
        commit: String,
        path: String,
    },
    BadSize {
        header: String,
        source: ParseIntError,
    },
    BadHeader {
        header: String,
        path: String,
    },
    ShortRead {
        path: String,
        source: io::Error,
    },
    SizeMismatch {
        path: String,
    },
    NoPins,
    OutsidePrefix {
        name: String,
        prefix: String,
    },
    ExtraMember {
        name: String,
    },
    MemberMode {
        name: String,
        mode: u32,
        want: u32,
    },
    MemberDiffers {
        name: String,
        commit: String,
    },
    DroppedMember {
        name: String,
    },
    Immutable {
        path: PathBuf,
    },
    OutputDirRelative {
        path: PathBuf,
    },
    SourceJsonWrite(io::Error),
    IntegrityDiffers,
    SourceJsonNotUtf8(FromUtf8Error),
    ModuleCalls {
        count: usize,
    },
    ModuleNotClosed,
    ModuleName {
        named: Vec<String>,
        want: String,
    },
    VersionFields {
        count: usize,
    },
    VersionOutOfRange,
    RegistryDirRelative {
        path: PathBuf,
    },
    MemberMissing {
        member: String,
    },
    MemberRead {
        member: String,
        source: io::Error,
    },
    MemberNotUtf8 {
        member: String,
        source: FromUtf8Error,
    },
    NotHttps {
        url: String,
    },
    MirrorDigest {
        path: PathBuf,
        have: String,
        want: String,
    },
    MalformedLsRemote {
        line: String,
    },
    AmbiguousLsRemote {
        reference: String,
    },
    NoRemoteTag {
        tag: String,
    },
    MalformedForEachRef {
        line: String,
    },
    AmbiguousForEachRef {
        reference: String,
    },
    BadHead {
        head: String,
    },
}

impl fmt::Display for CandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(f, "candidate: {source}"),
            Self::Git(source) => write!(f, "candidate: {source}"),
            Self::GitState(source) => write!(f, "release: cannot read git state: {source}"),
            Self::Hash { path, source } => {
                write!(f, "candidate: cannot hash {}: {source}", path.display())
            }
            Self::Bcr(source) => source.fmt(f),
            Self::Module(source) => source.fmt(f),
            Self::Render(source) => write!(f, "cannot render JSON: {source}"),
            Self::InvalidCommit { commit } => write!(
                f,
                "candidate: invalid commit {commit:?}: want a full 40-char lowercase SHA"
            ),
            Self::CommitMissing {
                commit,
                repo,
                source,
            } => write!(
                f,
                "candidate: commit {commit} is not in {}: {source}",
                repo.display()
            ),
            Self::CommitResolves {
                commit,
                resolved,
                repo,
            } => write!(
                f,
                "candidate: commit {commit} resolves to {resolved:?} in {}",
                repo.display()
            ),
            Self::MalformedTreeRecord { record } => {
                write!(f, "candidate: malformed git ls-tree record {record:?}")
            }
            Self::NotRegularFile { path, kind, mode } => write!(
                f,
                "candidate: {path} is a {kind} with mode {mode}; only regular files are archived"
            ),
            Self::NewlineInPath { path } => {
                write!(f, "candidate: path {path:?} contains a newline")
            }
            Self::EmptyTree { commit } => write!(f, "candidate: commit {commit} has an empty tree"),
            Self::ScratchDir(source) => write!(f, "candidate: create scratch dir: {source}"),
            Self::BatchRead(source) => write!(f, "candidate: git cat-file output: {source}"),
            Self::BlobMissing { commit, path } => {
                write!(f, "candidate: git cat-file: {commit}:{path} is missing")
            }
            Self::BadSize { header, .. } => {
                write!(f, "candidate: git cat-file bad size line {header:?}")
            }
            Self::BadHeader { header, path } => {
                write!(
                    f,
                    "candidate: git cat-file bad header {header:?} for {path}"
                )
            }
            Self::ShortRead { path, source } => {
                write!(f, "candidate: git cat-file short read for {path}: {source}")
            }
            Self::SizeMismatch { path } => {
                write!(f, "candidate: git cat-file size mismatch for {path}")
            }
            Self::NoPins => write!(f, "candidate: commit has no {PINS_PATH}"),
            Self::OutsidePrefix { name, prefix } => write!(
                f,
                "candidate: archive member {name} is outside strip prefix {prefix}"
            ),
            Self::ExtraMember { name } => write!(f, "candidate: archive has extra member {name}"),
            Self::MemberMode { name, mode, want } => write!(
                f,
                "candidate: archive member {name} has mode {mode:o}, want {want:o}"
            ),
            Self::MemberDiffers { name, commit } => write!(
                f,
                "candidate: archive member {name} differs from commit {commit}"
            ),
            Self::DroppedMember { name } => write!(f, "candidate: archive drops member {name}"),
            Self::Immutable { path } => write!(
                f,
                "candidate: {} already exists with different bytes (release assets are immutable)",
                path.display()
            ),
            Self::OutputDirRelative { path } => write!(
                f,
                "candidate: output directory {} must be absolute",
                path.display()
            ),
            Self::SourceJsonWrite(source) => {
                write!(f, "candidate: cannot write source.json: {source}")
            }
            Self::IntegrityDiffers => {
                f.write_str("candidate: source.json integrity differs from the archive")
            }
            Self::SourceJsonNotUtf8(source) => {
                write!(f, "candidate: source.json is not UTF-8: {source}")
            }
            Self::ModuleCalls { count } => write!(
                f,
                "candidate: MODULE.bazel has {count} module() calls, want exactly one"
            ),
            Self::ModuleNotClosed => {
                f.write_str("candidate: MODULE.bazel module() call is not closed")
            }
            Self::ModuleName { named, want } => write!(
                f,
                "candidate: MODULE.bazel module() name is {named:?}, want [\"{want}\"]"
            ),
            Self::VersionFields { count } => write!(
                f,
                "candidate: MODULE.bazel module() has {count} version fields, want exactly one"
            ),
            Self::VersionOutOfRange => {
                f.write_str("candidate: MODULE.bazel module() version field is out of range")
            }
            Self::RegistryDirRelative { path } => write!(
                f,
                "candidate: registry dir {} must be absolute",
                path.display()
            ),
            Self::MemberMissing { member } => write!(f, "candidate: cannot read {member}"),
            Self::MemberRead { member, source } => {
                write!(f, "candidate: cannot read {member}: {source}")
            }
            Self::MemberNotUtf8 { member, .. } => write!(f, "candidate: {member} is not UTF-8"),
            Self::NotHttps { url } => write!(f, "candidate: url {url} is not https"),
            Self::MirrorDigest { path, have, want } => write!(
                f,
                "candidate: mirrored archive {} has sha256 {have}, want {want}",
                path.display()
            ),
            Self::MalformedLsRemote { line } => {
                write!(f, "release: malformed git ls-remote line {line:?}")
            }
            Self::AmbiguousLsRemote { reference } => write!(
                f,
                "release: ambiguous git ls-remote output: {reference} listed twice"
            ),
            Self::NoRemoteTag { tag } => write!(f, "release: remote has no tag {tag}"),
            Self::MalformedForEachRef { line } => {
                write!(f, "release: malformed git for-each-ref line {line:?}")
            }
            Self::AmbiguousForEachRef { reference } => write!(
                f,
                "release: ambiguous git for-each-ref output: {reference} listed twice"
            ),
            Self::BadHead { head } => write!(f, "release: cannot read git state: HEAD is {head:?}"),
        }
    }
}

impl std::error::Error for CandidateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Git(source) | Self::GitState(source) => Some(source),
            Self::CommitMissing { source, .. } => Some(source.as_ref()),
            Self::Hash { source, .. }
            | Self::ShortRead { source, .. }
            | Self::MemberRead { source, .. }
            | Self::ScratchDir(source)
            | Self::BatchRead(source)
            | Self::SourceJsonWrite(source) => Some(source),
            Self::BadSize { source, .. } => Some(source),
            Self::SourceJsonNotUtf8(source) | Self::MemberNotUtf8 { source, .. } => Some(source),
            Self::Bcr(source) => Some(source),
            Self::Module(source) => Some(source),
            Self::Render(source) => Some(source),
            Self::InvalidCommit { .. }
            | Self::CommitResolves { .. }
            | Self::MalformedTreeRecord { .. }
            | Self::NotRegularFile { .. }
            | Self::NewlineInPath { .. }
            | Self::EmptyTree { .. }
            | Self::BlobMissing { .. }
            | Self::BadHeader { .. }
            | Self::SizeMismatch { .. }
            | Self::NoPins
            | Self::OutsidePrefix { .. }
            | Self::ExtraMember { .. }
            | Self::MemberMode { .. }
            | Self::MemberDiffers { .. }
            | Self::DroppedMember { .. }
            | Self::Immutable { .. }
            | Self::OutputDirRelative { .. }
            | Self::IntegrityDiffers
            | Self::ModuleCalls { .. }
            | Self::ModuleNotClosed
            | Self::ModuleName { .. }
            | Self::VersionFields { .. }
            | Self::VersionOutOfRange
            | Self::RegistryDirRelative { .. }
            | Self::MemberMissing { .. }
            | Self::NotHttps { .. }
            | Self::MirrorDigest { .. }
            | Self::MalformedLsRemote { .. }
            | Self::AmbiguousLsRemote { .. }
            | Self::NoRemoteTag { .. }
            | Self::MalformedForEachRef { .. }
            | Self::AmbiguousForEachRef { .. }
            | Self::BadHead { .. } => None,
        }
    }
}

impl From<BcrError> for CandidateError {
    fn from(source: BcrError) -> Self {
        Self::Bcr(source)
    }
}

impl From<ModuleError> for CandidateError {
    fn from(source: ModuleError) -> Self {
        Self::Module(source)
    }
}

impl From<PathIoError> for CandidateError {
    fn from(source: PathIoError) -> Self {
        Self::Io(source)
    }
}

fn hash_file(path: &Path) -> Result<String, CandidateError> {
    dx_digest::sha256_file_hex(path).map_err(|source| CandidateError::Hash {
        path: path.to_path_buf(),
        source,
    })
}

/// Returns stdout of a git command, or its failure as an error.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn git_output_of(
    args: &[&str],
    output: Result<Captured, RunError>,
) -> Result<String, GitError> {
    let command = format!("git {}", args.join(" "));
    match output {
        Ok(out) if out.success() => {
            String::from_utf8(out.stdout).map_err(|source| GitError::NotUtf8 { command, source })
        }
        Ok(out) => {
            let stderr = out.stderr_text();
            let detail = if stderr.is_empty() {
                match out.code {
                    Some(code) => format!("exit status: {code}"),
                    None => "terminated by signal".to_owned(),
                }
            } else {
                stderr
            };
            Err(GitError::Exit { command, detail })
        }
        Err(source) => Err(GitError::Run { command, source }),
    }
}

/// Runs git in a directory and returns its stdout.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn git_in(dir: Option<&Path>, args: &[&str]) -> Result<String, GitError> {
    let mut cmd = capture::git(dir.unwrap_or(Path::new(".")));
    cmd.args(args);
    git_output_of(args, run_captured(&mut cmd, DEFAULT_TIMEOUT))
}

/// # Errors
///
/// Returns an error when the operation fails.
pub fn ls_remote_output(
    dir: Option<&Path>,
    remote: &str,
    candidate: &Candidate,
) -> Result<String, GitError> {
    let [reference, peeled] = candidate.ls_remote_patterns();
    git_in(dir, &["ls-remote", "--tags", remote, &reference, &peeled])
}

const READ_CONFIG: [&str; 4] = ["-c", "core.autocrlf=false", "-c", "core.eol=lf"];

fn git_read(repo: &Path, args: &[&str]) -> Result<String, GitError> {
    let mut full: Vec<&str> = READ_CONFIG.to_vec();
    full.extend_from_slice(args);
    git_in(Some(repo), &full)
}

struct TreeFile {
    path: String,
    mode: u32,
    data: Vec<u8>,
}

fn commit_tree(repo: &Path, candidate: &Candidate) -> Result<Vec<TreeFile>, CandidateError> {
    let commit = candidate.commit();
    let resolved = git_read(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{commit}^{{commit}}"),
        ],
    )
    .map_err(|source| CandidateError::CommitMissing {
        commit: commit.to_owned(),
        repo: repo.to_path_buf(),
        source: Box::new(source),
    })?;
    if resolved.trim() != commit {
        return Err(CandidateError::CommitResolves {
            commit: commit.to_owned(),
            resolved: resolved.trim().to_owned(),
            repo: repo.to_path_buf(),
        });
    }
    let listing = git_read(repo, &["ls-tree", "-r", "-z", "--full-tree", commit])
        .map_err(CandidateError::Git)?;
    let malformed = |record: &str| CandidateError::MalformedTreeRecord {
        record: record.to_owned(),
    };
    let mut entries: Vec<(String, u32)> = Vec::new();
    for record in listing.split('\0').filter(|record| !record.is_empty()) {
        let (meta, path) = record.split_once('\t').ok_or_else(|| malformed(record))?;
        let mode = match meta.split(' ').collect::<Vec<_>>().as_slice() {
            ["100644", "blob", _] => 0o644,
            ["100755", "blob", _] => 0o755,
            [mode, kind, _] => {
                return Err(CandidateError::NotRegularFile {
                    path: path.to_owned(),
                    kind: (*kind).to_owned(),
                    mode: (*mode).to_owned(),
                });
            }
            _ => return Err(malformed(record)),
        };
        if path.contains('\n') {
            return Err(CandidateError::NewlineInPath {
                path: path.to_owned(),
            });
        }
        entries.push((path.to_owned(), mode));
    }
    if entries.is_empty() {
        return Err(CandidateError::EmptyTree {
            commit: commit.to_owned(),
        });
    }
    let blobs = cat_blobs(repo, commit, &entries)?;
    Ok(entries
        .into_iter()
        .zip(blobs)
        .map(|((path, mode), data)| TreeFile { path, mode, data })
        .collect())
}

const FILTER_ATTRS: [&str; 4] = ["eol", "filter", "ident", "working-tree-encoding"];

fn changes_on_checkout(attr: &[u8], value: &[u8]) -> bool {
    match attr {
        b"eol" => value == b"crlf",
        b"ident" => value == b"set",
        _ => value != b"unspecified" && value != b"unset",
    }
}

struct CommitIndex {
    dir: PathBuf,
    index: PathBuf,
    tree: PathBuf,
    out: PathBuf,
}

impl CommitIndex {
    fn new(repo: &Path, commit: &str) -> Result<Self, CandidateError> {
        let dir = dx_deploy_tools::tempfile_stage_dir(&format!("dx-candidate-{commit}-"))
            .map_err(CandidateError::ScratchDir)?;
        let scratch = Self {
            index: dir.join("index"),
            tree: dir.join("tree"),
            out: dir.join("out"),
            dir,
        };
        for path in [&scratch.tree, &scratch.out] {
            path_io::create_dir_all(path)?;
        }
        scratch.pipe(repo, &["read-tree", commit], &[], |_| Ok(()))?;
        Ok(scratch)
    }

    fn pipe<T>(
        &self,
        repo: &Path,
        args: &[&str],
        input: &[u8],
        read: impl FnOnce(&[u8]) -> Result<T, CandidateError>,
    ) -> Result<T, CandidateError> {
        let work_tree = format!("--work-tree={}", self.tree.display());
        let mut full = vec![work_tree.as_str()];
        full.extend_from_slice(&READ_CONFIG);
        full.extend_from_slice(args);
        let mut cmd = capture::git(repo);
        cmd.env("GIT_INDEX_FILE", &self.index).args(&full);
        git_run(&mut cmd, &full, input, read)
    }
}

impl Drop for CommitIndex {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.dir) {
            eprintln!("candidate: remove {}: {error}", self.dir.display());
        }
    }
}

fn git_run<T>(
    cmd: &mut std::process::Command,
    args: &[&str],
    input: &[u8],
    read: impl FnOnce(&[u8]) -> Result<T, CandidateError>,
) -> Result<T, CandidateError> {
    let command = format!("git {}", args.join(" "));
    let output = match run_captured_with_input(cmd, Some(input), DEFAULT_TIMEOUT) {
        Ok(output) => output,
        Err(source) => return Err(CandidateError::Git(GitError::Run { command, source })),
    };
    if !output.success() {
        return Err(CandidateError::Git(GitError::Exit {
            command,
            detail: output.stderr_text().trim().to_owned(),
        }));
    }
    read(&output.stdout)
}

fn git_pipe<T>(
    repo: &Path,
    args: &[&str],
    input: &[u8],
    read: impl FnOnce(&[u8]) -> Result<T, CandidateError>,
) -> Result<T, CandidateError> {
    let mut cmd = capture::git(repo);
    cmd.args(args);
    git_run(&mut cmd, args, input, read)
}

fn nul_separated(paths: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<u8> {
    let mut input = Vec::new();
    for path in paths {
        input.extend_from_slice(path.as_ref().as_bytes());
        input.push(0);
    }
    input
}

fn filtered_paths(
    repo: &Path,
    index: &CommitIndex,
    entries: &[(String, u32)],
) -> Result<Vec<bool>, CandidateError> {
    let input = nul_separated(entries.iter().map(|(path, _)| path));
    let mut args = vec!["check-attr", "--cached", "-z", "--stdin"];
    args.extend_from_slice(&FILTER_ATTRS);
    let raw = index.pipe(repo, &args, &input, |stdout| Ok(stdout.to_vec()))?;
    let fields: Vec<&[u8]> = raw.split(|byte| *byte == 0).collect();
    let mut flagged = std::collections::BTreeSet::new();
    for record in fields.chunks(3) {
        if let [path, attr, value] = record
            && changes_on_checkout(attr, value)
        {
            flagged.insert(*path);
        }
    }
    Ok(entries
        .iter()
        .map(|(path, _)| flagged.contains(path.as_bytes()))
        .collect())
}

fn cat_blobs(
    repo: &Path,
    commit: &str,
    entries: &[(String, u32)],
) -> Result<Vec<Vec<u8>>, CandidateError> {
    let mut input = String::new();
    for (path, _) in entries {
        writeln!(input, "{commit}:{path}").ok();
    }
    let mut blobs = git_pipe(repo, &["cat-file", "--batch"], input.as_bytes(), |stdout| {
        read_batch(stdout, commit, entries)
    })?;
    let index = CommitIndex::new(repo, commit)?;
    let flagged = filtered_paths(repo, &index, entries)?;
    let checkout: Vec<&str> = entries
        .iter()
        .zip(&flagged)
        .filter(|(_, filter)| **filter)
        .map(|((path, _), _)| path.as_str())
        .collect();
    if checkout.is_empty() {
        return Ok(blobs);
    }
    let prefix = format!("--prefix={}/", index.out.display());
    index.pipe(
        repo,
        &["checkout-index", &prefix, "-z", "--stdin"],
        &nul_separated(&checkout),
        |_| Ok(()),
    )?;
    for ((blob, (path, _)), filter) in blobs.iter_mut().zip(entries).zip(flagged) {
        if filter {
            *blob = path_io::read(&index.out.join(path))?;
        }
    }
    Ok(blobs)
}

fn read_batch(
    mut reader: impl BufRead,
    commit: &str,
    entries: &[(String, u32)],
) -> Result<Vec<Vec<u8>>, CandidateError> {
    let mut blobs = Vec::with_capacity(entries.len());
    for (path, _) in entries {
        let mut header = String::new();
        reader
            .read_line(&mut header)
            .map_err(CandidateError::BatchRead)?;
        let line = header.trim_end_matches('\n');
        if line.ends_with(" missing") || line.ends_with(" ambiguous") {
            return Err(CandidateError::BlobMissing {
                commit: commit.to_owned(),
                path: path.clone(),
            });
        }
        let fields: Vec<&str> = line.split(' ').collect();
        let size = match fields.as_slice() {
            [_, "blob", size] => {
                size.parse::<usize>()
                    .map_err(|source| CandidateError::BadSize {
                        header: header.clone(),
                        source,
                    })?
            }
            _ => {
                return Err(CandidateError::BadHeader {
                    header: header.clone(),
                    path: path.clone(),
                });
            }
        };
        let mut data = vec![0; size + 1];
        reader
            .read_exact(&mut data)
            .map_err(|source| CandidateError::ShortRead {
                path: path.clone(),
                source,
            })?;
        if data.pop() != Some(b'\n') {
            return Err(CandidateError::SizeMismatch { path: path.clone() });
        }
        blobs.push(data);
    }
    Ok(blobs)
}

/// The one archive member a release may fill in.
pub const PINS_PATH: &str = "cli/prebuilt/pins.bzl";

/// Builds the source archive of a candidate commit from git blobs.
///
/// With `pins`, the archive holds those bytes for [`PINS_PATH`] instead of the commit's blob.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn build_source_archive(
    repo: &Path,
    candidate: &Candidate,
    dst: &Path,
    pins: Option<&[u8]>,
) -> Result<SourceArchive, CandidateError> {
    let mut files = commit_tree(repo, candidate)?;
    if let Some(pins) = pins {
        let file = files
            .iter_mut()
            .find(|file| file.path == PINS_PATH)
            .ok_or(CandidateError::NoPins)?;
        file.data = pins.to_vec();
    }
    let prefix = candidate.strip_prefix();
    let names: Vec<String> = files
        .iter()
        .map(|file| format!("{prefix}/{}", file.path))
        .collect();
    let members: Vec<(&str, &[u8], u32)> = files
        .iter()
        .zip(&names)
        .map(|(file, name)| (name.as_str(), file.data.as_slice(), file.mode))
        .collect();
    dx_deploy_tools::archive_tree(&members, dst).map_err(|source| PathIoError::Write {
        path: dst.to_path_buf(),
        source,
    })?;
    let digest = hash_file(dst)?;
    let integrity = bcr_integrity_of_digest(&digest)?;
    Ok(SourceArchive { digest, integrity })
}

/// Checks that an archive holds exactly the candidate commit's files.
///
/// With `pins`, [`PINS_PATH`] must hold those bytes instead of the commit's blob.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn source_archive_error(
    archive: &Path,
    repo: &Path,
    candidate: &Candidate,
    pins: Option<&[u8]>,
) -> Result<(), CandidateError> {
    let prefix = format!("{}/", candidate.strip_prefix());
    let mut want: BTreeMap<String, (u32, String)> = commit_tree(repo, candidate)?
        .into_iter()
        .map(|file| {
            let data = match pins {
                Some(pins) if file.path == PINS_PATH => pins,
                _ => file.data.as_slice(),
            };
            (
                format!("{prefix}{}", file.path),
                (file.mode, dx_digest::sha256_hex(data)),
            )
        })
        .collect();
    if pins.is_some() && !want.contains_key(&format!("{prefix}{PINS_PATH}")) {
        return Err(CandidateError::NoPins);
    }
    let listing =
        dx_deploy_tools::archive_listing(archive).map_err(|source| PathIoError::Read {
            path: archive.to_path_buf(),
            source,
        })?;
    for (name, mode, digest) in listing {
        if !name.starts_with(&prefix) {
            return Err(CandidateError::OutsidePrefix { name, prefix });
        }
        let Some((want_mode, want_digest)) = want.remove(&name) else {
            return Err(CandidateError::ExtraMember { name });
        };
        if mode != want_mode {
            return Err(CandidateError::MemberMode {
                name,
                mode,
                want: want_mode,
            });
        }
        if digest != want_digest {
            return Err(CandidateError::MemberDiffers {
                name,
                commit: candidate.commit().to_owned(),
            });
        }
    }
    match want.into_keys().next() {
        Some(name) => Err(CandidateError::DroppedMember { name }),
        None => Ok(()),
    }
}

/// Returns the release source archive file name of a candidate.
#[must_use]
pub fn release_source_name(candidate: &Candidate) -> String {
    format!("{BCR_WANT_MODULE}-{}.tar.gz", candidate.version())
}

fn conflicting_output_error(path: &Path, bytes: &[u8]) -> Result<(), CandidateError> {
    match std::fs::read(path) {
        Ok(existing) if existing == bytes => Ok(()),
        Ok(_) => Err(CandidateError::Immutable {
            path: path.to_path_buf(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(PathIoError::Read {
            path: path.to_path_buf(),
            source,
        }
        .into()),
    }
}

/// Builds, verifies and describes the release source archive in `out_dir`.
///
/// Writes `rules_dx-<version>.tar.gz`, its `.sha256` sidecar, and `source.json`.
/// Refuses an `out_dir` that already holds different bytes under those names.
/// `pins` replaces [`PINS_PATH`] as in [`build_source_archive`].
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn write_release_source(
    repo: &Path,
    candidate: &Candidate,
    out_dir: &Path,
    pins: Option<&[u8]>,
) -> Result<SourceArchive, CandidateError> {
    if !out_dir.is_absolute() {
        return Err(CandidateError::OutputDirRelative {
            path: out_dir.to_path_buf(),
        });
    }
    path_io::create_dir_all(out_dir)?;
    let name = release_source_name(candidate);
    let staged = out_dir.join(format!(".{name}.staging"));
    let staged_json = out_dir.join(".source.json.staging");
    let result = stage_release_source(repo, candidate, &name, &staged, &staged_json, pins);
    dx_atomic_fs::discard_file(&staged);
    dx_atomic_fs::discard_file(&staged_json);
    let (built, outputs) = result?;
    for (file, bytes) in &outputs {
        conflicting_output_error(&out_dir.join(file), bytes)?;
    }
    for (file, bytes) in &outputs {
        let path = out_dir.join(file);
        std::fs::write(&path, bytes).map_err(|source| PathIoError::Write { path, source })?;
    }
    Ok(built)
}

type StagedOutputs = (SourceArchive, Vec<(String, Vec<u8>)>);

fn stage_release_source(
    repo: &Path,
    candidate: &Candidate,
    name: &str,
    staged: &Path,
    staged_json: &Path,
    pins: Option<&[u8]>,
) -> Result<StagedOutputs, CandidateError> {
    let built = build_source_archive(repo, candidate, staged, pins)?;
    source_archive_error(staged, repo, candidate, pins)?;
    let module_text = commit_module_file(repo, candidate)?;
    module_version_error(&module_text, candidate.version())?;
    local_override_error(&module_text)?;
    let archive = path_io::read(staged)?;
    let sidecar = format!("{}  {name}\n", built.digest).into_bytes();
    let integrity = write_bcr_source_for_archive(staged_json, candidate.version(), staged)
        .map_err(CandidateError::SourceJsonWrite)?;
    if integrity != built.integrity {
        return Err(CandidateError::IntegrityDiffers);
    }
    let json = path_io::read(staged_json)?;
    let json_text = String::from_utf8(json.clone()).map_err(CandidateError::SourceJsonNotUtf8)?;
    bcr_candidate_error(
        &json_text,
        BCR_WANT_MODULE,
        candidate.version(),
        &built.digest,
    )?;
    let outputs = vec![
        (name.to_owned(), archive),
        (format!("{name}.sha256"), sidecar),
        ("source.json".to_owned(), json),
    ];
    Ok((built, outputs))
}

/// Reads the commit's root MODULE.bazel.
fn commit_module_file(repo: &Path, candidate: &Candidate) -> Result<String, CandidateError> {
    git_read(
        repo,
        &["show", &format!("{}:MODULE.bazel", candidate.commit())],
    )
    .map_err(CandidateError::Git)
}

pub(crate) fn call_end(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (at, ch) in text.get(open..)?.char_indices() {
        if in_string {
            match (escaped, ch) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + at);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn string_fields(body: &str, base: usize) -> Vec<(String, String, usize, usize)> {
    let mut fields = Vec::new();
    let bytes = body.as_bytes();
    let mut at = 0;
    let mut depth = 0usize;
    while let Some(&current) = bytes.get(at) {
        match current {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b'"' => {
                at += 1;
                while let Some(&inner) = bytes.get(at) {
                    if inner == b'"' {
                        break;
                    }
                    at += if inner == b'\\' { 2 } else { 1 };
                }
            }
            byte if depth == 0 && (byte.is_ascii_alphabetic() || byte == b'_') => {
                let start = at;
                while bytes
                    .get(at)
                    .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
                {
                    at += 1;
                }
                let name = body.get(start..at).unwrap_or("");
                let rest = body.get(at..).unwrap_or("");
                let after_eq = rest.trim_start();
                if let Some(value) = after_eq.strip_prefix('=').filter(|v| !v.starts_with('=')) {
                    let value_trim = value.trim_start();
                    if let Some(quoted) = value_trim.strip_prefix('"')
                        && let Some((inner, _)) = quoted.split_once('"')
                    {
                        let close = inner.len();
                        let open = body.len() - quoted.len();
                        fields.push((
                            name.to_owned(),
                            inner.to_owned(),
                            base + open,
                            base + open + close,
                        ));
                    }
                }
                continue;
            }
            _ => {}
        }
        at += 1;
    }
    fields
}

/// Rewrites the version of the single `rules_dx` `module()` call and keeps the rest.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn candidate_module_file(text: &str, version: &str) -> Result<String, CandidateError> {
    candidate_named_module_file(text, version, BCR_WANT_MODULE)
}

/// Like [`candidate_module_file`] for the module called `name`.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn candidate_named_module_file(
    text: &str,
    version: &str,
    name: &str,
) -> Result<String, CandidateError> {
    bcr_candidate_version_error(version)?;
    let starts: Vec<usize> = text
        .match_indices("module(")
        .map(|(at, _)| at)
        .filter(|at| {
            *at == 0
                || text
                    .as_bytes()
                    .get(..*at)
                    .is_some_and(|before| before.last() == Some(&b'\n'))
        })
        .collect();
    let open = match starts.as_slice() {
        [one] => *one + "module".len(),
        found => return Err(CandidateError::ModuleCalls { count: found.len() }),
    };
    let close = call_end(text, open).ok_or(CandidateError::ModuleNotClosed)?;
    let body = text
        .get(open + 1..close)
        .ok_or(CandidateError::ModuleNotClosed)?;
    let fields = string_fields(body, open + 1);
    let named: Vec<&String> = fields
        .iter()
        .filter(|field| field.0 == "name")
        .map(|field| &field.1)
        .collect();
    if named.as_slice() != [name] {
        return Err(CandidateError::ModuleName {
            named: named.into_iter().cloned().collect(),
            want: name.to_owned(),
        });
    }
    let versions: Vec<&(String, String, usize, usize)> =
        fields.iter().filter(|field| field.0 == "version").collect();
    let (start, end) = match versions.as_slice() {
        [one] => (one.2, one.3),
        found => return Err(CandidateError::VersionFields { count: found.len() }),
    };
    let (Some(head), Some(tail)) = (text.get(..start), text.get(end..)) else {
        return Err(CandidateError::VersionOutOfRange);
    };
    Ok(format!("{head}{version}{tail}"))
}

/// Returns one absolute path as a `file://` URL with forward slashes.
#[must_use]
pub fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), CandidateError> {
    if let Some(parent) = path.parent() {
        path_io::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes).map_err(|source| {
        PathIoError::Write {
            path: path.to_path_buf(),
            source,
        }
        .into()
    })
}

/// Stages a local Bazel registry that serves exactly one candidate archive.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn stage_local_registry(
    dir: &Path,
    candidate: &Candidate,
    archive: &Path,
) -> Result<PathBuf, CandidateError> {
    if !dir.is_absolute() {
        return Err(CandidateError::RegistryDirRelative {
            path: dir.to_path_buf(),
        });
    }
    let digest = hash_file(archive)?;
    let version = candidate.version();
    let mirror = dir.join("mirror");
    write_file(
        &dir.join("bazel_registry.json"),
        pretty_json(&serde_json::json!({
            "mirrors": [format!("{}/", file_url(&mirror))],
        }))
        .map_err(CandidateError::Render)?
        .as_bytes(),
    )?;
    let prefix = candidate.strip_prefix();
    let listing =
        dx_deploy_tools::archive_listing(archive).map_err(|source| PathIoError::List {
            path: archive.to_path_buf(),
            source,
        })?;
    let module_path = format!("{prefix}/MODULE.bazel");
    if !listing.iter().any(|(path, _, _)| path == &module_path) {
        return Err(CandidateError::MemberMissing {
            member: module_path,
        });
    }
    let bytes = dx_deploy_tools::archive_read_member(archive, &module_path).map_err(|source| {
        CandidateError::MemberRead {
            member: module_path.clone(),
            source,
        }
    })?;
    let text = String::from_utf8(bytes).map_err(|source| CandidateError::MemberNotUtf8 {
        member: module_path.clone(),
        source,
    })?;
    let module_file = candidate_named_module_file(&text, version, BCR_WANT_MODULE)?;
    let module_dir = dir.join("modules").join(BCR_WANT_MODULE);
    write_file(
        &module_dir.join("metadata.json"),
        pretty_json(&serde_json::json!({
            "homepage": RELEASE_REPOSITORY,
            "maintainers": [],
            "repository": [RELEASE_REPOSITORY.replace("https://github.com/", "github:")],
            "versions": [version],
            "yanked_versions": {},
        }))
        .map_err(CandidateError::Render)?
        .as_bytes(),
    )?;
    write_file(
        &module_dir.join(version).join("MODULE.bazel"),
        module_file.as_bytes(),
    )?;
    let source = render_bcr_source_for_digest(BCR_WANT_MODULE, version, &digest)?;
    let source_path = module_dir.join(version).join("source.json");
    write_file(&source_path, source.as_bytes())?;
    let staged = path_io::read_to_string(&source_path)?;
    bcr_candidate_error(&staged, BCR_WANT_MODULE, version, &digest)?;
    let url = candidate.url();
    let Some(relative) = url.strip_prefix("https://") else {
        return Err(CandidateError::NotHttps { url });
    };
    let mirrored = mirror.join(relative);
    let bytes = path_io::read(archive)?;
    write_file(&mirrored, &bytes)?;
    let mirrored_digest = hash_file(&mirrored)?;
    if mirrored_digest != digest {
        return Err(CandidateError::MirrorDigest {
            path: mirrored,
            have: mirrored_digest,
            want: digest,
        });
    }
    Ok(dir.to_path_buf())
}

/// Returns the peeled commit of one tag from git ls-remote output.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn remote_tag_commit(output: &str, tag: &str) -> Result<String, CandidateError> {
    let want = format!("refs/tags/{tag}");
    let peeled = format!("{want}^{{}}");
    let mut direct: Option<&str> = None;
    let mut peel: Option<&str> = None;
    for line in output.lines().filter(|line| !line.is_empty()) {
        let (sha, reference) = line
            .split_once('\t')
            .filter(|(sha, reference)| {
                dx_digest::is_lower_hex(sha, 20)
                    && !reference.is_empty()
                    && !reference.contains(char::is_whitespace)
            })
            .ok_or_else(|| CandidateError::MalformedLsRemote {
                line: line.to_owned(),
            })?;
        let slot = if reference == peeled {
            &mut peel
        } else if reference == want {
            &mut direct
        } else {
            continue;
        };
        if slot.replace(sha).is_some() {
            return Err(CandidateError::AmbiguousLsRemote {
                reference: reference.to_owned(),
            });
        }
    }
    peel.or(direct)
        .map(str::to_owned)
        .ok_or_else(|| CandidateError::NoRemoteTag {
            tag: tag.to_owned(),
        })
}

/// The git for-each-ref format that `local_tag_commit` parses.
pub const LOCAL_TAG_FORMAT: &str = "--format=%(refname) %(objectname) %(*objectname)";

/// Returns the peeled commit of one local tag from git for-each-ref output.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn local_tag_commit(output: &str, tag: &str) -> Result<Option<String>, CandidateError> {
    let want = format!("refs/tags/{tag}");
    let mut found: Option<String> = None;
    for line in output.lines().filter(|line| !line.is_empty()) {
        let malformed = || CandidateError::MalformedForEachRef {
            line: line.to_owned(),
        };
        let fields: Vec<&str> = line.split(' ').collect();
        let (reference, object, peeled) = match fields.as_slice() {
            [reference, object, peeled] => (*reference, *object, *peeled),
            _ => return Err(malformed()),
        };
        if !dx_digest::is_lower_hex(object, 20)
            || !(peeled.is_empty() || dx_digest::is_lower_hex(peeled, 20))
        {
            return Err(malformed());
        }
        if reference != want {
            continue;
        }
        let commit = if peeled.is_empty() { object } else { peeled };
        if found.replace(commit.to_owned()).is_some() {
            return Err(CandidateError::AmbiguousForEachRef { reference: want });
        }
    }
    Ok(found)
}

/// Combines git status, HEAD and tag output into the local release state.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn git_state_of(
    status: Result<String, GitError>,
    head: Result<String, GitError>,
    tags: Result<String, GitError>,
    tag: &str,
) -> Result<GitState, CandidateError> {
    let status = status.map_err(CandidateError::GitState)?;
    let head = head.map_err(CandidateError::GitState)?;
    let tags = tags.map_err(CandidateError::GitState)?;
    let head = head.trim();
    if !dx_digest::is_lower_hex(head, 20) {
        return Err(CandidateError::BadHead {
            head: head.to_owned(),
        });
    }
    Ok(GitState {
        tree_dirty: !status.trim().is_empty(),
        head: head.to_owned(),
        tag_commit: local_tag_commit(&tags, tag)?,
    })
}

/// Reads the local release state of a repository with git.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn read_git_state(repo: Option<&Path>, tag: &str) -> Result<GitState, CandidateError> {
    let read = |args: &[&str]| git_in(repo, args);
    let reference = format!("refs/tags/{tag}");
    git_state_of(
        read(&["status", "--porcelain"]),
        read(&["rev-parse", "--verify", "HEAD"]),
        read(&["for-each-ref", LOCAL_TAG_FORMAT, &reference]),
        tag,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "a948904f2f0f479b8f8197694b30184b0d2ed1c1";
    const PEELED: &str = "b948904f2f0f479b8f8197694b30184b0d2ed1c1";

    #[test]
    fn candidate_identity_is_one_version_and_commit() {
        let candidate = Candidate::new("1.2.3-rc1", SHA).expect("candidate");
        assert_eq!(candidate.version(), "1.2.3-rc1");
        assert_eq!(candidate.commit(), SHA);
        assert_eq!(candidate.tag(), "v1.2.3-rc1");
        assert_eq!(candidate.reference(), "refs/tags/v1.2.3-rc1");
        assert_eq!(
            candidate.ls_remote_patterns(),
            [
                "refs/tags/v1.2.3-rc1".to_owned(),
                "refs/tags/v1.2.3-rc1^{}".to_owned()
            ]
        );
        assert_eq!(candidate.strip_prefix(), "rules_dx-1.2.3-rc1");
        assert_eq!(
            candidate.url(),
            "https://github.com/devhusdk/rules_dx/releases/download/v1.2.3-rc1/rules_dx-1.2.3-rc1.tar.gz"
        );
        assert_eq!(
            candidate.remote(),
            "https://github.com/devhusdk/rules_dx.git"
        );
    }

    #[test]
    fn candidate_rejects_bad_versions_and_commits() {
        let error = Candidate::new("0.0.0", SHA).expect_err("shape version");
        assert!(matches!(
            error,
            CandidateError::Bcr(BcrError::UnpublishableVersion)
        ));
        assert!(error.to_string().contains("unpublishable"));
        for bad in ["", "1.2", "v1.2.3", "1.2.3.4", "1..3"] {
            assert!(
                Candidate::new(bad, SHA).is_err(),
                "version {bad:?} must fail"
            );
        }
        for bad in ["a948904", "", &SHA.to_uppercase(), &format!("{SHA}00")] {
            let error = Candidate::new("1.2.3", bad).expect_err("bad commit");
            assert!(matches!(error, CandidateError::InvalidCommit { .. }));
            let error = error.to_string();
            assert!(error.contains("invalid commit"), "{bad:?}: {error}");
        }
    }

    const MODULE: &str = "module(\n    name = \"rules_dx\",\n    version = \"0.0.0\",\n    compatibility_level = 1,\n)\n\nbazel_dep(name = \"rules_cc\", version = \"0.1.1\")\nsingle_version_override(\n    module_name = \"rules_cc\",\n    patches = [\"//patches:x.patch\"],\n    patch_strip = 1,\n)\next = use_extension(\"//lib:ext.bzl\", \"ext\")\nuse_repo(ext, \"thing\")\n";

    #[test]
    fn candidate_module_file_keeps_deps_patches_and_extensions() {
        let text = candidate_module_file(MODULE, "1.2.3-rc1").expect("module");
        assert_eq!(
            text,
            MODULE.replacen("version = \"0.0.0\"", "version = \"1.2.3-rc1\"", 1)
        );
        assert!(text.contains("bazel_dep(name = \"rules_cc\", version = \"0.1.1\")"));
        let single = candidate_module_file(
            "module(name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"x\", version = \"1\")\n",
            "2.0.0",
        )
        .expect("single line");
        assert_eq!(
            single,
            "module(name = \"rules_dx\", version = \"2.0.0\")\nbazel_dep(name = \"x\", version = \"1\")\n"
        );
    }

    #[test]
    fn candidate_module_file_rejects_foreign_and_ambiguous_modules() {
        let foreign = MODULE.replacen("\"rules_dx\"", "\"other\"", 1);
        let error = candidate_module_file(&foreign, "1.2.3").expect_err("foreign");
        assert!(matches!(error, CandidateError::ModuleName { .. }));
        assert_eq!(
            error.to_string(),
            "candidate: MODULE.bazel module() name is [\"other\"], want [\"rules_dx\"]"
        );
        let twice = format!("{MODULE}module(name = \"rules_dx\", version = \"1\")\n");
        let error = candidate_module_file(&twice, "1.2.3").expect_err("two calls");
        assert!(matches!(error, CandidateError::ModuleCalls { count: 2 }));
        assert!(error.to_string().contains("2 module() calls"));
        let unversioned = "module(name = \"rules_dx\")\n";
        let error = candidate_module_file(unversioned, "1.2.3").expect_err("no version");
        assert!(matches!(error, CandidateError::VersionFields { count: 0 }));
        assert!(error.to_string().contains("0 version fields"));
        let error = candidate_module_file("bazel_dep(name = \"x\")\n", "1.2.3").expect_err("none");
        assert!(matches!(error, CandidateError::ModuleCalls { count: 0 }));
        assert!(error.to_string().contains("0 module() calls"));
        let error =
            candidate_module_file("module(name = \"rules_dx\"\n", "1.2.3").expect_err("open");
        assert!(matches!(error, CandidateError::ModuleNotClosed));
        assert!(error.to_string().contains("not closed"));
        assert!(matches!(
            candidate_module_file(MODULE, "0.0.0"),
            Err(CandidateError::Bcr(BcrError::UnpublishableVersion))
        ));
    }

    #[test]
    fn remote_tag_commit_prefers_the_peeled_annotated_commit() {
        let output = format!("{SHA}\trefs/tags/v1.2.3\n{PEELED}\trefs/tags/v1.2.3^{{}}\n");
        assert_eq!(remote_tag_commit(&output, "v1.2.3").expect("tag"), PEELED);
    }

    #[test]
    fn remote_tag_commit_reads_a_lightweight_tag_exactly() {
        let output = format!(
            "{PEELED}\trefs/tags/v1.2.30\n{SHA}\trefs/tags/v1.2.3\n{PEELED}\trefs/heads/v1.2.3\n"
        );
        assert_eq!(remote_tag_commit(&output, "v1.2.3").expect("tag"), SHA);
    }

    #[test]
    fn remote_tag_commit_rejects_malformed_lines() {
        for bad in [
            "garbage without a tab\n".to_owned(),
            "not-a-sha\trefs/tags/v1.2.3\n".to_owned(),
            format!("{}\trefs/tags/v1.2.3\n", SHA.to_uppercase()),
            format!("{SHA}\t\n"),
            format!("{SHA}\trefs/tags/v1.2.3 extra\n"),
        ] {
            let error = remote_tag_commit(&bad, "v1.2.3").expect_err("malformed");
            assert!(matches!(error, CandidateError::MalformedLsRemote { .. }));
            let error = error.to_string();
            assert!(
                error.contains("malformed git ls-remote line"),
                "{bad:?}: {error}"
            );
        }
    }

    #[test]
    fn remote_tag_commit_rejects_duplicate_and_conflicting_lines() {
        let duplicate = format!("{SHA}\trefs/tags/v1.2.3\n{SHA}\trefs/tags/v1.2.3\n");
        let error = remote_tag_commit(&duplicate, "v1.2.3").expect_err("duplicate");
        assert!(matches!(error, CandidateError::AmbiguousLsRemote { .. }));
        assert!(error.to_string().contains("ambiguous"));
        let conflicting =
            format!("{SHA}\trefs/tags/v1.2.3^{{}}\n{PEELED}\trefs/tags/v1.2.3^{{}}\n");
        assert!(
            remote_tag_commit(&conflicting, "v1.2.3")
                .expect_err("conflicting")
                .to_string()
                .contains("ambiguous")
        );
    }

    #[test]
    fn remote_tag_commit_rejects_empty_output() {
        let error = remote_tag_commit("", "v1.2.3").expect_err("no tag");
        assert!(matches!(error, CandidateError::NoRemoteTag { .. }));
        assert_eq!(error.to_string(), "release: remote has no tag v1.2.3");
    }

    #[test]
    fn local_tag_commit_peels_and_matches_exact_refs() {
        assert_eq!(local_tag_commit("", "v1.2.3").expect("empty"), None);
        assert_eq!(
            local_tag_commit(&format!("refs/tags/v1.2.3 {SHA} \n"), "v1.2.3").expect("direct"),
            Some(SHA.to_owned())
        );
        assert_eq!(
            local_tag_commit(&format!("refs/tags/v1.2.3 {SHA} {PEELED}\n"), "v1.2.3")
                .expect("peeled"),
            Some(PEELED.to_owned())
        );
        assert_eq!(
            local_tag_commit(&format!("refs/tags/v1.2.3/x {SHA} \n"), "v1.2.3").expect("other"),
            None
        );
        let twice = format!("refs/tags/v1.2.3 {SHA} \nrefs/tags/v1.2.3 {PEELED} \n");
        let error = local_tag_commit(&twice, "v1.2.3").expect_err("twice");
        assert!(matches!(error, CandidateError::AmbiguousForEachRef { .. }));
        assert!(error.to_string().contains("ambiguous"));
        for bad in ["v1.2.3\n".to_owned(), "refs/tags/v1.2.3 xyz \n".to_string()] {
            let error = local_tag_commit(&bad, "v1.2.3").expect_err("malformed");
            assert!(matches!(error, CandidateError::MalformedForEachRef { .. }));
            assert!(error.to_string().contains("malformed"));
        }
    }

    #[test]
    fn git_state_of_propagates_first_error_and_parses_ok() {
        let failed = |detail: &str| GitError::Exit {
            command: "git status".to_owned(),
            detail: detail.to_owned(),
        };
        let state = git_state_of(
            Ok(" M a\n".to_owned()),
            Ok(format!("{SHA}\n")),
            Ok(format!("refs/tags/v1.2.3 {SHA} \n")),
            "v1.2.3",
        )
        .expect("state");
        assert!(state.tree_dirty);
        assert_eq!(state.head, SHA);
        assert_eq!(state.tag_commit.as_deref(), Some(SHA));
        let clean = git_state_of(
            Ok("  \n".to_owned()),
            Ok(SHA.to_owned()),
            Ok(String::new()),
            "v1.2.3",
        )
        .expect("state");
        assert!(!clean.tree_dirty);
        assert_eq!(clean.tag_commit, None);
        let error = git_state_of(
            Err(failed("first")),
            Err(failed("second")),
            Ok(String::new()),
            "v1.2.3",
        )
        .expect_err("first");
        assert!(matches!(
            error,
            CandidateError::GitState(GitError::Exit { .. })
        ));
        assert_eq!(
            error.to_string(),
            "release: cannot read git state: git status failed: first"
        );
        assert_eq!(
            git_state_of(
                Ok(String::new()),
                Ok(SHA.to_owned()),
                Err(failed("third")),
                "v1.2.3"
            )
            .expect_err("third")
            .to_string(),
            "release: cannot read git state: git status failed: third"
        );
        let error = git_state_of(
            Ok(String::new()),
            Ok("HEAD".to_owned()),
            Ok(String::new()),
            "v1.2.3",
        )
        .expect_err("bad head");
        assert!(matches!(error, CandidateError::BadHead { .. }));
        assert!(error.to_string().contains("HEAD is"));
    }

    #[test]
    fn git_failure_is_an_error_not_clean() {
        let captured = |code: Option<i32>, stdout: Vec<u8>, stderr: &[u8]| Captured {
            command: "git".to_owned(),
            code,
            stdout,
            stderr: stderr.to_vec(),
        };
        let failed = captured(Some(1), Vec::new(), b"fatal: not a git repository\n");
        let error = git_output_of(&["status", "--porcelain"], Ok(failed)).expect_err("failed");
        assert!(matches!(error, GitError::Exit { .. }));
        assert_eq!(
            error.to_string(),
            "git status --porcelain failed: fatal: not a git repository"
        );
        let silent = captured(Some(1), Vec::new(), b"");
        let err = git_output_of(&["tag"], Ok(silent))
            .expect_err("silent failure")
            .to_string();
        assert!(err.starts_with("git tag failed: "), "{err}");
        let spawn = RunError::Spawn {
            command: "git tag".to_owned(),
            source: io::Error::new(io::ErrorKind::NotFound, "no git"),
        };
        let error = git_output_of(&["tag"], Err(spawn)).expect_err("spawn failure");
        assert!(matches!(error, GitError::Run { .. }));
        assert!(error.to_string().starts_with("git tag failed: "));
        let binary = captured(Some(0), vec![0xff], b"");
        let error = git_output_of(&["status"], Ok(binary)).expect_err("binary");
        assert!(matches!(error, GitError::NotUtf8 { .. }));
        assert!(
            std::error::Error::source(&error).is_some_and(|source| source.is::<FromUtf8Error>())
        );
        assert_eq!(error.to_string(), "git status printed non-UTF-8 output");
    }

    #[test]
    fn read_batch_rejects_missing_and_short_blobs() {
        let entries = vec![("a.txt".to_owned(), 0o644)];
        assert_eq!(
            read_batch(&b"0123 blob 2\nhi\n"[..], SHA, &entries).expect("blob"),
            vec![b"hi".to_vec()]
        );
        let error = read_batch(&b"a.txt missing\n"[..], SHA, &entries).expect_err("missing");
        assert!(matches!(error, CandidateError::BlobMissing { .. }));
        assert!(error.to_string().contains("is missing"));
        let error = read_batch(&b"0123 blob 5\nhi\n"[..], SHA, &entries).expect_err("short");
        assert!(matches!(error, CandidateError::ShortRead { .. }));
        assert!(error.to_string().contains("short read"));
        let error = read_batch(&b"0123 blob 1\nhi\n"[..], SHA, &entries).expect_err("size");
        assert!(matches!(error, CandidateError::SizeMismatch { .. }));
        assert!(error.to_string().contains("size mismatch"));
        let error = read_batch(&b"0123 tree 1\nh\n"[..], SHA, &entries).expect_err("tree");
        assert!(matches!(error, CandidateError::BadHeader { .. }));
        assert!(error.to_string().contains("bad header"));
        let error = read_batch(&b"0123 blob x\nh\n"[..], SHA, &entries).expect_err("size");
        assert!(matches!(error, CandidateError::BadSize { .. }));
        assert!(error.to_string().contains("bad size"));
    }

    #[test]
    fn errors_keep_their_sources() {
        let entries = vec![("a.txt".to_owned(), 0o644)];
        let error = read_batch(&b"0123 blob x\nh\n"[..], SHA, &entries).expect_err("size");
        assert!(
            std::error::Error::source(&error).is_some_and(|source| source.is::<ParseIntError>())
        );
        let dir = tempfile::tempdir().expect("tempdir");
        let candidate = Candidate::new("1.2.3", SHA).expect("candidate");
        let missing = dir.path().join("missing.tar.gz");
        let error = stage_local_registry(&dir.path().join("registry"), &candidate, &missing)
            .expect_err("missing archive");
        assert!(
            matches!(&error, CandidateError::Hash { path, source }
                if path == &missing && source.kind() == io::ErrorKind::NotFound),
            "{error}"
        );
        assert!(std::error::Error::source(&error).is_some_and(|source| source.is::<io::Error>()));
        assert!(error.to_string().starts_with("candidate: cannot hash "));
    }
}
