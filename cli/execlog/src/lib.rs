//! Reader for Bazel's compact execution log (`--execution_log_compact_file`).
//!
//! The log is a zstd stream (one or more frames) of length-delimited
//! `ExecLogEntry` messages. Entries reference earlier entries by ID. Each
//! `Invocation` entry starts a new segment, and IDs resolve only inside the
//! segment that defines them, so logs joined from several Bazel runs read as
//! one [`Log`]. Entries are kept as decoded and resolved on demand: callers
//! build input maps only for the spawns they look at.
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use prost::Message;
use ruzstd::decoding::errors::{FrameDecoderError, ReadFrameHeaderError};
use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder};

pub mod diff;

pub use diff::{explain, CacheMiss, Change, ChangeKind, Delta, CHANGE_CAP};
pub use spawn_proto::tools::protos as proto;

use proto::exec_log_entry::{self as entry, Type};

/// Why an execution log could not be read or resolved.
#[derive(Debug, thiserror::Error)]
pub enum ExeclogError {
    #[error("execution log {0} does not exist")]
    Missing(PathBuf),
    #[error("execution log is not zstd-compressed")]
    NotZstd,
    #[error("execution log is truncated")]
    Truncated,
    #[error("execution log entry {0} does not decode")]
    BadEntry(usize),
    #[error("execution log references unknown entry {0}")]
    UnknownReference(u32),
    #[error("reading execution log: {0}")]
    Io(#[source] io::Error),
}

/// Flattened spawn inputs: path to content fingerprint. Files map to their
/// digest hash (empty for empty files), unresolved symlinks to `-> target`,
/// and empty directories to `<directory>`.
pub type InputMap = BTreeMap<String, String>;

/// A decoded execution log: one segment per Bazel invocation.
#[derive(Debug, Default)]
pub struct Log {
    segments: Vec<Segment>,
}

impl Log {
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Every spawn in log order, with the segment that resolves its IDs.
    pub fn spawns(&self) -> impl Iterator<Item = (&Segment, &entry::Spawn)> {
        self.segments
            .iter()
            .flat_map(|segment| segment.spawns.iter().map(move |spawn| (segment, spawn)))
    }
}

/// The entries of one Bazel invocation.
#[derive(Debug, Default)]
pub struct Segment {
    invocation: Option<entry::Invocation>,
    entries: HashMap<u32, Type>,
    spawns: Vec<entry::Spawn>,
    flat_sets: RefCell<HashMap<u32, Rc<InputMap>>>,
    resolving: RefCell<HashSet<u32>>,
}

impl Segment {
    pub fn invocation(&self) -> Option<&entry::Invocation> {
        self.invocation.as_ref()
    }

    pub fn spawns(&self) -> &[entry::Spawn] {
        &self.spawns
    }

    fn workspace_dir(&self) -> &str {
        match &self.invocation {
            Some(invocation) if !invocation.workspace_runfiles_directory.is_empty() => {
                &invocation.workspace_runfiles_directory
            }
            _ => "_main",
        }
    }

    fn entry(&self, id: u32) -> Result<&Type, ExeclogError> {
        self.entries
            .get(&id)
            .ok_or(ExeclogError::UnknownReference(id))
    }

    /// The spawn's first output path, if it declares any.
    pub fn primary_output(&self, spawn: &entry::Spawn) -> Result<Option<String>, ExeclogError> {
        let Some(output) = spawn.outputs.first() else {
            return Ok(None);
        };
        match &output.r#type {
            Some(entry::output::Type::OutputId(id)) => match self.entry(*id)? {
                Type::File(file) => Ok(Some(file.path.clone())),
                Type::Directory(dir) => Ok(Some(dir.path.clone())),
                Type::UnresolvedSymlink(link) => Ok(Some(link.path.clone())),
                _ => Err(ExeclogError::UnknownReference(*id)),
            },
            Some(entry::output::Type::InvalidOutputPath(path)) => Ok(Some(path.clone())),
            None => Ok(None),
        }
    }

