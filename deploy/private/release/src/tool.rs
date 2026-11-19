//! `release_tool <kind> <args...>`: dispatch for the release generators.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use dx_atomic_fs::path_io::{self, PathIoError};

use crate::inventory::InventoryError;

const PROG: &str = "release_tool";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Spdx,
    Provenance,
    BcrSource,
    Notice,
    Inventory,
}

/// How many arguments a kind takes after its name.
#[derive(Clone, Copy)]
enum Arity {
    Exactly(usize),
    AtLeast(usize),
    Between(usize, usize),
}

impl Arity {
    fn accepts(self, count: usize) -> bool {
        match self {
            Self::Exactly(n) => count == n,
            Self::AtLeast(n) => count >= n,
            Self::Between(low, high) => (low..=high).contains(&count),
        }
    }
}

struct KindSpec {
    kind: Kind,
    name: &'static str,
    usage: &'static str,
    arity: Arity,
}

const KINDS: &[KindSpec] = &[
    KindSpec {
        kind: Kind::Spdx,
        name: "spdx",
        usage: "<artifact> <out> <package> <supplier> [inventory] [--runtimes=<runtimes.json>] [--lock=<npm|go|maven|pip>:<lockfile> ...]",
        arity: Arity::AtLeast(4),
    },
    KindSpec {
        kind: Kind::Provenance,
        name: "provenance",
        usage: "<artifact> <out> <builder_id>",
        arity: Arity::Exactly(3),
    },
    KindSpec {
        kind: Kind::BcrSource,
        name: "bcr-source",
        usage: "<out> <module> <version>",
        arity: Arity::Exactly(3),
    },
    KindSpec {
        kind: Kind::Notice,
        name: "notice",
        usage: "<manifest> <out> <root> [--runtimes=<runtimes.json>] <text|@list> [<text|@list> ...]",
        arity: Arity::AtLeast(4),
    },
    KindSpec {
        kind: Kind::Inventory,
        name: "inventory",
        usage: "<shipped.json> <cargo-bazel-lock.json> <license.toml> <manifest> <texts_dir> [<runtimes.json>]",
        arity: Arity::Between(5, 6),
    },
];

#[derive(Debug, PartialEq, Eq)]
enum Invocation {
    Spdx {
        src: PathBuf,
        dst: PathBuf,
        package: String,
        supplier: String,
        inventory: Option<PathBuf>,
        runtimes: Option<PathBuf>,
        locks: Vec<(String, PathBuf)>,
    },
    Provenance {
        src: PathBuf,
        dst: PathBuf,
        builder: String,
    },
    BcrSource {
        dst: PathBuf,
        module: String,
        version: String,
    },
    Notice {
        manifest: PathBuf,
        dst: PathBuf,
        root: String,
        texts: Vec<PathBuf>,
        runtimes: Option<PathBuf>,
    },
    Inventory {
        shipped: PathBuf,
        lock: PathBuf,
        licenses: PathBuf,
        dst: PathBuf,
        texts: PathBuf,
        runtimes: Option<PathBuf>,
    },
}

impl Invocation {
    fn dst(&self) -> &Path {
        match self {
            Self::Spdx { dst, .. }
            | Self::Provenance { dst, .. }
            | Self::BcrSource { dst, .. }
            | Self::Notice { dst, .. }
            | Self::Inventory { dst, .. } => dst,
        }
    }
}

/// Why argv did not parse: no known kind, or a known kind with bad arity.
#[derive(Debug, PartialEq, Eq)]
enum ParseError {
    UnknownKind,
    Arity {
        name: &'static str,
        usage: &'static str,
    },
    BadFlag(String),
}

fn spec_for(name: &str) -> Option<&'static KindSpec> {
    KINDS.iter().find(|spec| spec.name == name)
}

