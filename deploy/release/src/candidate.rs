//! Release candidate identity, source archive, local registry and tag checks.

use std::collections::BTreeMap;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::{
    bcr_candidate_error, bcr_candidate_version_error, bcr_integrity_of_digest, bcr_source_url,
    bcr_strip_prefix, render_bcr_source_for_digest, render_pretty, write_bcr_source_for_archive,
    GitState, BCR_WANT_MODULE, RELEASE_REMOTE, RELEASE_REPOSITORY,
};

/// One release candidate: a SemVer version at one full commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    version: String,
    commit: String,
}

impl Candidate {
    /// Returns a candidate for a publishable version and a full lowercase commit.
    pub fn new(version: &str, commit: &str) -> Result<Self, String> {
        bcr_candidate_version_error(version)?;
        if !dx_digest::is_lower_hex(commit, 20) {
            return Err(format!(
                "candidate: invalid commit {commit:?}: want a full 40-char lowercase SHA"
            ));
        }
        Ok(Self {
            version: version.to_owned(),
            commit: commit.to_owned(),
        })
    }

    /// Returns the SemVer version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns the full commit SHA.
    pub fn commit(&self) -> &str {
        &self.commit
    }

    /// Returns the release tag.
    pub fn tag(&self) -> String {
        format!("v{}", self.version)
    }

    /// Returns the full tag ref.
    pub fn reference(&self) -> String {
        format!("refs/tags/{}", self.tag())
    }

    /// Returns the ls-remote patterns for the tag and its peeled commit.
    pub fn ls_remote_patterns(&self) -> [String; 2] {
        let reference = self.reference();
        let peeled = format!("{reference}^{{}}");
        [reference, peeled]
    }

    /// Returns the archive strip prefix.
    pub fn strip_prefix(&self) -> String {
        bcr_strip_prefix(BCR_WANT_MODULE, &self.version)
    }

    /// Returns the release asset URL.
    pub fn url(&self) -> String {
        bcr_source_url(BCR_WANT_MODULE, &self.version)
    }

    /// Returns the canonical release remote.
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

/// Returns stdout of a git command, or its failure as an error.
pub fn git_output_of(args: &[&str], output: io::Result<Output>) -> Result<String, String> {
    let command = format!("git {}", args.join(" "));
    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8(out.stdout).map_err(|_| format!("{command} printed non-UTF-8 output"))
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            let detail = if stderr.is_empty() {
                out.status.to_string()
            } else {
                stderr
            };
            Err(format!("{command} failed: {detail}"))
        }
        Err(err) => Err(format!("{command} failed: {err}")),
    }
}

/// Runs git in a directory and returns its stdout.
pub fn git_in(dir: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    git_output_of(args, cmd.output())
}

pub fn ls_remote_output(
    dir: Option<&Path>,
    remote: &str,
    candidate: &Candidate,
) -> Result<String, String> {
    let [reference, peeled] = candidate.ls_remote_patterns();
    git_in(dir, &["ls-remote", "--tags", remote, &reference, &peeled])
}

const READ_CONFIG: [&str; 4] = ["-c", "core.autocrlf=false", "-c", "core.eol=lf"];

fn git_read(repo: &Path, args: &[&str]) -> Result<String, String> {
    let mut full: Vec<&str> = READ_CONFIG.to_vec();
    full.extend_from_slice(args);
    git_in(Some(repo), &full)
}

struct TreeFile {
    path: String,
    mode: u32,
    data: Vec<u8>,
}