    /// All inputs of a spawn, tools included.
    pub fn inputs(&self, spawn: &entry::Spawn) -> Result<InputMap, ExeclogError> {
        let mut map = InputMap::new();
        for id in [spawn.input_set_id, spawn.tool_set_id] {
            if id != 0 {
                let set = self.flatten_set(id)?;
                map.extend(set.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
        }
        Ok(map)
    }

    fn flatten_set(&self, id: u32) -> Result<Rc<InputMap>, ExeclogError> {
        if let Some(set) = self.flat_sets.borrow().get(&id) {
            return Ok(Rc::clone(set));
        }
        let Type::InputSet(set) = self.entry(id)? else {
            return Err(ExeclogError::UnknownReference(id));
        };
        if !self.resolving.borrow_mut().insert(id) {
            return Err(ExeclogError::UnknownReference(id));
        }
        let result = self.build_set(set);
        self.resolving.borrow_mut().remove(&id);
        let map = Rc::new(result?);
        self.flat_sets.borrow_mut().insert(id, Rc::clone(&map));
        Ok(map)
    }

    fn build_set(&self, set: &entry::InputSet) -> Result<InputMap, ExeclogError> {
        let mut map = InputMap::new();
        for transitive in &set.transitive_set_ids {
            let inner = self.flatten_set(*transitive)?;
            map.extend(inner.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        for input in &set.input_ids {
            self.add_artifact(*input, None, &mut map)?;
        }
        Ok(map)
    }

    /// Adds a file, directory, unresolved symlink or runfiles tree, at its
    /// own path or at `at` when it is the target of a runfiles symlink.
    fn add_artifact(
        &self,
        id: u32,
        at: Option<&str>,
        map: &mut InputMap,
    ) -> Result<(), ExeclogError> {
        match self.entry(id)? {
            Type::File(file) => {
                map.insert(
                    at.unwrap_or(&file.path).to_owned(),
                    digest_text(&file.digest),
                );
            }
            Type::Directory(dir) => {
                let root = at.unwrap_or(&dir.path);
                if dir.files.is_empty() {
                    map.insert(root.to_owned(), "<directory>".to_owned());
                }
                for file in &dir.files {
                    map.insert(format!("{root}/{}", file.path), digest_text(&file.digest));
                }
            }
            Type::UnresolvedSymlink(link) => {
                map.insert(
                    at.unwrap_or(&link.path).to_owned(),
                    format!("-> {}", link.target_path),
                );
            }
            Type::RunfilesTree(tree) if at.is_none() => self.add_runfiles(tree, map)?,
            _ => return Err(ExeclogError::UnknownReference(id)),
        }
        Ok(())
    }

    /// Reassembles a runfiles tree in Bazel's order, later parts winning:
    /// symlinks, canonical artifacts, empty files, root symlinks, repo mapping.
    fn add_runfiles(
        &self,
        tree: &entry::RunfilesTree,
        map: &mut InputMap,
    ) -> Result<(), ExeclogError> {
        let root = tree.path.as_str();
        let ws = self.workspace_dir();
        for (rel, target) in self.flatten_symlinks(tree.symlinks_id)? {
            self.add_artifact(target, Some(&format!("{root}/{ws}/{rel}")), map)?;
        }
        if tree.input_set_id != 0 {
            for (path, digest) in self.flatten_set(tree.input_set_id)?.iter() {
                map.insert(
                    format!("{root}/{}", runfiles_path(path, ws)),
                    digest.clone(),
                );
            }
        }
        for rel in &tree.empty_files {
            map.insert(format!("{root}/{ws}/{rel}"), String::new());
        }
        for (rel, target) in self.flatten_symlinks(tree.root_symlinks_id)? {
            self.add_artifact(target, Some(&format!("{root}/{rel}")), map)?;
        }
        if let Some(manifest) = &tree.repo_mapping_manifest {
            map.insert(
                format!("{root}/_repo_mapping"),
                digest_text(&manifest.digest),
            );
        }
        Ok(())
    }

    fn flatten_symlinks(&self, id: u32) -> Result<BTreeMap<String, u32>, ExeclogError> {
        let mut out = BTreeMap::new();
        if id != 0 {
            let mut seen = HashSet::new();
            self.collect_symlinks(id, &mut out, &mut seen)?;
        }
        Ok(out)
    }

    fn collect_symlinks(
        &self,
        id: u32,
        out: &mut BTreeMap<String, u32>,
        seen: &mut HashSet<u32>,
    ) -> Result<(), ExeclogError> {
        let Type::SymlinkEntrySet(set) = self.entry(id)? else {
            return Err(ExeclogError::UnknownReference(id));
        };
        if !seen.insert(id) {
            return Ok(());
        }
        for transitive in &set.transitive_set_ids {
            self.collect_symlinks(*transitive, out, seen)?;
        }
        for (rel, target) in &set.direct_entries {
            out.insert(rel.clone(), *target);
        }
        Ok(())
    }
}

fn digest_text(digest: &Option<proto::Digest>) -> String {
    digest
        .as_ref()
        .map(|digest| digest.hash.clone())
        .unwrap_or_default()
}

/// The path of an artifact inside a runfiles tree, from its exec path.
fn runfiles_path(exec_path: &str, ws: &str) -> String {
    let mut path = exec_path;
    if let Some(rest) = path.strip_prefix("bazel-out/") {
        if let Some((_, rest)) = rest.split_once('/') {
            if let Some((_, rest)) = rest.split_once('/') {
                path = rest;
            }
        }
    }
    if let Some(rest) = path.strip_prefix("external/") {
        return rest.to_owned();
    }
    if let Some(rest) = path.strip_prefix("../") {
        return rest.to_owned();
    }
    format!("{ws}/{path}")
}

/// Reads the execution log at `path`.
pub fn read(path: &Path) -> Result<Log, ExeclogError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Err(ExeclogError::Missing(path.to_path_buf()));
        }
        Err(err) => return Err(ExeclogError::Io(err)),
    };
    decode(BufReader::new(file))
}

/// Encodes entries as one zstd frame of length-delimited messages.
pub fn encode(entries: &[proto::ExecLogEntry]) -> Vec<u8> {
    let mut raw = Vec::new();
    for entry in entries {
        raw.extend(entry.encode_length_delimited_to_vec());
    }
    ruzstd::encoding::compress_to_vec(raw.as_slice(), ruzstd::encoding::CompressionLevel::Fastest)
}

/// Decodes an execution log from a zstd stream.
pub fn decode<R: BufRead>(source: R) -> Result<Log, ExeclogError> {
    let mut frames = BufReader::new(Frames::new(source));
    let result = decode_entries(&mut frames);
    match (result, frames.get_mut().failure.take()) {
        (_, Some(failure)) => Err(failure),
        (result, None) => result,
    }
}

fn decode_entries<R: BufRead>(reader: &mut BufReader<Frames<R>>) -> Result<Log, ExeclogError> {
    let mut log = Log::default();
    let mut buf = Vec::new();
    let mut index = 0usize;
    while let Some(len) = read_varint(reader)? {
        let len = usize::try_from(len).map_err(|_| ExeclogError::BadEntry(index))?;
        buf.resize(len, 0);
        reader.read_exact(&mut buf).map_err(eof_is_truncated)?;
        let entry = proto::ExecLogEntry::decode(buf.as_slice())
            .map_err(|_| ExeclogError::BadEntry(index))?;
        add_entry(&mut log, entry);
        index += 1;
    }
    if index == 0 && reader.get_ref().frames == 0 {
        return Err(ExeclogError::Truncated);
    }
    Ok(log)
}

fn add_entry(log: &mut Log, entry: proto::ExecLogEntry) {
    let Some(payload) = entry.r#type else {
        return;
    };
    if let Type::Invocation(invocation) = payload {
        log.segments.push(Segment {
            invocation: Some(invocation),
            ..Segment::default()
        });
        return;
    }
    if log.segments.is_empty() {
        log.segments.push(Segment::default());
    }
    let Some(segment) = log.segments.last_mut() else {
        return;
    };
    match payload {
        Type::Spawn(spawn) => segment.spawns.push(spawn),
        payload if entry.id != 0 => {
            segment.entries.insert(entry.id, payload);
        }
        _ => {}
    }
}