/// Parses `[prog, kind, args...]`.
fn parse(argv: &[String]) -> Result<Invocation, ParseError> {
    let spec = argv
        .get(1)
        .and_then(|name| spec_for(name))
        .ok_or(ParseError::UnknownKind)?;
    let rest = argv.get(2..).ok_or(ParseError::UnknownKind)?;
    let arity_error = || ParseError::Arity {
        name: spec.name,
        usage: spec.usage,
    };
    if !spec.arity.accepts(rest.len()) {
        return Err(arity_error());
    }
    if spec.kind == Kind::Spdx {
        for arg in rest.iter().skip(4).filter(|arg| arg.starts_with("--")) {
            let known = arg.starts_with("--runtimes=")
                || arg.strip_prefix("--lock=").is_some_and(|spec| {
                    spec.split_once(':')
                        .is_some_and(|(kind, path)| !kind.is_empty() && !path.is_empty())
                });
            if !known {
                return Err(ParseError::BadFlag(arg.clone()));
            }
        }
    }
    Ok(match spec.kind {
        Kind::Spdx => {
            let [src, dst, package, supplier, extra @ ..] = rest else {
                return Err(arity_error());
            };
            let runtimes = extra
                .iter()
                .find_map(|arg| arg.strip_prefix("--runtimes="))
                .map(PathBuf::from);
            let inventory = extra
                .iter()
                .find(|arg| !arg.starts_with("--"))
                .map(PathBuf::from);
            let locks = extra
                .iter()
                .filter_map(|arg| arg.strip_prefix("--lock="))
                .filter_map(|spec| spec.split_once(':'))
                .map(|(kind, path)| (kind.to_owned(), PathBuf::from(path)))
                .collect();
            Invocation::Spdx {
                src: PathBuf::from(src),
                dst: PathBuf::from(dst),
                package: package.clone(),
                supplier: supplier.clone(),
                inventory,
                runtimes,
                locks,
            }
        }
        Kind::Provenance => {
            let [src, dst, builder, ..] = rest else {
                return Err(arity_error());
            };
            Invocation::Provenance {
                src: PathBuf::from(src),
                dst: PathBuf::from(dst),
                builder: builder.clone(),
            }
        }
        Kind::BcrSource => {
            let [dst, module, version, ..] = rest else {
                return Err(arity_error());
            };
            Invocation::BcrSource {
                dst: PathBuf::from(dst),
                module: module.clone(),
                version: version.clone(),
            }
        }
        Kind::Notice => {
            let [manifest, dst, root, third, ..] = rest else {
                return Err(arity_error());
            };
            let runtimes = third.strip_prefix("--runtimes=").map(PathBuf::from);
            let texts_from = if runtimes.is_some() { 4 } else { 3 };
            Invocation::Notice {
                manifest: PathBuf::from(manifest),
                dst: PathBuf::from(dst),
                root: root.clone(),
                texts: rest
                    .get(texts_from..)
                    .unwrap_or(&[])
                    .iter()
                    .map(PathBuf::from)
                    .collect(),
                runtimes,
            }
        }
        Kind::Inventory => {
            let [shipped, lock, licenses, dst, texts, rest @ ..] = rest else {
                return Err(arity_error());
            };
            Invocation::Inventory {
                shipped: PathBuf::from(shipped),
                lock: PathBuf::from(lock),
                licenses: PathBuf::from(licenses),
                dst: PathBuf::from(dst),
                texts: PathBuf::from(texts),
                runtimes: rest.first().map(PathBuf::from),
            }
        }
    })
}

fn all_kinds_usage() -> String {
    KINDS
        .iter()
        .map(|spec| format!("{PROG} {} {}", spec.name, spec.usage))
        .collect::<Vec<_>>()
        .join("\n       ")
}

fn usage_for(error: &ParseError) -> i32 {
    match error {
        ParseError::UnknownKind => crate::bin_error(format!("usage: {}", all_kinds_usage())),
        ParseError::Arity { name, usage } => crate::bin_usage(&format!("{PROG} {name}"), usage),
        ParseError::BadFlag(flag) => crate::bin_error(format!(
            "unknown or malformed spdx flag {flag}; use --runtimes=<path> or --lock=<kind>:<path>"
        )),
    }
}

