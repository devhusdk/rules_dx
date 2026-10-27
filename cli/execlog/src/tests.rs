use super::*;
use proto::exec_log_entry::{
    Directory, File as FileEntry, InputSet, Invocation, Output, RunfilesTree, Spawn,
    SymlinkEntrySet, UnresolvedSymlink,
};
use proto::{Digest, ExecLogEntry};
use ruzstd::encoding::{compress_to_vec, CompressionLevel};

pub(crate) fn entry(id: u32, payload: Type) -> ExecLogEntry {
    ExecLogEntry {
        id,
        r#type: Some(payload),
    }
}

pub(crate) fn invocation(id: &str) -> ExecLogEntry {
    entry(
        0,
        Type::Invocation(Invocation {
            hash_function_name: "SHA-256".to_owned(),
            workspace_runfiles_directory: "_main".to_owned(),
            id: id.to_owned(),
        }),
    )
}

pub(crate) fn file(id: u32, path: &str, hash: &str) -> ExecLogEntry {
    entry(
        id,
        Type::File(FileEntry {
            path: path.to_owned(),
            digest: Some(Digest {
                hash: hash.to_owned(),
                size_bytes: 1,
                hash_function_name: String::new(),
            }),
        }),
    )
}

pub(crate) fn set(id: u32, inputs: &[u32], transitive: &[u32]) -> ExecLogEntry {
    entry(
        id,
        Type::InputSet(InputSet {
            input_ids: inputs.to_vec(),
            transitive_set_ids: transitive.to_vec(),
        }),
    )
}

pub(crate) fn spawn(input_set: u32, output: u32, mnemonic: &str) -> Spawn {
    Spawn {
        args: vec!["tool".to_owned()],
        input_set_id: input_set,
        outputs: (output != 0)
            .then(|| Output {
                r#type: Some(entry::output::Type::OutputId(output)),
            })
            .into_iter()
            .collect(),
        target_label: "//pkg:target".to_owned(),
        mnemonic: mnemonic.to_owned(),
        cacheable: true,
        remote_cacheable: true,
        ..Spawn::default()
    }
}

pub(crate) fn frame(entries: &[ExecLogEntry]) -> Vec<u8> {
    encode(entries)
}

pub(crate) fn log(entries: &[ExecLogEntry]) -> Log {
    decode(frame(entries).as_slice()).expect("decode")
}

fn decode_err(bytes: &[u8]) -> ExeclogError {
    decode(bytes).expect_err("must fail")
}

#[test]
fn reads_spawns_and_resolves_inputs() {
    let log = log(&[
        invocation("a"),
        file(1, "src/a.rs", "h1"),
        file(2, "bazel-out/k8/bin/liba.rlib", "out"),
        set(3, &[1], &[]),
        entry(0, Type::Spawn(spawn(3, 2, "Rustc"))),
    ]);
    assert_eq!(log.segments().len(), 1);
    let (segment, spawn) = log.spawns().next().expect("spawn");
    assert_eq!(segment.invocation().expect("inv").id, "a");
    assert_eq!(
        segment.primary_output(spawn).expect("output").as_deref(),
        Some("bazel-out/k8/bin/liba.rlib")
    );
    let inputs = segment.inputs(spawn).expect("inputs");
    assert_eq!(inputs.get("src/a.rs").map(String::as_str), Some("h1"));
}

#[test]
fn each_invocation_starts_a_segment_with_its_own_ids() {
    let mut bytes = frame(&[
        invocation("first"),
        file(1, "a", "h1"),
        set(2, &[1], &[]),
        entry(0, Type::Spawn(spawn(2, 1, "A"))),
    ]);
    bytes.extend(frame(&[
        invocation("second"),
        file(1, "b", "h2"),
        set(2, &[1], &[]),
        entry(0, Type::Spawn(spawn(2, 1, "B"))),
    ]));
    let log = decode(bytes.as_slice()).expect("decode");
    assert_eq!(log.segments().len(), 2);
    let inputs: Vec<InputMap> = log
        .spawns()
        .map(|(segment, spawn)| segment.inputs(spawn).expect("inputs"))
        .collect();
    assert!(inputs[0].contains_key("a"));
    assert!(inputs[1].contains_key("b"));
    assert!(!inputs[1].contains_key("a"));
}

#[test]
fn skippable_frames_are_ignored() {
    let mut bytes = vec![0x50, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 9, 9, 9];
    bytes.extend(frame(&[invocation("x")]));
    let log = decode(bytes.as_slice()).expect("decode");
    assert_eq!(log.segments().len(), 1);
}

#[test]
fn unknown_references_fail_on_resolution() {
    let log = log(&[
        invocation("a"),
        set(3, &[7], &[]),
        entry(0, Type::Spawn(spawn(3, 9, "Rustc"))),
    ]);
    let (segment, spawn) = log.spawns().next().expect("spawn");
    assert!(matches!(
        segment.primary_output(spawn),
        Err(ExeclogError::UnknownReference(9))
    ));
    assert!(matches!(
        segment.inputs(spawn),
        Err(ExeclogError::UnknownReference(7))
    ));
}

#[test]
fn self_referencing_sets_do_not_recurse_forever() {
    let log = log(&[set(3, &[], &[3]), entry(0, Type::Spawn(spawn(3, 0, "X")))]);
    let (segment, spawn) = log.spawns().next().expect("spawn");
    assert!(matches!(
        segment.inputs(spawn),
        Err(ExeclogError::UnknownReference(3))
    ));
}

