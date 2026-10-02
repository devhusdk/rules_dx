use super::*;

use std::path::PathBuf;

use codegen_shard::proto::{DxCodegenEntry, DxCodegenShard};
use dx_bep::CollectedArtifact;

fn entry(logical_path: &str, import_root: &str, namespace: &str) -> CodegenEntry {
    CodegenEntry {
        logical_path: logical_path.to_owned(),
        import_root: import_root.to_owned(),
        namespace: namespace.to_owned(),
        exec_path: String::new(),
        replaces: String::new(),
    }
}

fn entry_with_exec(
    logical_path: &str,
    import_root: &str,
    namespace: &str,
    exec_path: &str,
) -> CodegenEntry {
    CodegenEntry {
        logical_path: logical_path.to_owned(),
        import_root: import_root.to_owned(),
        namespace: namespace.to_owned(),
        exec_path: exec_path.to_owned(),
        replaces: String::new(),
    }
}

fn entry_with_replaces(
    logical_path: &str,
    import_root: &str,
    namespace: &str,
    exec_path: &str,
    replaces: &str,
) -> CodegenEntry {
    CodegenEntry {
        logical_path: logical_path.to_owned(),
        import_root: import_root.to_owned(),
        namespace: namespace.to_owned(),
        exec_path: exec_path.to_owned(),
        replaces: replaces.to_owned(),
    }
}

fn record(producer: &str, language: &str, entries: Vec<CodegenEntry>) -> CodegenRecord {
    CodegenRecord {
        producer: producer.to_owned(),
        language: language.to_owned(),
        entries,
    }
}

fn record_a() -> CodegenRecord {
    record(
        "//gen:alpha",
        "rust",
        vec![entry("src/alpha.rs", "src", "alpha")],
    )
}

fn record_b() -> CodegenRecord {
    record(
        "//gen:beta",
        "rust",
        vec![entry("src/beta.rs", "src", "beta")],
    )
}

fn shard_bytes(producer: &str, language: &str, entries: Vec<(&str, &str, &str, &str)>) -> Vec<u8> {
    shard_bytes_with_replaces(
        producer,
        language,
        entries
            .into_iter()
            .map(|(logical_path, import_root, namespace, exec_path)| {
                (logical_path, import_root, namespace, exec_path, "")
            })
            .collect(),
    )
}

fn shard_bytes_with_replaces(
    producer: &str,
    language: &str,
    entries: Vec<(&str, &str, &str, &str, &str)>,
) -> Vec<u8> {
    let shard = DxCodegenShard {
        producer: producer.to_owned(),
        language: language.to_owned(),
        entries: entries
            .into_iter()
            .map(
                |(logical_path, import_root, namespace, exec_path, replaces)| DxCodegenEntry {
                    logical_path: logical_path.to_owned(),
                    import_root: import_root.to_owned(),
                    namespace: namespace.to_owned(),
                    read_only: true,
                    exec_path: exec_path.to_owned(),
                    replaces: replaces.to_owned(),
                },
            )
            .collect(),
    };
    codegen_shard::encode_validated(&shard).expect("valid shard")
}

fn output(label: &str, files: Vec<(&str, Vec<u8>)>) -> TargetOutput {
    TargetOutput {
        label: label.to_owned(),
        success: true,
        artifacts: files
            .into_iter()
            .map(|(path, bytes)| CollectedArtifact {
                exec_path: PathBuf::from(path),
                bytes,
            })
            .collect(),
    }
}

#[test]
fn frozen_identities_match_starlark() {
    assert_eq!(OUTPUT_GROUP, "dx_codegen_plans");
    assert_eq!(SHARD_SUFFIX, ".dxcodegen.pb");
    assert_eq!(
        CODEGEN_ASPECT,
        "//generation:codegen.bzl%dx_codegen_plan_aspect"
    );
    assert_eq!(REPOSITORY_TARGET, "//dx:codegen");
    assert_eq!(EXPANSION_KIND_FILTER, ".*codegen_shard rule");
}