fn kind_name(invocation: &Invocation) -> &'static str {
    match invocation {
        Invocation::Spdx { .. } => "spdx",
        Invocation::Provenance { .. } => "provenance",
        Invocation::BcrSource { .. } => "bcr-source",
        Invocation::Notice { .. } => "notice",
        Invocation::Inventory { .. } => "inventory",
    }
}

/// A release generator that could not read its inputs or write its output.
#[derive(Debug)]
enum ToolError {
    Io(io::Error),
    TextList(PathIoError),
    Inventory(InventoryError),
    Sbom(crate::SbomError),
    Notice(crate::NoticeError),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(f, "{source}"),
            Self::TextList(source) => write!(f, "{source}"),
            Self::Inventory(source) => write!(f, "{source}"),
            Self::Sbom(source) => write!(f, "{source}"),
            Self::Notice(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for ToolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::TextList(source) => Some(source),
            Self::Inventory(source) => Some(source),
            Self::Sbom(source) => Some(source),
            Self::Notice(source) => Some(source),
        }
    }
}

impl From<io::Error> for ToolError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

impl From<crate::SbomError> for ToolError {
    fn from(source: crate::SbomError) -> Self {
        Self::Sbom(source)
    }
}

impl From<crate::NoticeError> for ToolError {
    fn from(source: crate::NoticeError) -> Self {
        Self::Notice(source)
    }
}

/// Replaces each `@<list>` entry with the newline-separated paths in that file.
fn expand_text_lists(texts: &[PathBuf]) -> Result<Vec<PathBuf>, PathIoError> {
    let mut out = Vec::with_capacity(texts.len());
    for text in texts {
        match text.to_str().and_then(|text| text.strip_prefix('@')) {
            Some(list) => {
                let body = path_io::read_to_string(Path::new(list))?;
                out.extend(
                    body.lines()
                        .filter(|line| !line.is_empty())
                        .map(PathBuf::from),
                );
            }
            None => out.push(text.clone()),
        }
    }
    Ok(out)
}

fn execute(invocation: &Invocation) -> Result<(), ToolError> {
    match invocation {
        Invocation::Spdx {
            src,
            dst,
            package,
            supplier,
            inventory,
            runtimes,
            locks,
        } => crate::write_spdx_locked(
            src,
            dst,
            package,
            supplier,
            inventory.as_deref(),
            runtimes.as_deref(),
            locks,
        )?,
        Invocation::Provenance { src, dst, builder } => {
            crate::write_provenance(src, dst, builder)?;
        }
        Invocation::BcrSource {
            dst,
            module,
            version,
        } => crate::write_bcr_source(dst, module, version)?,
        Invocation::Notice {
            manifest,
            dst,
            root,
            texts,
            runtimes,
        } => {
            let texts = expand_text_lists(texts).map_err(ToolError::TextList)?;
            crate::write_notice(manifest, dst, root, &texts, runtimes.as_deref())?;
        }
        Invocation::Inventory {
            shipped,
            lock,
            licenses,
            dst,
            texts,
            runtimes,
        } => crate::inventory::write_inventory(
            shipped,
            lock,
            licenses,
            dst,
            texts,
            runtimes.as_deref(),
        )
        .map_err(ToolError::Inventory)?,
    }
    Ok(())
}