fn commit_tree(repo: &Path, candidate: &Candidate) -> Result<Vec<TreeFile>, String> {
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
    .map_err(|error| {
        format!(
            "candidate: commit {commit} is not in {}: {error}",
            repo.display()
        )
    })?;
    if resolved.trim() != commit {
        return Err(format!(
            "candidate: commit {commit} resolves to {:?} in {}",
            resolved.trim(),
            repo.display()
        ));
    }
    let listing = git_read(repo, &["ls-tree", "-r", "-z", "--full-tree", commit])
        .map_err(|error| format!("candidate: {error}"))?;
    let mut entries: Vec<(String, u32)> = Vec::new();
    for record in listing.split('\0').filter(|record| !record.is_empty()) {
        let (meta, path) = record
            .split_once('\t')
            .ok_or_else(|| format!("candidate: malformed git ls-tree record {record:?}"))?;
        let mode = match meta.split(' ').collect::<Vec<_>>().as_slice() {
            ["100644", "blob", _] => 0o644,
            ["100755", "blob", _] => 0o755,
            [mode, kind, _] => {
                return Err(format!(
                "candidate: {path} is a {kind} with mode {mode}; only regular files are archived"
            ))
            }
            _ => {
                return Err(format!(
                    "candidate: malformed git ls-tree record {record:?}"
                ))
            }
        };
        if path.contains('\n') {
            return Err(format!("candidate: path {path:?} contains a newline"));
        }
        entries.push((path.to_owned(), mode));
    }
    if entries.is_empty() {
        return Err(format!("candidate: commit {commit} has an empty tree"));
    }
    let blobs = cat_blobs(repo, commit, &entries)?;
    Ok(entries
        .into_iter()
        .zip(blobs)
        .map(|((path, mode), data)| TreeFile { path, mode, data })
        .collect())
}

const FILTER_ATTRS: [&str; 6] = [
    "text",
    "eol",
    "crlf",
    "filter",
    "ident",
    "working-tree-encoding",
];