#[test]
fn expansion_expression_queries_shard_rdeps() {
    assert_eq!(
        expansion_expression("//generation:result_proto"),
        "kind('.*codegen_shard rule', rdeps(//..., set(\"//generation:result_proto\")))"
    );
    assert_eq!(
        expansion_expression("@repo//pkg:schema"),
        "kind('.*codegen_shard rule', rdeps(//..., set(\"@repo//pkg:schema\")))"
    );
    assert_eq!(
        expansion_expression("//pkg:a\"b\\c"),
        "kind('.*codegen_shard rule', rdeps(//..., set(\"//pkg:a\\\"b\\\\c\")))"
    );
}

#[test]
fn quote_matches_json_for_label_alphabet() {
    for label in [
        "//generation:result_proto",
        "@repo//pkg:schema",
        "//pkg:a\"b\\c",
        "//a:b-c_d.e/f@g",
    ] {
        assert_eq!(
            quote_label(label),
            serde_json::to_string(label).expect("label quotes"),
            "parity for {label:?}"
        );
    }
}

#[test]
fn expand_roots_unions_schema_with_projections() {
    assert_eq!(
        expand_roots("//generation:result_proto", &[]),
        vec!["//generation:result_proto".to_owned()]
    );
    assert_eq!(
        expand_roots(
            "//generation:result_proto",
            &[
                "//generation:codegen_prost_fixture".to_owned(),
                "//generation:codegen_shard_beta".to_owned(),
                "//generation:codegen_prost_fixture".to_owned(),
            ]
        ),
        vec![
            "//generation:codegen_prost_fixture".to_owned(),
            "//generation:codegen_shard_beta".to_owned(),
            "//generation:result_proto".to_owned(),
        ]
    );
    assert_eq!(
        expand_roots(
            "//generation:codegen_shard_beta",
            &["//generation:codegen_shard_beta".to_owned()]
        ),
        vec!["//generation:codegen_shard_beta".to_owned()]
    );
}

#[test]
fn empty_scope_selects_repository() {
    assert_eq!(resolve_scope(&[]), Ok(CodegenScope::Repository));
    assert_eq!(
        scope_targets(&CodegenScope::Repository),
        vec!["//dx:codegen"]
    );
}

#[test]
fn root_plan_composes_baseline_invocation() {
    let plan = dx_roots::repository_plan();
    assert_eq!(targets_for_root_plan(&plan), vec!["//dx:codegen"]);
    assert_eq!(
        build_argv_for_plan(&plan),
        vec![
            "build".to_owned(),
            "//dx:codegen".to_owned(),
            format!("--aspects={CODEGEN_ASPECT}"),
            format!("--output_groups={OUTPUT_GROUP}"),
        ]
    );
}

#[test]
fn root_plan_passes_aggregate_roots_through() {
    let plan = dx_roots::RepositoryRootPlan::monolithic_aggregate("//dx:codegen_roots");
    assert_eq!(
        targets_for_root_plan(&plan),
        vec!["//dx:codegen_roots".to_owned()]
    );
    assert_eq!(
        build_argv_for_plan(&plan)[1],
        "//dx:codegen_roots".to_owned()
    );
}

#[test]
fn exact_labels_pass_through() {
    for label in ["//gen:consumer", "@rules_dx//generation:result_proto_rs"] {
        let scope = resolve_scope(&[label.to_owned()]).expect("exact label");
        assert_eq!(scope, CodegenScope::Exact(label.to_owned()));
        assert_eq!(scope_targets(&scope), vec![label.to_owned()]);
    }
}

#[test]
fn scope_rejects_non_exact_inputs() {
    assert_eq!(
        resolve_scope(&["//a:x".to_owned(), "//b:y".to_owned()]),
        Err(ScopeError::MultipleTargets { count: 2 })
    );
    for pattern in ["//...", "//gen/...", "//gen:*", "@repo//pkg:all?"] {
        assert_eq!(
            resolve_scope(&[pattern.to_owned()]),
            Err(ScopeError::TargetPattern {
                value: pattern.to_owned(),
            }),
            "pattern {pattern:?} must fail"
        );
    }
    for other in ["gen/alpha.rs", "gen", "rust", "--profile=fast", ":relative"] {
        assert_eq!(
            resolve_scope(&[other.to_owned()]),
            Err(ScopeError::NotTargetLabel {
                value: other.to_owned(),
            }),
            "non-label {other:?} must fail"
        );
    }
}