/// Runs `release_tool <kind> <args...>` and returns the process exit code.
/// `argv[0]` is the program name, as in `std::env::args()`.
#[must_use]
pub fn release_tool_run(argv: &[String]) -> i32 {
    let invocation = match parse(argv) {
        Ok(invocation) => invocation,
        Err(error) => return usage_for(&error),
    };
    match execute(&invocation) {
        Ok(()) => 0,
        Err(error) => crate::bin_cannot_write(
            &format!("{PROG} {}", kind_name(&invocation)),
            invocation.dst(),
            error,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_ecosystems::Ecosystem;

    fn argv_of(parts: &[&str]) -> Vec<String> {
        parts.iter().map(ToString::to_string).collect()
    }

    fn argv_for(kind: &str, count: usize) -> Vec<String> {
        let mut argv = argv_of(&[PROG, kind]);
        argv.extend((0..count).map(|i| format!("a{i}")));
        argv
    }

    #[test]
    fn arity_is_checked_per_kind() {
        for (name, exact, unbounded) in [
            ("provenance", 3, false),
            ("bcr-source", 3, false),
            ("notice", 4, true),
            ("inventory", 5, true),
        ] {
            assert!(parse(&argv_for(name, exact)).is_ok(), "{name} exact");
            assert_eq!(
                parse(&argv_for(name, 0)),
                Err(ParseError::Arity {
                    name,
                    usage: spec_for(name).expect("kind").usage,
                }),
                "{name} empty"
            );
            assert_eq!(
                parse(&argv_for(name, exact - 1)),
                Err(ParseError::Arity {
                    name,
                    usage: spec_for(name).expect("kind").usage,
                }),
                "{name} short"
            );
            assert_eq!(
                parse(&argv_for(name, exact + 1)).is_ok(),
                unbounded,
                "{name} long"
            );
        }
        assert!(parse(&argv_for("notice", 9)).is_ok());
        assert!(parse(&argv_for("spdx", 4)).is_ok());
        assert!(parse(&argv_for("spdx", 5)).is_ok());
        assert!(parse(&argv_for("spdx", 3)).is_err());
        assert!(parse(&argv_for("spdx", 6)).is_ok());
        assert!(parse(&argv_for("spdx", 7)).is_ok());
    }

    #[test]
    fn spdx_rejects_malformed_locks_and_unknown_flags() {
        let mut argv = argv_for("spdx", 4);
        argv.push("--lock=npm:package-lock.json".to_owned());
        argv.push("--runtimes=r.json".to_owned());
        assert!(parse(&argv).is_ok());
        for bad in [
            "--lock=package-lock.json",
            "--lock=npm:",
            "--lock=:x",
            "--bogus",
        ] {
            let mut argv = argv_for("spdx", 4);
            argv.push(bad.to_owned());
            assert_eq!(
                parse(&argv),
                Err(ParseError::BadFlag(bad.to_owned())),
                "{bad}"
            );
        }
    }

    #[test]
    fn every_kind_in_the_table_is_reachable() {
        for spec in KINDS {
            assert_eq!(spec_for(spec.name).map(|found| found.kind), Some(spec.kind));
            assert!(all_kinds_usage().contains(spec.usage));
        }
    }

    #[test]
    fn spdx_keeps_all_fields() {
        assert_eq!(
            parse(&argv_of(&[
                PROG, "spdx", "artifact", "out", "dx", "rules_dx"
            ])),
            Ok(Invocation::Spdx {
                src: PathBuf::from("artifact"),
                dst: PathBuf::from("out"),
                package: "dx".to_owned(),
                supplier: "rules_dx".to_owned(),
                inventory: None,
                runtimes: None,
                locks: Vec::new(),
            })
        );
    }

    #[test]
    fn spdx_collects_lock_flags() {
        let go_manifest = Ecosystem::Go.manifest_name();
        let Ok(Invocation::Spdx {
            inventory, locks, ..
        }) = parse(&argv_of(&[
            PROG,
            "spdx",
            "artifact",
            "out",
            "dx",
            "rules_dx",
            "--lock=npm:package-lock.json",
            "inv",
            &format!("--lock=go:{go_manifest}"),
        ]))
        else {
            panic!("spdx did not parse");
        };
        assert_eq!(inventory, Some(PathBuf::from("inv")));
        assert_eq!(
            locks,
            vec![
                ("npm".to_owned(), PathBuf::from("package-lock.json")),
                ("go".to_owned(), PathBuf::from(go_manifest)),
            ]
        );
    }

    #[test]
    fn provenance_keeps_all_fields() {
        assert_eq!(
            parse(&argv_of(&[
                PROG,
                "provenance",
                "artifact",
                "out",
                "builder"
            ])),
            Ok(Invocation::Provenance {
                src: PathBuf::from("artifact"),
                dst: PathBuf::from("out"),
                builder: "builder".to_owned(),
            })
        );
    }

    #[test]
    fn bcr_source_keeps_all_fields() {
        assert_eq!(
            parse(&argv_of(&[PROG, "bcr-source", "out", "rules_dx", "1.2.3"])),
            Ok(Invocation::BcrSource {
                dst: PathBuf::from("out"),
                module: "rules_dx".to_owned(),
                version: "1.2.3".to_owned(),
            })
        );
    }

    #[test]
    fn notice_keeps_texts_in_order() {
        let single = parse(&argv_of(&[
            PROG,
            "notice",
            "manifest",
            "out",
            "//demo:root",
            "a.txt",
        ]));
        assert_eq!(
            single,
            Ok(Invocation::Notice {
                manifest: PathBuf::from("manifest"),
                dst: PathBuf::from("out"),
                root: "//demo:root".to_owned(),
                texts: vec![PathBuf::from("a.txt")],
                runtimes: None,
            })
        );
        let Ok(Invocation::Notice { texts, .. }) = parse(&argv_of(&[
            PROG,
            "notice",
            "manifest",
            "out",
            "//demo:root",
            "a.txt",
            "b.txt",
        ])) else {
            panic!("multi-text notice argv parses");
        };
        assert_eq!(texts, vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")]);
    }

    #[test]
    fn notice_text_lists_expand_in_order() {
        let dir = std::env::temp_dir().join(format!("dx_notice_list_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let list = dir.join("texts.txt");
        std::fs::write(&list, "a.txt\n\nb.txt\n").expect("list");
        let texts = vec![
            PathBuf::from(format!("@{}", list.display())),
            PathBuf::from("c.txt"),
        ];
        assert_eq!(
            expand_text_lists(&texts).expect("expand"),
            vec![
                PathBuf::from("a.txt"),
                PathBuf::from("b.txt"),
                PathBuf::from("c.txt")
            ]
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn unreadable_text_lists_and_inventories_keep_their_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.txt");
        let error = execute(&Invocation::Notice {
            manifest: dir.path().join("inventory.txt"),
            dst: dir.path().join("NOTICE"),
            root: "//demo:root".to_owned(),
            texts: vec![PathBuf::from(format!("@{}", missing.display()))],
            runtimes: None,
        })
        .expect_err("missing list");
        assert!(matches!(error, ToolError::TextList(_)), "{error:?}");
        assert!(error.to_string().starts_with("cannot read "), "{error}");

        let shipped = dir.path().join("shipped.json");
        std::fs::write(&shipped, "{}").expect("shipped");
        let error = execute(&Invocation::Inventory {
            shipped: dir.path().join("absent.json"),
            lock: shipped.clone(),
            licenses: shipped,
            dst: dir.path().join("manifest.txt"),
            texts: dir.path().join("texts"),
            runtimes: None,
        })
        .expect_err("missing shipped file");
        assert!(
            matches!(error, ToolError::Inventory(InventoryError::Io(_))),
            "{error:?}"
        );
        assert!(error.to_string().starts_with("cannot read "), "{error}");
    }

    #[test]
    fn inventory_keeps_all_fields() {
        assert_eq!(
            parse(&argv_of(&[
                PROG,
                "inventory",
                "shipped.json",
                "lock.json",
                "license.toml",
                "manifest.txt",
                "texts"
            ])),
            Ok(Invocation::Inventory {
                shipped: PathBuf::from("shipped.json"),
                lock: PathBuf::from("lock.json"),
                licenses: PathBuf::from("license.toml"),
                dst: PathBuf::from("manifest.txt"),
                texts: PathBuf::from("texts"),
                runtimes: None,
            })
        );
    }

    #[test]
    fn unknown_or_missing_kind_exits_one() {
        assert_eq!(parse(&argv_of(&[PROG])), Err(ParseError::UnknownKind));
        assert_eq!(parse(&argv_of(&[])), Err(ParseError::UnknownKind));
        assert_eq!(
            parse(&argv_of(&[PROG, "sign"])),
            Err(ParseError::UnknownKind)
        );
        assert_eq!(release_tool_run(&argv_of(&[])), 1);
        assert_eq!(release_tool_run(&argv_of(&[PROG])), 1);
        assert_eq!(release_tool_run(&argv_of(&[PROG, "nope", "a", "b"])), 1);
    }

    #[test]
    fn bad_arity_exits_one() {
        for spec in KINDS {
            assert_eq!(
                release_tool_run(&argv_for(spec.name, 0)),
                1,
                "{}",
                spec.name
            );
        }
    }

    #[test]
    fn missing_input_or_unwritable_output_exits_one() {
        for argv in [
            argv_of(&[
                PROG,
                "spdx",
                "/definitely/missing/artifact.bin",
                "/definitely/missing/out.spdx.json",
                "dx",
                "rules_dx",
            ]),
            argv_of(&[
                PROG,
                "provenance",
                "/definitely/missing/artifact.bin",
                "/definitely/missing/out.prov.json",
                "https://invalid.test/builder",
            ]),
            argv_of(&[
                PROG,
                "bcr-source",
                "/definitely/missing/dir/out.source.json",
                "rules_dx",
                "1.2.3",
            ]),
            argv_of(&[
                PROG,
                "notice",
                "/definitely/missing/inventory.txt",
                "/definitely/missing/NOTICE",
                "//demo:root",
                "/definitely/missing/a.txt",
            ]),
            argv_of(&[
                PROG,
                "inventory",
                "/definitely/missing/shipped.json",
                "/definitely/missing/lock.json",
                "/definitely/missing/license.toml",
                "/definitely/missing/manifest.txt",
                "/definitely/missing/texts",
            ]),
        ] {
            assert_eq!(release_tool_run(&argv), 1, "{argv:?}");
        }
    }

    #[test]
    fn spdx_and_bcr_source_round_trip_through_a_tempdir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let artifact = dir.path().join("dx.tar.gz");
        std::fs::write(&artifact, b"payload").expect("artifact");
        let spdx = dir.path().join("dx.spdx.json");
        let source = dir.path().join("dx.source.json");
        let path = |p: &Path| p.to_str().expect("utf-8 path").to_owned();

        let spdx_argv = vec![
            PROG.to_owned(),
            "spdx".to_owned(),
            path(&artifact),
            path(&spdx),
            "dx".to_owned(),
            "rules_dx".to_owned(),
        ];
        assert_eq!(release_tool_run(&spdx_argv), 0);
        let direct = dir.path().join("direct.spdx.json");
        crate::write_spdx(&artifact, &direct, "dx", "rules_dx").expect("direct spdx");
        assert_eq!(
            std::fs::read(&spdx).expect("spdx"),
            std::fs::read(&direct).expect("direct")
        );

        let source_argv = vec![
            PROG.to_owned(),
            "bcr-source".to_owned(),
            path(&source),
            "rules_dx".to_owned(),
            "1.2.3".to_owned(),
        ];
        assert_eq!(release_tool_run(&source_argv), 0);
        assert_eq!(
            std::fs::read_to_string(&source).expect("source"),
            crate::render_bcr_source("rules_dx", "1.2.3").expect("render")
        );
    }
}
