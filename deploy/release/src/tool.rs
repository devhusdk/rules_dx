//! `release_tool <kind> <args...>`: one dispatcher for the release generators
//! that genrules in sbom.bzl, notice.bzl and bcr.bzl call.
//!
//! Kinds are argv-only because they run inside genrules, not `bazel run`.
//! The `bcr` and `sign` deploy kinds join this table once #2453 settles the
//! DX_DEPLOY_CONFIG protocol (follow-up of #2836).

use std::path::{Path, PathBuf};

const PROG: &str = "release_tool";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Spdx,
    Provenance,
    BcrSource,
    Notice,
}

/// How many arguments a kind takes after its name.
#[derive(Clone, Copy)]
enum Arity {
    Exactly(usize),
    AtLeast(usize),
}

impl Arity {
    fn accepts(self, count: usize) -> bool {
        match self {
            Self::Exactly(n) => count == n,
            Self::AtLeast(n) => count >= n,
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
        usage: "<artifact> <out> <package> <supplier>",
        arity: Arity::Exactly(4),
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
        usage: "<manifest> <out> <root> <text> [<text> ...]",
        arity: Arity::AtLeast(4),
    },
];

#[derive(Debug, PartialEq, Eq)]
enum Invocation {
    Spdx {
        src: PathBuf,
        dst: PathBuf,
        package: String,
        supplier: String,
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
    },
}

impl Invocation {
    fn dst(&self) -> &Path {
        match self {
            Self::Spdx { dst, .. }
            | Self::Provenance { dst, .. }
            | Self::BcrSource { dst, .. }
            | Self::Notice { dst, .. } => dst,
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
    let args = &argv[2..];
    if !spec.arity.accepts(args.len()) {
        return Err(ParseError::Arity {
            name: spec.name,
            usage: spec.usage,
        });
    }
    Ok(match spec.kind {
        Kind::Spdx => Invocation::Spdx {
            src: PathBuf::from(&args[0]),
            dst: PathBuf::from(&args[1]),
            package: args[2].clone(),
            supplier: args[3].clone(),
        },
        Kind::Provenance => Invocation::Provenance {
            src: PathBuf::from(&args[0]),
            dst: PathBuf::from(&args[1]),
            builder: args[2].clone(),
        },
        Kind::BcrSource => Invocation::BcrSource {
            dst: PathBuf::from(&args[0]),
            module: args[1].clone(),
            version: args[2].clone(),
        },
        Kind::Notice => Invocation::Notice {
            manifest: PathBuf::from(&args[0]),
            dst: PathBuf::from(&args[1]),
            root: args[2].clone(),
            texts: args[3..].iter().map(PathBuf::from).collect(),
        },
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
    }
}

fn kind_name(invocation: &Invocation) -> &'static str {
    match invocation {
        Invocation::Spdx { .. } => "spdx",
        Invocation::Provenance { .. } => "provenance",
        Invocation::BcrSource { .. } => "bcr-source",
        Invocation::Notice { .. } => "notice",
    }
}

fn execute(invocation: &Invocation) -> std::io::Result<()> {
    match invocation {
        Invocation::Spdx {
            src,
            dst,
            package,
            supplier,
        } => crate::write_spdx(src, dst, package, supplier),
        Invocation::Provenance { src, dst, builder } => crate::write_provenance(src, dst, builder),
        Invocation::BcrSource {
            dst,
            module,
            version,
        } => crate::write_bcr_source(dst, module, version),
        Invocation::Notice {
            manifest,
            dst,
            root,
            texts,
        } => crate::write_notice(manifest, dst, root, texts),
    }
}

/// Runs `release_tool <kind> <args...>` and returns the process exit code.
/// `argv[0]` is the program name, as in `std::env::args()`.
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
            ("spdx", 4, false),
            ("provenance", 3, false),
            ("bcr-source", 3, false),
            ("notice", 4, true),
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
            })
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
            crate::render_bcr_source("rules_dx", "1.2.3")
        );
    }
}