#[test]
fn scope_errors_display() {
    assert!(ScopeError::MultipleTargets { count: 2 }
        .to_string()
        .contains("at most one target"));
}

#[test]
fn shard_suffix_matches_on_path_bytes() {
    assert!(is_shard_artifact(Path::new("/out/a.dxcodegen.pb")));
    assert!(!is_shard_artifact(Path::new("/out/a.pb")));
    assert!(!is_shard_artifact(Path::new("/out/a.dxcodegen.pb.txt")));
}

#[test]
fn exec_suffix_matches_on_component_boundaries() {
    assert!(exec_matches(
        Path::new("/out/result_proto.lib.rs"),
        "result_proto.lib.rs"
    ));
    assert!(exec_matches(
        Path::new("/bazel-out/k8-fastbuild/bin/gen/result_proto.lib.rs"),
        "result_proto.lib.rs"
    ));
    assert!(!exec_matches(
        Path::new("/out/other.lib.rs"),
        "result_proto.lib.rs"
    ));
    assert!(!exec_matches(
        Path::new("/out/xresult_proto.lib.rs"),
        "result_proto.lib.rs"
    ));
    assert!(!exec_matches(Path::new("/out/a.rs"), ""));
}

#[test]
fn collect_shards_accepts_logical_only_without_artifacts() {
    let outputs = vec![output(
        "//gen:beta",
        vec![(
            "/out/beta.dxcodegen.pb",
            shard_bytes(
                "//gen:beta",
                "rust",
                vec![("gen/beta.rs", "gen", "beta", "")],
            ),
        )],
    )];
    let records = collect_shards(&outputs).expect("collect");
    assert_eq!(
        records,
        vec![record(
            "//gen:beta",
            "rust",
            vec![entry("gen/beta.rs", "gen", "beta")]
        )]
    );
}

#[test]
fn collect_shards_binds_exec_suffix_to_one_artifact() {
    let outputs = vec![output(
        "//gen:beta",
        vec![
            (
                "/out/beta.dxcodegen.pb",
                shard_bytes(
                    "//gen:beta",
                    "rust",
                    vec![("gen/beta.rs", "gen", "beta", "beta.lib.rs")],
                ),
            ),
            ("/bazel-out/k8-fastbuild/bin/gen/beta.lib.rs", vec![1, 2, 3]),
        ],
    )];
    let records = collect_shards(&outputs).expect("collect");
    assert_eq!(
        records,
        vec![record(
            "//gen:beta",
            "rust",
            vec![entry_with_exec("gen/beta.rs", "gen", "beta", "beta.lib.rs")]
        )]
    );
}

#[test]
fn collect_shards_rejects_missing_duplicate_and_unreported() {
    let missing = vec![output(
        "//gen:beta",
        vec![(
            "/out/beta.dxcodegen.pb",
            shard_bytes(
                "//gen:beta",
                "rust",
                vec![("gen/beta.rs", "gen", "beta", "beta.lib.rs")],
            ),
        )],
    )];
    assert_eq!(
        collect_shards(&missing),
        Err(CollectError::MissingArtifact {
            producer: "//gen:beta".to_owned(),
            logical_path: "gen/beta.rs".to_owned(),
            exec_path: "beta.lib.rs".to_owned(),
        })
    );
    let ambiguous = vec![output(
        "//gen:beta",
        vec![
            (
                "/out/beta.dxcodegen.pb",
                shard_bytes(
                    "//gen:beta",
                    "rust",
                    vec![("gen/beta.rs", "gen", "beta", "beta.lib.rs")],
                ),
            ),
            ("/out/a/beta.lib.rs", vec![1]),
            ("/out/b/beta.lib.rs", vec![2]),
        ],
    )];
    assert!(matches!(
        collect_shards(&ambiguous),
        Err(CollectError::DuplicateArtifact { .. })
    ));
    let unreported = vec![output(
        "//gen:beta",
        vec![
            (
                "/out/beta.dxcodegen.pb",
                shard_bytes(
                    "//gen:beta",
                    "rust",
                    vec![("gen/beta.rs", "gen", "beta", "")],
                ),
            ),
            ("/out/beta.lib.rs", vec![1, 2, 3]),
        ],
    )];
    assert_eq!(
        collect_shards(&unreported),
        Err(CollectError::UnreportedArtifact {
            path: "/out/beta.lib.rs".to_owned(),
        })
    );
    let duplicate = vec![output(
        "//gen:beta",
        vec![
            (
                "/out/beta.dxcodegen.pb",
                shard_bytes(
                    "//gen:beta",
                    "rust",
                    vec![
                        ("gen/a.rs", "gen", "a", "shared.lib.rs"),
                        ("gen/b.rs", "gen", "b", "shared.lib.rs"),
                    ],
                ),
            ),
            ("/out/shared.lib.rs", vec![1]),
        ],
    )];
    assert!(matches!(
        collect_shards(&duplicate),
        Err(CollectError::DuplicateArtifact { .. })
    ));
}