#[test]
fn not_zstd_truncated_and_bad_entries_are_told_apart() {
    assert!(matches!(
        decode_err(b"plain text log"),
        ExeclogError::NotZstd
    ));
    assert!(matches!(decode_err(b""), ExeclogError::Truncated));
    let whole = frame(&[invocation("a"), file(1, "a", "h")]);
    assert!(matches!(
        decode_err(&whole[..whole.len() - 3]),
        ExeclogError::Truncated
    ));
    let mut raw = Vec::new();
    invocation("a")
        .encode_length_delimited(&mut raw)
        .expect("encode");
    raw.extend([5, 0xff, 0xff, 0xff, 0xff, 0xff]);
    let bad = compress_to_vec(raw.as_slice(), CompressionLevel::Fastest);
    assert!(matches!(decode_err(&bad), ExeclogError::BadEntry(1)));
    let mut cut = Vec::new();
    invocation("a")
        .encode_length_delimited(&mut cut)
        .expect("encode");
    cut.extend([40, 1, 2]);
    let short = compress_to_vec(cut.as_slice(), CompressionLevel::Fastest);
    assert!(matches!(decode_err(&short), ExeclogError::Truncated));
}

#[test]
fn missing_file_is_reported() {
    let dir = std::env::temp_dir().join("dx_execlog_missing_test");
    let path = dir.join("nope.binpb.zst");
    assert!(matches!(read(&path), Err(ExeclogError::Missing(p)) if p == path));
}

#[test]
fn entries_without_a_known_type_are_skipped() {
    let mut raw = Vec::new();
    // Field 8 (symlink_action) is not vendored.
    let unknown = [0x42u8, 2, 0x0a, 0];
    raw.push(unknown.len() as u8);
    raw.extend(unknown);
    invocation("a")
        .encode_length_delimited(&mut raw)
        .expect("encode");
    let log = decode(compress_to_vec(raw.as_slice(), CompressionLevel::Fastest).as_slice())
        .expect("decode");
    assert_eq!(log.segments().len(), 1);
}

#[test]
fn directories_symlinks_and_runfiles_flatten() {
    let log = log(&[
        invocation("a"),
        file(1, "bazel-out/k8/bin/pkg/data.txt", "d"),
        entry(
            2,
            Type::Directory(Directory {
                path: "pkg/tree".to_owned(),
                files: vec![FileEntry {
                    path: "x.txt".to_owned(),
                    digest: None,
                }],
            }),
        ),
        entry(
            3,
            Type::UnresolvedSymlink(UnresolvedSymlink {
                path: "pkg/link".to_owned(),
                target_path: "../elsewhere".to_owned(),
            }),
        ),
        file(4, "external/repo/lib.txt", "r"),
        set(5, &[1, 4], &[]),
        file(6, "pkg/custom.txt", "c"),
        entry(
            7,
            Type::SymlinkEntrySet(SymlinkEntrySet {
                direct_entries: [("alias.txt".to_owned(), 6)].into_iter().collect(),
                transitive_set_ids: vec![],
            }),
        ),
        entry(
            8,
            Type::RunfilesTree(RunfilesTree {
                path: "bazel-out/k8/bin/pkg/tool.runfiles".to_owned(),
                input_set_id: 5,
                symlinks_id: 7,
                root_symlinks_id: 0,
                empty_files: vec!["empty.py".to_owned()],
                repo_mapping_manifest: Some(FileEntry {
                    path: String::new(),
                    digest: Some(Digest {
                        hash: "m".to_owned(),
                        size_bytes: 1,
                        hash_function_name: String::new(),
                    }),
                }),
            }),
        ),
        file(9, "out", "o"),
        set(10, &[2, 3], &[]),
        set(11, &[8], &[10]),
        entry(0, Type::Spawn(spawn(11, 9, "Run"))),
    ]);
    let (segment, spawn) = log.spawns().next().expect("spawn");
    let inputs = segment.inputs(spawn).expect("inputs");
    let root = "bazel-out/k8/bin/pkg/tool.runfiles";
    let want: InputMap = [
        ("pkg/tree/x.txt".to_owned(), String::new()),
        ("pkg/link".to_owned(), "-> ../elsewhere".to_owned()),
        (format!("{root}/_main/pkg/data.txt"), "d".to_owned()),
        (format!("{root}/repo/lib.txt"), "r".to_owned()),
        (format!("{root}/_main/alias.txt"), "c".to_owned()),
        (format!("{root}/_main/empty.py"), String::new()),
        (format!("{root}/_repo_mapping"), "m".to_owned()),
    ]
    .into_iter()
    .collect();
    assert_eq!(inputs, want);
}

#[test]
fn tool_set_members_are_inputs() {
    let mut tool_spawn = spawn(0, 1, "Genrule");
    tool_spawn.tool_set_id = 3;
    let log = log(&[
        file(1, "out", "o"),
        file(2, "tools/gen.sh", "t"),
        set(3, &[2], &[]),
        entry(0, Type::Spawn(tool_spawn)),
    ]);
    let (segment, spawn) = log.spawns().next().expect("spawn");
    assert!(segment
        .inputs(spawn)
        .expect("inputs")
        .contains_key("tools/gen.sh"));
}