fn git_pipe<T>(
    repo: &Path,
    args: &[&str],
    input: Vec<u8>,
    read: impl FnOnce(io::BufReader<std::process::ChildStdout>) -> Result<T, String>,
) -> Result<T, String> {
    let command = format!("git {}", args.join(" "));
    let mut child = Command::new("git")
        .args(args)
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("candidate: {command} failed: {error}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| format!("candidate: {command} has no stdin"))?;
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("candidate: {command} has no stdout"))?;
    let value = read(io::BufReader::new(stdout));
    let written = writer.join();
    let output = child
        .wait_with_output()
        .map_err(|error| format!("candidate: {command} failed: {error}"))?;
    let value = value?;
    if !output.status.success() {
        return Err(format!(
            "candidate: {command} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    match written {
        Ok(Ok(())) => Ok(value),
        _ => Err(format!("candidate: {command} closed its input early")),
    }
}

fn filtered_paths(
    repo: &Path,
    commit: &str,
    entries: &[(String, u32)],
) -> Result<Vec<bool>, String> {
    let mut input = Vec::new();
    for (path, _) in entries {
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    let source = format!("--attr-source={commit}");
    let mut args = vec![source.as_str()];
    args.extend_from_slice(&READ_CONFIG);
    args.extend_from_slice(&["check-attr", "-z", "--stdin"]);
    args.extend_from_slice(&FILTER_ATTRS);
    let raw = git_pipe(repo, &args, input, |mut reader| {
        let mut raw = Vec::new();
        reader
            .read_to_end(&mut raw)
            .map_err(|error| format!("candidate: git check-attr output: {error}"))?;
        Ok(raw)
    })?;
    let fields: Vec<&[u8]> = raw.split(|byte| *byte == 0).collect();
    let mut flagged = std::collections::BTreeSet::new();
    for record in fields.chunks(3).filter(|record| record.len() == 3) {
        if record[2] != b"unspecified" {
            flagged.insert(record[0]);
        }
    }
    Ok(entries
        .iter()
        .map(|(path, _)| flagged.contains(path.as_bytes()))
        .collect())
}

fn cat_blobs(repo: &Path, commit: &str, entries: &[(String, u32)]) -> Result<Vec<Vec<u8>>, String> {
    let mut input = String::new();
    for (path, _) in entries {
        input.push_str(&format!("{commit}:{path}\n"));
    }
    let mut blobs = git_pipe(
        repo,
        &["cat-file", "--batch"],
        input.into_bytes(),
        |reader| read_batch(reader, commit, entries),
    )?;
    let flagged = filtered_paths(repo, commit, entries)?;
    let source = format!("--attr-source={commit}");
    for ((blob, (path, _)), filter) in blobs.iter_mut().zip(entries).zip(flagged) {
        if !filter {
            continue;
        }
        let object = format!("{commit}:{path}");
        let mut args = vec![source.as_str()];
        args.extend_from_slice(&READ_CONFIG);
        args.extend_from_slice(&["cat-file", "--filters", &object]);
        *blob = git_pipe(repo, &args, Vec::new(), |mut reader| {
            let mut data = Vec::new();
            reader
                .read_to_end(&mut data)
                .map_err(|error| format!("candidate: git cat-file --filters {object}: {error}"))?;
            Ok(data)
        })?;
    }
    Ok(blobs)
}

fn read_batch(
    mut reader: impl BufRead,
    commit: &str,
    entries: &[(String, u32)],
) -> Result<Vec<Vec<u8>>, String> {
    let mut blobs = Vec::with_capacity(entries.len());
    for (path, _) in entries {
        let mut header = String::new();
        reader
            .read_line(&mut header)
            .map_err(|error| format!("candidate: git cat-file output: {error}"))?;
        let line = header.trim_end_matches('\n');
        if line.ends_with(" missing") || line.ends_with(" ambiguous") {
            return Err(format!(
                "candidate: git cat-file: {commit}:{path} is missing"
            ));
        }
        let fields: Vec<&str> = line.split(' ').collect();
        let size = match fields.as_slice() {
            [_, "blob", size] => size
                .parse::<usize>()
                .map_err(|_| format!("candidate: git cat-file bad size line {header:?}"))?,
            _ => {
                return Err(format!(
                    "candidate: git cat-file bad header {header:?} for {path}"
                ))
            }
        };
        let mut data = vec![0; size + 1];
        reader
            .read_exact(&mut data)
            .map_err(|error| format!("candidate: git cat-file short read for {path}: {error}"))?;
        if data.pop() != Some(b'\n') {
            return Err(format!("candidate: git cat-file size mismatch for {path}"));
        }
        blobs.push(data);
    }
    Ok(blobs)
}

/// Builds the source archive of a candidate commit from git blobs.
pub fn build_source_archive(
    repo: &Path,
    candidate: &Candidate,
    dst: &Path,
) -> Result<SourceArchive, String> {
    let files = commit_tree(repo, candidate)?;
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
    dx_deploy_tools::archive_tree(&members, dst)
        .map_err(|error| format!("candidate: cannot write {}: {error}", dst.display()))?;
    let digest = dx_digest::sha256_file_hex(dst)
        .map_err(|error| format!("candidate: cannot hash {}: {error}", dst.display()))?;
    let integrity = bcr_integrity_of_digest(&digest)?;
    Ok(SourceArchive { digest, integrity })
}

/// Checks that an archive holds exactly the candidate commit's files.
pub fn source_archive_error(
    archive: &Path,
    repo: &Path,
    candidate: &Candidate,
) -> Result<(), String> {
    let prefix = format!("{}/", candidate.strip_prefix());
    let mut want: BTreeMap<String, (u32, String)> = commit_tree(repo, candidate)?
        .into_iter()
        .map(|file| {
            (
                format!("{prefix}{}", file.path),
                (file.mode, dx_digest::sha256_hex(&file.data)),
            )
        })
        .collect();
    let listing = dx_deploy_tools::archive_listing(archive)
        .map_err(|error| format!("candidate: cannot read {}: {error}", archive.display()))?;
    for (name, mode, digest) in listing {
        if !name.starts_with(&prefix) {
            return Err(format!(
                "candidate: archive member {name} is outside strip prefix {prefix}"
            ));
        }
        let (want_mode, want_digest) = want
            .remove(&name)
            .ok_or_else(|| format!("candidate: archive has extra member {name}"))?;
        if mode != want_mode {
            return Err(format!(
                "candidate: archive member {name} has mode {mode:o}, want {want_mode:o}"
            ));
        }
        if digest != want_digest {
            return Err(format!(
                "candidate: archive member {name} differs from commit {}",
                candidate.commit()
            ));
        }
    }
    match want.into_keys().next() {
        Some(name) => Err(format!("candidate: archive drops member {name}")),
        None => Ok(()),
    }
}

/// Returns the release source archive file name of a candidate.
pub fn release_source_name(candidate: &Candidate) -> String {
    format!("{BCR_WANT_MODULE}-{}.tar.gz", candidate.version())
}

fn conflicting_output_error(path: &Path, bytes: &[u8]) -> Result<(), String> {
    match std::fs::read(path) {
        Ok(existing) if existing == bytes => Ok(()),
        Ok(_) => Err(format!(
            "candidate: {} already exists with different bytes (release assets are immutable)",
            path.display()
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "candidate: cannot read {}: {error}",
            path.display()
        )),
    }
}

/// Builds, verifies and describes the release source archive in `out_dir`.
///
/// Writes `rules_dx-<version>.tar.gz`, its `.sha256` sidecar and `source.json`.
/// Refuses an `out_dir` that already holds different bytes under those names.
pub fn write_release_source(
    repo: &Path,
    candidate: &Candidate,
    out_dir: &Path,
) -> Result<SourceArchive, String> {
    if !out_dir.is_absolute() {
        return Err(format!(
            "candidate: output directory {} must be absolute",
            out_dir.display()
        ));
    }
    std::fs::create_dir_all(out_dir)
        .map_err(|error| format!("candidate: cannot create {}: {error}", out_dir.display()))?;
    let name = release_source_name(candidate);
    let staged = out_dir.join(format!(".{name}.staging"));
    let staged_json = out_dir.join(".source.json.staging");
    let result = stage_release_source(repo, candidate, &name, &staged, &staged_json);
    let _ = std::fs::remove_file(&staged);
    let _ = std::fs::remove_file(&staged_json);
    let (built, outputs) = result?;
    for (file, bytes) in &outputs {
        conflicting_output_error(&out_dir.join(file), bytes)?;
    }
    for (file, bytes) in &outputs {
        std::fs::write(out_dir.join(file), bytes)
            .map_err(|error| format!("candidate: cannot write {file}: {error}"))?;
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
) -> Result<StagedOutputs, String> {
    let built = build_source_archive(repo, candidate, staged)?;
    source_archive_error(staged, repo, candidate)?;
    let integrity =
        write_bcr_source_for_archive(staged_json, BCR_WANT_MODULE, candidate.version(), staged)
            .map_err(|error| format!("candidate: cannot write source.json: {error}"))?;
    if integrity != built.integrity {
        return Err("candidate: source.json integrity differs from the archive".to_owned());
    }
    let json = std::fs::read(staged_json)
        .map_err(|error| format!("candidate: cannot read source.json: {error}"))?;
    let json_text = String::from_utf8(json.clone())
        .map_err(|error| format!("candidate: source.json is not UTF-8: {error}"))?;
    bcr_candidate_error(
        &json_text,
        BCR_WANT_MODULE,
        candidate.version(),
        &built.digest,
    )?;
    let archive = std::fs::read(staged)
        .map_err(|error| format!("candidate: cannot read {}: {error}", staged.display()))?;
    let sidecar = format!("{}  {name}\n", built.digest).into_bytes();
    let outputs = vec![
        (name.to_owned(), archive),
        (format!("{name}.sha256"), sidecar),
        ("source.json".to_owned(), json),
    ];
    Ok((built, outputs))
}

fn call_end(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (at, ch) in text[open..].char_indices() {
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

fn string_fields(body: &str, base: usize) -> Vec<(String, String, usize, usize)> {
    let mut fields = Vec::new();
    let bytes = body.as_bytes();
    let mut at = 0;
    let mut depth = 0usize;
    while at < bytes.len() {
        match bytes[at] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b'"' => {
                at += 1;
                while at < bytes.len() && bytes[at] != b'"' {
                    at += if bytes[at] == b'\\' { 2 } else { 1 };
                }
            }
            byte if depth == 0 && (byte.is_ascii_alphabetic() || byte == b'_') => {
                let start = at;
                while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                    at += 1;
                }
                let name = &body[start..at];
                let rest = &body[at..];
                let after_eq = rest.trim_start();
                if let Some(value) = after_eq.strip_prefix('=').filter(|v| !v.starts_with('=')) {
                    let value_trim = value.trim_start();
                    if let Some(quoted) = value_trim.strip_prefix('"') {
                        if let Some(close) = quoted.find('"') {
                            let open = body.len() - quoted.len();
                            fields.push((
                                name.to_owned(),
                                quoted[..close].to_owned(),
                                base + open,
                                base + open + close,
                            ));
                        }
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

/// Rewrites the version of the single rules_dx module() call and keeps the rest.
pub fn candidate_module_file(text: &str, version: &str) -> Result<String, String> {
    bcr_candidate_version_error(version)?;
    let starts: Vec<usize> = text
        .match_indices("module(")
        .map(|(at, _)| at)
        .filter(|at| *at == 0 || text.as_bytes()[at - 1] == b'\n')
        .collect();
    let open = match starts.as_slice() {
        [one] => *one + "module".len(),
        found => {
            return Err(format!(
                "candidate: MODULE.bazel has {} module() calls, want exactly one",
                found.len()
            ))
        }
    };
    let close = call_end(text, open)
        .ok_or_else(|| "candidate: MODULE.bazel module() call is not closed".to_owned())?;
    let fields = string_fields(&text[open + 1..close], open + 1);
    let named: Vec<&String> = fields
        .iter()
        .filter(|field| field.0 == "name")
        .map(|field| &field.1)
        .collect();
    if named.as_slice() != [BCR_WANT_MODULE] {
        return Err(format!(
            "candidate: MODULE.bazel module() name is {named:?}, want [\"{BCR_WANT_MODULE}\"]"
        ));
    }
    let versions: Vec<&(String, String, usize, usize)> =
        fields.iter().filter(|field| field.0 == "version").collect();
    let (start, end) = match versions.as_slice() {
        [one] => (one.2, one.3),
        found => {
            return Err(format!(
                "candidate: MODULE.bazel module() has {} version fields, want exactly one",
                found.len()
            ))
        }
    };
    Ok(format!("{}{version}{}", &text[..start], &text[end..]))
}

fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.starts_with('/') {
        format!("file://{text}")
    } else {
        format!("file:///{text}")
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("candidate: cannot create {}: {error}", parent.display()))?;
    }
    std::fs::write(path, bytes)
        .map_err(|error| format!("candidate: cannot write {}: {error}", path.display()))
}

/// Stages a local Bazel registry that serves exactly one candidate archive.
pub fn stage_local_registry(
    dir: &Path,
    candidate: &Candidate,
    archive: &Path,
) -> Result<PathBuf, String> {
    if !dir.is_absolute() {
        return Err(format!(
            "candidate: registry dir {} must be absolute",
            dir.display()
        ));
    }
    let digest = dx_digest::sha256_file_hex(archive)
        .map_err(|error| format!("candidate: cannot hash {}: {error}", archive.display()))?;
    let member = format!("{}/MODULE.bazel", candidate.strip_prefix());
    let module_bytes = dx_deploy_tools::archive_read_member(archive, &member)
        .map_err(|error| format!("candidate: cannot read {member}: {error}"))?;
    let module_text =
        String::from_utf8(module_bytes).map_err(|_| format!("candidate: {member} is not UTF-8"))?;
    let module_file = candidate_module_file(&module_text, candidate.version())?;
    let version = candidate.version();
    let module_dir = dir.join("modules").join(BCR_WANT_MODULE);
    let mirror = dir.join("mirror");
    write_file(
        &dir.join("bazel_registry.json"),
        render_pretty(&serde_json::json!({
            "mirrors": [format!("{}/", file_url(&mirror))],
        }))
        .as_bytes(),
    )?;
    write_file(
        &module_dir.join("metadata.json"),
        render_pretty(&serde_json::json!({
            "homepage": RELEASE_REPOSITORY,
            "maintainers": [],
            "repository": [RELEASE_REPOSITORY.replace("https://github.com/", "github:")],
            "versions": [version],
            "yanked_versions": {},
        }))
        .as_bytes(),
    )?;
    write_file(
        &module_dir.join(version).join("MODULE.bazel"),
        module_file.as_bytes(),
    )?;
    let source = render_bcr_source_for_digest(BCR_WANT_MODULE, version, &digest)?;
    let source_path = module_dir.join(version).join("source.json");
    write_file(&source_path, source.as_bytes())?;
    let url = candidate.url();
    let relative = url
        .strip_prefix("https://")
        .ok_or_else(|| format!("candidate: url {url} is not https"))?;
    let mirrored = mirror.join(relative);
    let bytes = std::fs::read(archive)
        .map_err(|error| format!("candidate: cannot read {}: {error}", archive.display()))?;
    write_file(&mirrored, &bytes)?;
    let staged = std::fs::read_to_string(&source_path)
        .map_err(|error| format!("candidate: cannot read {}: {error}", source_path.display()))?;
    bcr_candidate_error(&staged, BCR_WANT_MODULE, version, &digest)?;
    let mirrored_digest = dx_digest::sha256_file_hex(&mirrored)
        .map_err(|error| format!("candidate: cannot hash {}: {error}", mirrored.display()))?;
    if mirrored_digest != digest {
        return Err(format!(
            "candidate: mirrored archive {} has sha256 {mirrored_digest}, want {digest}",
            mirrored.display()
        ));
    }
    Ok(dir.to_path_buf())
}

/// Returns the peeled commit of one tag from git ls-remote output.
pub fn remote_tag_commit(output: &str, tag: &str) -> Result<String, String> {
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
            .ok_or_else(|| format!("release: malformed git ls-remote line {line:?}"))?;
        let slot = if reference == peeled {
            &mut peel
        } else if reference == want {
            &mut direct
        } else {
            continue;
        };
        if slot.replace(sha).is_some() {
            return Err(format!(
                "release: ambiguous git ls-remote output: {reference} listed twice"
            ));
        }
    }
    peel.or(direct)
        .map(str::to_owned)
        .ok_or_else(|| format!("release: remote has no tag {tag}"))
}

/// The git for-each-ref format that local_tag_commit parses.
pub const LOCAL_TAG_FORMAT: &str = "--format=%(refname) %(objectname) %(*objectname)";

/// Returns the peeled commit of one local tag from git for-each-ref output.
pub fn local_tag_commit(output: &str, tag: &str) -> Result<Option<String>, String> {
    let want = format!("refs/tags/{tag}");
    let mut found: Option<String> = None;
    for line in output.lines().filter(|line| !line.is_empty()) {
        let fields: Vec<&str> = line.split(' ').collect();
        let (reference, object, peeled) = match fields.as_slice() {
            [reference, object, peeled] => (*reference, *object, *peeled),
            _ => return Err(format!("release: malformed git for-each-ref line {line:?}")),
        };
        if !dx_digest::is_lower_hex(object, 20)
            || !(peeled.is_empty() || dx_digest::is_lower_hex(peeled, 20))
        {
            return Err(format!("release: malformed git for-each-ref line {line:?}"));
        }
        if reference != want {
            continue;
        }
        let commit = if peeled.is_empty() { object } else { peeled };
        if found.replace(commit.to_owned()).is_some() {
            return Err(format!(
                "release: ambiguous git for-each-ref output: {want} listed twice"
            ));
        }
    }
    Ok(found)
}

/// Combines git status, HEAD and tag output into the local release state.
pub fn git_state_of(
    status: Result<String, String>,
    head: Result<String, String>,
    tags: Result<String, String>,
    tag: &str,
) -> Result<GitState, String> {
    let status = status?;
    let head = head?;
    let tags = tags?;
    let head = head.trim();
    if !dx_digest::is_lower_hex(head, 20) {
        return Err(format!("release: cannot read git state: HEAD is {head:?}"));
    }
    Ok(GitState {
        tree_dirty: !status.trim().is_empty(),
        head: head.to_owned(),
        tag_commit: local_tag_commit(&tags, tag)?,
    })
}

/// Reads the local release state of a repository with git.
pub fn read_git_state(repo: Option<&Path>, tag: &str) -> Result<GitState, String> {
    let read = |args: &[&str]| {
        git_in(repo, args).map_err(|error| format!("release: cannot read git state: {error}"))
    };
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
        assert!(Candidate::new("0.0.0", SHA)
            .expect_err("shape version")
            .contains("unpublishable"));
        for bad in ["", "1.2", "v1.2.3", "1.2.3.4", "1..3"] {
            assert!(
                Candidate::new(bad, SHA).is_err(),
                "version {bad:?} must fail"
            );
        }
        for bad in ["a948904", "", &SHA.to_uppercase(), &format!("{SHA}00")] {
            let error = Candidate::new("1.2.3", bad).expect_err("bad commit");
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
        assert!(candidate_module_file(&foreign, "1.2.3")
            .expect_err("foreign")
            .contains("name is [\"other\"]"));
        let twice = format!("{MODULE}module(name = \"rules_dx\", version = \"1\")\n");
        assert!(candidate_module_file(&twice, "1.2.3")
            .expect_err("two calls")
            .contains("2 module() calls"));
        let unversioned = "module(name = \"rules_dx\")\n";
        assert!(candidate_module_file(unversioned, "1.2.3")
            .expect_err("no version")
            .contains("0 version fields"));
        assert!(candidate_module_file("bazel_dep(name = \"x\")\n", "1.2.3")
            .expect_err("none")
            .contains("0 module() calls"));
        assert!(
            candidate_module_file("module(name = \"rules_dx\"\n", "1.2.3")
                .expect_err("open")
                .contains("not closed")
        );
        assert!(candidate_module_file(MODULE, "0.0.0").is_err());
    }

    #[test]
    fn remote_tag_commit_prefers_the_peeled_annotated_commit() {
        let output = format!("{SHA}\trefs/tags/v1.2.3\n{PEELED}\trefs/tags/v1.2.3^{{}}\n");
        assert_eq!(remote_tag_commit(&output, "v1.2.3"), Ok(PEELED.to_owned()));
    }

    #[test]
    fn remote_tag_commit_reads_a_lightweight_tag_exactly() {
        let output = format!(
            "{PEELED}\trefs/tags/v1.2.30\n{SHA}\trefs/tags/v1.2.3\n{PEELED}\trefs/heads/v1.2.3\n"
        );
        assert_eq!(remote_tag_commit(&output, "v1.2.3"), Ok(SHA.to_owned()));
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
            assert!(
                error.contains("malformed git ls-remote line"),
                "{bad:?}: {error}"
            );
        }
    }

    #[test]
    fn remote_tag_commit_rejects_duplicate_and_conflicting_lines() {
        let duplicate = format!("{SHA}\trefs/tags/v1.2.3\n{SHA}\trefs/tags/v1.2.3\n");
        assert!(remote_tag_commit(&duplicate, "v1.2.3")
            .expect_err("duplicate")
            .contains("ambiguous"));
        let conflicting =
            format!("{SHA}\trefs/tags/v1.2.3^{{}}\n{PEELED}\trefs/tags/v1.2.3^{{}}\n");
        assert!(remote_tag_commit(&conflicting, "v1.2.3")
            .expect_err("conflicting")
            .contains("ambiguous"));
    }

    #[test]
    fn remote_tag_commit_rejects_empty_output() {
        assert_eq!(
            remote_tag_commit("", "v1.2.3"),
            Err("release: remote has no tag v1.2.3".to_owned())
        );
    }

    #[test]
    fn local_tag_commit_peels_and_matches_exact_refs() {
        assert_eq!(local_tag_commit("", "v1.2.3"), Ok(None));
        assert_eq!(
            local_tag_commit(&format!("refs/tags/v1.2.3 {SHA} \n"), "v1.2.3"),
            Ok(Some(SHA.to_owned()))
        );
        assert_eq!(
            local_tag_commit(&format!("refs/tags/v1.2.3 {SHA} {PEELED}\n"), "v1.2.3"),
            Ok(Some(PEELED.to_owned()))
        );
        assert_eq!(
            local_tag_commit(&format!("refs/tags/v1.2.3/x {SHA} \n"), "v1.2.3"),
            Ok(None)
        );
        let twice = format!("refs/tags/v1.2.3 {SHA} \nrefs/tags/v1.2.3 {PEELED} \n");
        assert!(local_tag_commit(&twice, "v1.2.3")
            .expect_err("twice")
            .contains("ambiguous"));
        for bad in ["v1.2.3\n".to_owned(), format!("refs/tags/v1.2.3 xyz \n")] {
            assert!(local_tag_commit(&bad, "v1.2.3")
                .expect_err("malformed")
                .contains("malformed"));
        }
    }

    #[test]
    fn git_state_of_propagates_first_error_and_parses_ok() {
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
        assert_eq!(
            git_state_of(
                Err("first".to_owned()),
                Err("second".to_owned()),
                Ok(String::new()),
                "v1.2.3"
            )
            .err(),
            Some("first".to_owned())
        );
        assert_eq!(
            git_state_of(
                Ok(String::new()),
                Ok(SHA.to_owned()),
                Err("third".to_owned()),
                "v1.2.3"
            )
            .err(),
            Some("third".to_owned())
        );
        assert!(git_state_of(
            Ok(String::new()),
            Ok("HEAD".to_owned()),
            Ok(String::new()),
            "v1.2.3"
        )
        .expect_err("bad head")
        .contains("HEAD is"));
    }

    #[cfg(unix)]
    #[test]
    fn git_failure_is_an_error_not_clean() {
        use std::os::unix::process::ExitStatusExt;
        let failed = Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: Vec::new(),
            stderr: b"fatal: not a git repository\n".to_vec(),
        };
        assert_eq!(
            git_output_of(&["status", "--porcelain"], Ok(failed)),
            Err("git status --porcelain failed: fatal: not a git repository".to_owned())
        );
        let silent = Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        let err = git_output_of(&["tag"], Ok(silent)).expect_err("silent failure");
        assert!(err.starts_with("git tag failed: "), "{err}");
        let spawn = io::Error::new(io::ErrorKind::NotFound, "no git");
        assert_eq!(
            git_output_of(&["tag"], Err(spawn)),
            Err("git tag failed: no git".to_owned())
        );
        let binary = Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: vec![0xff],
            stderr: Vec::new(),
        };
        assert!(git_output_of(&["status"], Ok(binary)).is_err());
    }

    #[test]
    fn read_batch_rejects_missing_and_short_blobs() {
        let entries = vec![("a.txt".to_owned(), 0o644)];
        assert_eq!(
            read_batch(&b"0123 blob 2\nhi\n"[..], SHA, &entries),
            Ok(vec![b"hi".to_vec()])
        );
        assert!(read_batch(&b"a.txt missing\n"[..], SHA, &entries)
            .expect_err("missing")
            .contains("is missing"));
        assert!(read_batch(&b"0123 blob 5\nhi\n"[..], SHA, &entries)
            .expect_err("short")
            .contains("short read"));
        assert!(read_batch(&b"0123 blob 1\nhi\n"[..], SHA, &entries)
            .expect_err("size")
            .contains("size mismatch"));
        assert!(read_batch(&b"0123 tree 1\nh\n"[..], SHA, &entries)
            .expect_err("tree")
            .contains("bad header"));
        assert!(read_batch(&b"0123 blob x\nh\n"[..], SHA, &entries)
            .expect_err("size")
            .contains("bad size"));
    }
}