#[test]
fn collect_shards_rejects_invalid_shards_with_path() {
    let outputs = vec![output(
        "//gen:evil",
        vec![("/out/evil.dxcodegen.pb", vec![0xff, 0x00, 0x01])],
    )];
    let error = collect_shards(&outputs).expect_err("invalid shard");
    assert!(matches!(error, CollectError::Shard { .. }));
    assert!(error.to_string().contains("invalid shard"));
}

#[test]
fn merge_groups_by_owner_and_sorts_entries() {
    let merged = merge_records(&[
        record(
            "//gen:b",
            "rust",
            vec![entry("z.rs", "s", ""), entry("a.rs", "s", "")],
        ),
        record_a(),
        record_a(),
        record("//gen:a", "python", vec![entry("m.py", "s", "")]),
    ]);
    let summary: Vec<(&str, &str, Vec<&str>)> = merged
        .iter()
        .map(|record| {
            (
                record.producer.as_str(),
                record.language.as_str(),
                record
                    .entries
                    .iter()
                    .map(|entry| entry.logical_path.as_str())
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("//gen:a", "python", vec!["m.py"]),
            ("//gen:alpha", "rust", vec!["src/alpha.rs"]),
            ("//gen:b", "rust", vec!["a.rs", "z.rs"]),
        ]
    );
}

#[test]
fn fingerprint_matches_starlark_rendering() {
    assert_eq!(
        fingerprint(&[record_b(), record_a(), record_a()]).expect("fingerprint"),
        "[{\"entries\":[{\"exec_path\":\"\",\"import_root\":\"src\",\"logical_path\":\"src/alpha.rs\",\"namespace\":\"alpha\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\",\"producer\":\"//gen:alpha\"},\
         {\"entries\":[{\"exec_path\":\"\",\"import_root\":\"src\",\"logical_path\":\"src/beta.rs\",\"namespace\":\"beta\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\",\"producer\":\"//gen:beta\"}]"
    );
    assert_eq!(
        fingerprint(&[record(
            "//gen:a",
            "rust",
            vec![entry_with_exec("a", "b", "", "out/a.rs")]
        )]).expect("fingerprint"),
        "[{\"entries\":[{\"exec_path\":\"out/a.rs\",\"import_root\":\"b\",\"logical_path\":\"a\",\"namespace\":\"\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\",\"producer\":\"//gen:a\"}]"
    );
    assert_eq!(
        fingerprint(&[record(
            "//gen:a",
            "rust",
            vec![entry_with_replaces("a", "b", "", "out/a.rs", "a")]
        )]).expect("fingerprint"),
        "[{\"entries\":[{\"exec_path\":\"out/a.rs\",\"import_root\":\"b\",\"logical_path\":\"a\",\"namespace\":\"\",\"read_only\":true,\"replaces\":\"a\"}],\"language\":\"rust\",\"producer\":\"//gen:a\"}]"
    );
}

#[test]
fn fingerprint_matches_chain_fixture() {
    let records = vec![
        record(
            "//generation:codegen_shard_beta",
            "rust",
            vec![entry("gen/beta.rs", "gen", "beta")],
        ),
        record(
            "//generation:codegen_shard_alpha",
            "rust",
            vec![entry("gen/alpha.rs", "gen", "alpha")],
        ),
    ];
    assert_eq!(
        fingerprint(&records).expect("fingerprint"),
        "[{\"entries\":[{\"exec_path\":\"\",\"import_root\":\"gen\",\"logical_path\":\"gen/alpha.rs\",\"namespace\":\"alpha\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\",\"producer\":\"//generation:codegen_shard_alpha\"},\
         {\"entries\":[{\"exec_path\":\"\",\"import_root\":\"gen\",\"logical_path\":\"gen/beta.rs\",\"namespace\":\"beta\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\",\"producer\":\"//generation:codegen_shard_beta\"}]"
    );
}

#[test]
fn fingerprint_matches_prost_fixture() {
    let records = vec![record(
        "//generation:codegen_prost_fixture",
        "rust",
        vec![entry_with_exec(
            "gen/prost_result.rs",
            "gen",
            "result",
            "result_proto.lib.rs",
        )],
    )];
    assert_eq!(
        fingerprint(&records).expect("fingerprint"),
        "[{\"entries\":[{\"exec_path\":\"result_proto.lib.rs\",\"import_root\":\"gen\",\"logical_path\":\"gen/prost_result.rs\",\"namespace\":\"result\",\"read_only\":true,\"replaces\":\"\"}],\"language\":\"rust\",\"producer\":\"//generation:codegen_prost_fixture\"}]"
    );
}

#[test]
fn fingerprint_escapes_quotes_newlines_and_controls() {
    let records = vec![CodegenRecord {
        producer: "//gen:\"a\"\n".to_owned(),
        language: "rust".to_owned(),
        entries: vec![CodegenEntry {
            logical_path: "a\"b\nc\u{1}d".to_owned(),
            import_root: "src".to_owned(),
            namespace: "ns\\q".to_owned(),
            exec_path: "out/a.rs".to_owned(),
            replaces: String::new(),
        }],
    }];
    let rendered = fingerprint(&records).expect("fingerprint");
    assert!(rendered.contains("\\\""), "{rendered}");
    assert!(rendered.contains("\\n"), "{rendered}");
    assert!(rendered.contains("\\\\"), "{rendered}");
    assert!(rendered.contains("\\u0001"), "{rendered}");
    let parsed: serde_json::Value =
        serde_json::from_str(&rendered).expect("fingerprint is valid JSON");
    assert_eq!(
        parsed[0]["entries"][0]["logical_path"],
        serde_json::Value::String("a\"b\nc\u{1}d".to_owned())
    );
    assert_eq!(
        serde_json::to_string(&parsed).expect("reserialize"),
        rendered
    );
}

#[test]
fn conflict_accepts_disjoint_and_identical_records() {
    assert_eq!(conflict_error(&[record_a(), record_b()]), "");
    assert_eq!(conflict_error(&[record_a(), record_a()]), "");
    assert_eq!(conflict_error(&[]), "");
}

#[test]
fn conflict_lists_every_claimant() {
    assert_eq!(
        conflict_error(&[
            record_a(),
            record(
                "//gen:evil",
                "rust",
                vec![entry("src/alpha.rs", "src", "alpha")]
            ),
        ]),
        "codegen path conflict: logical path 'src/alpha.rs' claimed by //gen:alpha, //gen:evil"
    );
    assert_eq!(
        conflict_error(&[
            record_a(),
            record(
                "//gen:alpha",
                "rust",
                vec![entry("src/alpha.rs", "other", "alpha")]
            ),
        ]),
        "codegen path conflict: logical path 'src/alpha.rs' claimed by //gen:alpha, //gen:alpha"
    );
    assert_eq!(
        conflict_error(&[
            record_a(),
            record(
                "//gen:alpha",
                "rust",
                vec![entry_with_exec("src/alpha.rs", "src", "alpha", "out/a.rs")]
            ),
        ]),
        "codegen path conflict: logical path 'src/alpha.rs' claimed by //gen:alpha, //gen:alpha"
    );
}

#[test]
fn replaces_binds_into_merge_conflict_and_fingerprint() {
    let contracted = record(
        "//gen:alpha",
        "rust",
        vec![entry_with_replaces(
            "src/alpha.rs",
            "src",
            "alpha",
            "out/a.rs",
            "src/alpha.rs",
        )],
    );
    assert_eq!(
        conflict_error(&[contracted.clone(), contracted.clone()]),
        ""
    );
    assert_eq!(
        merge_records(&[contracted.clone(), contracted.clone()]).len(),
        1
    );
    assert_eq!(
        conflict_error(&[
            record(
                "//gen:alpha",
                "rust",
                vec![entry_with_exec("src/alpha.rs", "src", "alpha", "out/a.rs")]
            ),
            contracted,
        ]),
        "codegen path conflict: logical path 'src/alpha.rs' claimed by //gen:alpha, //gen:alpha"
    );
    let plain = fingerprint(&[record(
        "//gen:a",
        "rust",
        vec![entry_with_exec("a", "b", "", "out/a.rs")],
    )])
    .expect("fingerprint");
    let replaced = fingerprint(&[record(
        "//gen:a",
        "rust",
        vec![entry_with_replaces("a", "b", "", "out/a.rs", "a")],
    )])
    .expect("fingerprint");
    assert_ne!(plain, replaced);
    assert!(replaced.contains("\"replaces\":\"a\""), "{replaced}");
}

#[test]
fn collect_shards_round_trips_replacement_contract() {
    let outputs = vec![output(
        "//gen:beta",
        vec![
            (
                "/out/beta.dxcodegen.pb",
                shard_bytes_with_replaces(
                    "//gen:beta",
                    "rust",
                    vec![("gen/beta.rs", "gen", "beta", "beta.lib.rs", "gen/beta.rs")],
                ),
            ),
            ("/bazel-out/k8-fastbuild/bin/gen/beta.lib.rs", vec![1, 2, 3]),
        ],
    )];
    let records = collect_shards(&outputs).expect("collect");
    assert_eq!(
        records,
        vec![record(
            "//gen:beta",
            "rust",
            vec![entry_with_replaces(
                "gen/beta.rs",
                "gen",
                "beta",
                "beta.lib.rs",
                "gen/beta.rs",
            )]
        )]
    );
    let projection = plan_projection(&records, &outputs).expect("projection");
    assert_eq!(projection.len(), 1);
    assert_eq!(projection[0].logical_path, "gen/beta.rs");
    assert_eq!(projection[0].replaces, "gen/beta.rs");
}

#[test]
fn collect_shards_rejects_bad_replacement_contract() {
    let bad = DxCodegenShard {
        producer: "//gen:beta".to_owned(),
        language: "rust".to_owned(),
        entries: vec![DxCodegenEntry {
            logical_path: "gen/beta.rs".to_owned(),
            import_root: "gen".to_owned(),
            namespace: "beta".to_owned(),
            read_only: true,
            exec_path: String::new(),
            replaces: "gen/beta.rs".to_owned(),
        }],
    };
    assert!(
        codegen_shard::encode_validated(&bad).is_err(),
        "replaces without exec must fail shard validation"
    );
}

#[test]
fn collect_plan_merges_hashes_and_sorts_deterministically() {
    let beta = shard_bytes(
        "//gen:beta",
        "rust",
        vec![("src/beta.rs", "src", "beta", "gen/beta.rs")],
    );
    let alpha = shard_bytes(
        "//gen:alpha",
        "rust",
        vec![("src/alpha.rs", "src", "alpha", "gen/alpha.rs")],
    );
    let forward = vec![
        output(
            "//gen:alpha",
            vec![
                ("/out/a.dxcodegen.pb", alpha.clone()),
                ("/bazel-out/k8-fastbuild/bin/gen/alpha.rs", vec![1]),
            ],
        ),
        output(
            "//gen:beta",
            vec![
                ("/out/b.dxcodegen.pb", beta.clone()),
                ("/bazel-out/k8-fastbuild/bin/gen/beta.rs", vec![2]),
            ],
        ),
    ];
    let reverse = vec![
        output(
            "//gen:beta",
            vec![
                ("/out/b.dxcodegen.pb", beta),
                ("/bazel-out/k8-fastbuild/bin/gen/beta.rs", vec![2]),
            ],
        ),
        output(
            "//gen:alpha",
            vec![
                ("/out/a.dxcodegen.pb", alpha),
                ("/bazel-out/k8-fastbuild/bin/gen/alpha.rs", vec![1]),
            ],
        ),
    ];
    let first = collect_plan(&forward).expect("plan");
    let second = collect_plan(&reverse).expect("plan");
    assert_eq!(first, second);
    assert_eq!(first.records.len(), 2);
    assert_eq!(first.records[0].producer, "//gen:alpha");
    assert_eq!(
        first.fingerprint,
        fingerprint(&first.records).expect("fingerprint")
    );
    assert_eq!(first.digest, plan_digest(&first.fingerprint));
    let summary: Vec<(&str, &str)> = first
        .projection
        .iter()
        .map(|leaf| (leaf.logical_path.as_str(), leaf.artifact.as_str()))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("src/alpha.rs", "/bazel-out/k8-fastbuild/bin/gen/alpha.rs"),
            ("src/beta.rs", "/bazel-out/k8-fastbuild/bin/gen/beta.rs"),
        ]
    );
}