fn eof_is_truncated(err: io::Error) -> ExeclogError {
    if err.kind() == io::ErrorKind::UnexpectedEof {
        ExeclogError::Truncated
    } else {
        ExeclogError::Io(err)
    }
}

/// Reads a protobuf varint; `None` at a clean end of stream.
fn read_varint(reader: &mut impl BufRead) -> Result<Option<u64>, ExeclogError> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let mut byte = [0u8; 1];
        let read = loop {
            match reader.read(&mut byte) {
                Ok(read) => break read,
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(ExeclogError::Io(err)),
            }
        };
        if read == 0 {
            return if shift == 0 {
                Ok(None)
            } else {
                Err(ExeclogError::Truncated)
            };
        }
        value |= u64::from(byte[0] & 0x7f) << shift;
        if byte[0] & 0x80 == 0 {
            return Ok(Some(value));
        }
    }
    Err(ExeclogError::Truncated)
}

/// The source, noting whether a read ever came back empty.
struct Tracked<R> {
    inner: R,
    eof: bool,
}

impl<R: BufRead> Read for Tracked<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        if read == 0 && !buf.is_empty() {
            self.eof = true;
        }
        Ok(read)
    }
}

/// Decompresses consecutive zstd frames, skipping skippable frames.
struct Frames<R> {
    source: Tracked<R>,
    decoder: FrameDecoder,
    active: bool,
    frames: usize,
    failure: Option<ExeclogError>,
}