#[test]
fn collect_plan_rejects_conflicts() {
    let outputs = vec![
        output(
            "//gen:alpha",
            vec![(
                "/out/a.dxcodegen.pb",
                shard_bytes("//gen:alpha", "rust", vec![("src/same.rs", "src", "a", "")]),
            )],
        ),
        output(
            "//gen:evil",
            vec![(
                "/out/e.dxcodegen.pb",
                shard_bytes("//gen:evil", "rust", vec![("src/same.rs", "src", "e", "")]),
            )],
        ),
    ];
    assert_eq!(
        collect_plan(&outputs),
        Err(CollectError::Conflict(
            "codegen path conflict: logical path 'src/same.rs' claimed by //gen:alpha, //gen:evil"
                .to_owned()
        ))
    );
}

#[test]
fn empty_outputs_select_an_empty_plan() {
    let plan = collect_plan(&[]).expect("empty plan");
    assert!(plan.records.is_empty());
    assert_eq!(plan.fingerprint, "[]");
    assert_eq!(plan.digest, dx_digest::blake3(b"[]"));
}

#[test]
fn projection_resolves_backed_entries_and_sorts() {
    let outputs = vec![
        output(
            "//gen:beta",
            vec![
                (
                    "/out/beta.dxcodegen.pb",
                    shard_bytes(
                        "//gen:beta",
                        "rust",
                        vec![
                            ("gen/z.rs", "gen", "z", "z.lib.rs"),
                            ("gen/a.rs", "gen", "a", "a.lib.rs"),
                            ("gen/logical.rs", "gen", "logical", ""),
                        ],
                    ),
                ),
                ("/bazel-out/k8-fastbuild/bin/gen/z.lib.rs", vec![1]),
                ("/bazel-out/k8-fastbuild/bin/gen/a.lib.rs", vec![2]),
            ],
        ),
        output(
            "//gen:alpha",
            vec![(
                "/out/alpha.dxcodegen.pb",
                shard_bytes("//gen:alpha", "rust", vec![("gen/m.rs", "gen", "m", "")]),
            )],
        ),
    ];
    let plan = collect_plan(&outputs).expect("plan");
    let projection = &plan.projection;
    let summary: Vec<(&str, &str)> = projection
        .iter()
        .map(|entry| (entry.logical_path.as_str(), entry.artifact.as_str()))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("gen/a.rs", "/bazel-out/k8-fastbuild/bin/gen/a.lib.rs"),
            ("gen/z.rs", "/bazel-out/k8-fastbuild/bin/gen/z.lib.rs"),
        ]
    );
    assert_eq!(projection[0].import_root, "gen");
    assert_eq!(projection[0].namespace, "a");
}

#[test]
fn projection_rejects_bad_indexes() {
    let missing_records = vec![record(
        "//gen:beta",
        "rust",
        vec![entry_with_exec("gen/beta.rs", "gen", "beta", "beta.lib.rs")],
    )];
    assert!(matches!(
        plan_projection(&missing_records, &[]),
        Err(CollectError::MissingArtifact { .. })
    ));
    let unreported_outputs = vec![output(
        "//gen:beta",
        vec![
            (
                "/out/beta.dxcodegen.pb",
                shard_bytes(
                    "//gen:beta",
                    "rust",
                    vec![("gen/beta.rs", "gen", "beta", "")],
                ),
            ),
            ("/out/beta.lib.rs", vec![1, 2, 3]),
        ],
    )];
    let logical_records = vec![record(
        "//gen:beta",
        "rust",
        vec![entry("gen/beta.rs", "gen", "beta")],
    )];
    assert!(matches!(
        plan_projection(&logical_records, &unreported_outputs),
        Err(CollectError::UnreportedArtifact { .. })
    ));
}