impl<R: BufRead> Frames<R> {
    fn new(source: R) -> Self {
        Frames {
            source: Tracked {
                inner: source,
                eof: false,
            },
            decoder: FrameDecoder::new(),
            active: false,
            frames: 0,
            failure: None,
        }
    }

    fn fail(&mut self, err: FrameDecoderError) -> io::Error {
        let failure = match err {
            FrameDecoderError::ReadFrameHeaderError(ReadFrameHeaderError::BadMagicNumber(_)) => {
                ExeclogError::NotZstd
            }
            _ if self.source.eof => ExeclogError::Truncated,
            _ => ExeclogError::NotZstd,
        };
        let message = failure.to_string();
        self.failure = Some(failure);
        io::Error::new(io::ErrorKind::InvalidData, message)
    }

    fn skip(&mut self, length: u32) -> io::Result<()> {
        let skipped = io::copy(
            &mut (&mut self.source).take(u64::from(length)),
            &mut io::sink(),
        )?;
        if skipped < u64::from(length) {
            self.failure = Some(ExeclogError::Truncated);
            return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated"));
        }
        Ok(())
    }
}

impl<R: BufRead> Read for Frames<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.active {
                if self.decoder.can_collect() > 0 {
                    return self.decoder.read(buf);
                }
                if self.decoder.is_finished() {
                    self.active = false;
                    continue;
                }
                if let Err(err) = self
                    .decoder
                    .decode_blocks(&mut self.source, BlockDecodingStrategy::UptoBlocks(1))
                {
                    return Err(self.fail(err));
                }
                continue;
            }
            if self.source.inner.fill_buf()?.is_empty() {
                return Ok(0);
            }
            match self.decoder.reset(&mut self.source) {
                Ok(()) => {
                    self.active = true;
                    self.frames += 1;
                }
                Err(FrameDecoderError::ReadFrameHeaderError(ReadFrameHeaderError::SkipFrame {
                    length,
                    ..
                })) => self.skip(length)?,
                Err(err) => return Err(self.fail(err)),
            }
        }
    }
}

#[cfg(test)]
mod tests;
