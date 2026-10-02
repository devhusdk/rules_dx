use super::*;

use std::path::PathBuf;

use dx_bep::CollectedArtifact;
use env_shard::proto::{DxEnvEntry, DxEnvShard};

fn entry(key: &str, value: &str) -> EnvEntry {
    EnvEntry {
        key: key.to_owned(),
        value: value.to_owned(),
        exec_path: String::new(),
    }
}

fn entry_with_exec(key: &str, value: &str, exec_path: &str) -> EnvEntry {
    EnvEntry {
        key: key.to_owned(),
        value: value.to_owned(),
        exec_path: exec_path.to_owned(),
    }
}

fn record(producer: &str, integration: &str, entries: Vec<EnvEntry>) -> EnvRecord {
    EnvRecord {
        producer: producer.to_owned(),
        integration: integration.to_owned(),
        entries,
    }
}

fn record_a() -> EnvRecord {
    record(
        "//env:alpha",
        "rust",
        vec![entry("runtime", "stable-x86_64")],
    )
}

fn record_b() -> EnvRecord {
    record("//env:beta", "rust", vec![entry("abi", "gnu")])
}

fn shard_bytes(producer: &str, integration: &str, entries: Vec<(&str, &str, &str)>) -> Vec<u8> {
    let shard = DxEnvShard {
        producer: producer.to_owned(),
        integration: integration.to_owned(),
        entries: entries
            .into_iter()
            .map(|(key, value, exec_path)| DxEnvEntry {
                key: key.to_owned(),
                value: value.to_owned(),
                exec_path: exec_path.to_owned(),
            })
            .collect(),
    };
    env_shard::encode_validated(&shard).expect("valid shard")
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
    assert_eq!(OUTPUT_GROUP, "dx_env_plans");
    assert_eq!(SHARD_SUFFIX, ".dxenv.pb");
    assert_eq!(ENV_ASPECT, "//env:plan.bzl%dx_env_plan_aspect");
    assert_eq!(REPOSITORY_TARGET, "//dx:env");
}

#[test]
fn empty_scope_selects_repository() {
    assert_eq!(resolve_scope(&[]), Ok(EnvScope::Repository));
    assert_eq!(scope_targets(&EnvScope::Repository), vec!["//dx:env"]);
}

#[test]
fn root_plan_composes_baseline_invocation() {
    let plan = dx_roots::repository_plan();
    assert_eq!(targets_for_root_plan(&plan), vec!["//dx:env"]);
    assert_eq!(
        build_argv_for_plan(&plan),
        vec![
            "build".to_owned(),
            "//dx:env".to_owned(),
            format!("--aspects={ENV_ASPECT}"),
            format!("--output_groups={OUTPUT_GROUP}"),
        ]
    );
}

#[test]
fn root_plan_passes_aggregate_roots_through() {
    let plan = dx_roots::RepositoryRootPlan::monolithic_aggregate("//dx:env_roots");
    assert_eq!(
        targets_for_root_plan(&plan),
        vec!["//dx:env_roots".to_owned()]
    );
    assert_eq!(build_argv_for_plan(&plan)[1], "//dx:env_roots".to_owned());
}

#[test]
fn exact_labels_pass_through() {
    for label in ["//app:server", "@rules_dx//env:plan_proto"] {
        let scope = resolve_scope(&[label.to_owned()]).expect("exact label");
        assert_eq!(scope, EnvScope::Exact(label.to_owned()));
        assert_eq!(scope_targets(&scope), vec![label.to_owned()]);
    }
}

#[test]
fn scope_rejects_non_exact_inputs() {
    assert_eq!(
        resolve_scope(&["//a:x".to_owned(), "//b:y".to_owned()]),
        Err(ScopeError::MultipleTargets { count: 2 })
    );
    for pattern in ["//...", "//env/...", "//env:*", "@repo//pkg:all?"] {
        assert_eq!(
            resolve_scope(&[pattern.to_owned()]),
            Err(ScopeError::TargetPattern {
                value: pattern.to_owned(),
            }),
            "pattern {pattern:?} must fail"
        );
    }
    for other in ["env/alpha", "env", "rust", "--profile=fast", ":relative"] {
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
    assert!(is_shard_artifact(Path::new("/out/a.dxenv.pb")));
    assert!(!is_shard_artifact(Path::new("/out/a.pb")));
    assert!(!is_shard_artifact(Path::new("/out/a.dxenv.pb.txt")));
}

#[test]
fn exec_suffix_matches_on_component_boundaries() {
    assert!(exec_matches(Path::new("/out/rustc"), "rustc"));
    assert!(exec_matches(
        Path::new("/bazel-out/k8-fastbuild/bin/env/rustc"),
        "env/rustc"
    ));
    assert!(!exec_matches(Path::new("/out/other"), "rustc"));
    assert!(!exec_matches(Path::new("/out/xrustc"), "rustc"));
    assert!(!exec_matches(Path::new("/out/rustc"), ""));
}

#[test]
fn collect_shards_accepts_logical_only_without_artifacts() {
    let outputs = vec![output(
        "//env:beta",
        vec![(
            "/out/beta.dxenv.pb",
            shard_bytes("//env:beta", "rust", vec![("abi", "gnu", "")]),
        )],
    )];
    let records = collect_shards(&outputs).expect("collect");
    assert_eq!(
        records,
        vec![record("//env:beta", "rust", vec![entry("abi", "gnu")])]
    );
}

#[test]
fn collect_shards_preserves_exec_paths() {
    let outputs = vec![output(
        "//env:rust",
        vec![
            (
                "/out/rust.dxenv.pb",
                shard_bytes(
                    "//env:rust",
                    "rust",
                    vec![("runtime", "stable-x86_64", "env/env_shard/src/lib.rs")],
                ),
            ),
            (
                "/bazel-out/k8-fastbuild/bin/env/env_shard/src/lib.rs",
                vec![1, 2, 3],
            ),
        ],
    )];
    let records = collect_shards(&outputs).expect("collect");
    assert_eq!(
        records,
        vec![record(
            "//env:rust",
            "rust",
            vec![entry_with_exec(
                "runtime",
                "stable-x86_64",
                "env/env_shard/src/lib.rs"
            )]
        )]
    );
}

#[test]
fn collect_shards_binds_exec_suffix_to_one_artifact() {
    let outputs = vec![output(
        "//env:rust",
        vec![
            (
                "/out/rust.dxenv.pb",
                shard_bytes(
                    "//env:rust",
                    "rust",
                    vec![("toolchain", "1.89.0", "toolchain/rustc")],
                ),
            ),
            ("/bazel-out/k8-fastbuild/bin/env/toolchain/rustc", vec![7]),
        ],
    )];
    let records = collect_shards(&outputs).expect("collect");
    assert_eq!(
        records,
        vec![record(
            "//env:rust",
            "rust",
            vec![entry_with_exec("toolchain", "1.89.0", "toolchain/rustc")]
        )]
    );
}

#[test]
fn collect_shards_rejects_missing_ambiguous_and_unreported() {
    let missing = vec![output(
        "//env:rust",
        vec![(
            "/out/rust.dxenv.pb",
            shard_bytes(
                "//env:rust",
                "rust",
                vec![("toolchain", "1.89.0", "toolchain/rustc")],
            ),
        )],
    )];
    assert_eq!(
        collect_shards(&missing),
        Err(CollectError::MissingArtifact {
            producer: "//env:rust".to_owned(),
            key: "toolchain".to_owned(),
            exec_path: "toolchain/rustc".to_owned(),
        })
    );
    let ambiguous = vec![output(
        "//env:rust",
        vec![
            (
                "/out/rust.dxenv.pb",
                shard_bytes(
                    "//env:rust",
                    "rust",
                    vec![("toolchain", "1.89.0", "toolchain/rustc")],
                ),
            ),
            ("/out/a/toolchain/rustc", vec![1]),
            ("/out/b/toolchain/rustc", vec![2]),
        ],
    )];
    assert!(matches!(
        collect_shards(&ambiguous),
        Err(CollectError::AmbiguousArtifact { .. })
    ));
    let unreported = vec![output(
        "//env:beta",
        vec![
            (
                "/out/beta.dxenv.pb",
                shard_bytes("//env:beta", "rust", vec![("abi", "gnu", "")]),
            ),
            ("/out/backing.lib.rs", vec![1, 2, 3]),
        ],
    )];
    assert_eq!(
        collect_shards(&unreported),
        Err(CollectError::UnreportedArtifact {
            path: "/out/backing.lib.rs".to_owned(),
        })
    );
}

#[test]
fn collect_shards_allows_shared_backing_artifact() {
    let outputs = vec![output(
        "//env:rust",
        vec![
            (
                "/out/rust.dxenv.pb",
                shard_bytes(
                    "//env:rust",
                    "rust",
                    vec![
                        ("toolchain", "1.89.0", "shared/rustc"),
                        ("runtime", "stable", "shared/rustc"),
                    ],
                ),
            ),
            ("/out/shared/rustc", vec![1]),
        ],
    )];
    let records = collect_shards(&outputs).expect("shared backing");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].entries.len(), 2);
}

#[test]
fn collect_shards_rejects_invalid_shards_with_path() {
    let outputs = vec![output(
        "//env:evil",
        vec![("/out/evil.dxenv.pb", vec![0xff, 0x00, 0x01])],
    )];
    let error = collect_shards(&outputs).expect_err("invalid shard");
    assert!(matches!(error, CollectError::Shard { .. }));
    assert!(error.to_string().contains("invalid shard"));
}

#[test]
fn merge_groups_by_owner_and_sorts_entries() {
    let merged = merge_records(&[
        record(
            "//env:b",
            "rust",
            vec![entry("runtime", "stable"), entry("abi", "gnu")],
        ),
        record_a(),
        record_a(),
        record("//env:a", "rust", vec![entry("abi", "gnu")]),
    ]);
    let summary: Vec<(&str, &str, Vec<&str>)> = merged
        .iter()
        .map(|record| {
            (
                record.producer.as_str(),
                record.integration.as_str(),
                record
                    .entries
                    .iter()
                    .map(|entry| entry.key.as_str())
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("//env:a", "rust", vec!["abi"]),
            ("//env:alpha", "rust", vec!["runtime"]),
            ("//env:b", "rust", vec!["abi", "runtime"]),
        ]
    );
}

#[test]
fn fingerprint_matches_starlark_rendering() {
    assert_eq!(
        fingerprint(&[record_b(), record_a(), record_a()]).expect("fingerprint"),
        "[{\"entries\":[{\"exec_path\":\"\",\"key\":\"runtime\",\"value\":\"stable-x86_64\"}],\"integration\":\"rust\",\"producer\":\"//env:alpha\"},\
         {\"entries\":[{\"exec_path\":\"\",\"key\":\"abi\",\"value\":\"gnu\"}],\"integration\":\"rust\",\"producer\":\"//env:beta\"}]"
    );
    assert_eq!(
        fingerprint(&[record(
            "//env:a",
            "rust",
            vec![entry_with_exec("runtime", "stable", "out/rustc")]
        )]).expect("fingerprint"),
        "[{\"entries\":[{\"exec_path\":\"out/rustc\",\"key\":\"runtime\",\"value\":\"stable\"}],\"integration\":\"rust\",\"producer\":\"//env:a\"}]"
    );
}

#[test]
fn fingerprint_matches_chain_fixture() {
    let records = vec![
        record("//env:env_shard_beta", "rust", vec![entry("abi", "gnu")]),
        record(
            "//env:env_shard_alpha",
            "rust",
            vec![entry("runtime", "stable-x86_64")],
        ),
    ];
    assert_eq!(
        fingerprint(&records).expect("fingerprint"),
        "[{\"entries\":[{\"exec_path\":\"\",\"key\":\"runtime\",\"value\":\"stable-x86_64\"}],\"integration\":\"rust\",\"producer\":\"//env:env_shard_alpha\"},\
         {\"entries\":[{\"exec_path\":\"\",\"key\":\"abi\",\"value\":\"gnu\"}],\"integration\":\"rust\",\"producer\":\"//env:env_shard_beta\"}]"
    );
}

#[test]
fn fingerprint_escapes_quotes_newlines_and_controls() {
    let records = vec![EnvRecord {
        producer: "//env:\"a\"\n".to_owned(),
        integration: "rust".to_owned(),
        entries: vec![EnvEntry {
            key: "k\"ey".to_owned(),
            value: "v\nalue\u{1}".to_owned(),
            exec_path: "out\\bin".to_owned(),
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
        parsed[0]["entries"][0]["value"],
        serde_json::Value::String("v\nalue\u{1}".to_owned())
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
            record("//env:evil", "rust", vec![entry("runtime", "nightly")]),
        ]),
        "env plan conflict: identity key 'runtime' claimed by //env:alpha, //env:evil"
    );
    assert_eq!(
        conflict_error(&[
            record_a(),
            record(
                "//env:alpha",
                "rust",
                vec![entry_with_exec("runtime", "stable-x86_64", "out/rustc")]
            ),
        ]),
        "env plan conflict: identity key 'runtime' claimed by //env:alpha, //env:alpha"
    );
}

#[test]
fn collect_plan_merges_hashes_and_sorts_deterministically() {
    let beta = shard_bytes("//env:beta", "rust", vec![("abi", "gnu", "lib/beta.so")]);
    let alpha = shard_bytes(
        "//env:alpha",
        "rust",
        vec![("runtime", "stable-x86_64", "lib/alpha.so")],
    );
    let forward = vec![
        output(
            "//env:alpha",
            vec![
                ("/out/a.dxenv.pb", alpha.clone()),
                ("/bazel-out/k8-fastbuild/bin/lib/alpha.so", vec![1]),
            ],
        ),
        output(
            "//env:beta",
            vec![
                ("/out/b.dxenv.pb", beta.clone()),
                ("/bazel-out/k8-fastbuild/bin/lib/beta.so", vec![2]),
            ],
        ),
    ];
    let reverse = vec![
        output(
            "//env:beta",
            vec![
                ("/out/b.dxenv.pb", beta),
                ("/bazel-out/k8-fastbuild/bin/lib/beta.so", vec![2]),
            ],
        ),
        output(
            "//env:alpha",
            vec![
                ("/out/a.dxenv.pb", alpha),
                ("/bazel-out/k8-fastbuild/bin/lib/alpha.so", vec![1]),
            ],
        ),
    ];
    let first = collect_plan(&forward).expect("plan");
    let second = collect_plan(&reverse).expect("plan");
    assert_eq!(first, second);
    assert_eq!(first.records.len(), 2);
    assert_eq!(first.records[0].producer, "//env:alpha");
    assert_eq!(
        first.fingerprint,
        fingerprint(&first.records).expect("fingerprint")
    );
    assert_eq!(first.digest, plan_digest(&first.fingerprint));
    let summary: Vec<(&str, &str)> = first
        .projection
        .iter()
        .map(|leaf| (leaf.key.as_str(), leaf.artifact.as_str()))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("abi", "/bazel-out/k8-fastbuild/bin/lib/beta.so"),
            ("runtime", "/bazel-out/k8-fastbuild/bin/lib/alpha.so"),
        ]
    );
}

#[test]
fn collect_plan_rejects_conflicts() {
    let outputs = vec![
        output(
            "//env:alpha",
            vec![(
                "/out/a.dxenv.pb",
                shard_bytes("//env:alpha", "rust", vec![("runtime", "stable", "")]),
            )],
        ),
        output(
            "//env:evil",
            vec![(
                "/out/e.dxenv.pb",
                shard_bytes("//env:evil", "rust", vec![("runtime", "nightly", "")]),
            )],
        ),
    ];
    assert_eq!(
        collect_plan(&outputs),
        Err(CollectError::Conflict(
            "env plan conflict: identity key 'runtime' claimed by //env:alpha, //env:evil"
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
            "//env:rust",
            vec![
                (
                    "/out/rust.dxenv.pb",
                    shard_bytes(
                        "//env:rust",
                        "rust",
                        vec![
                            ("toolchain", "1.89.0", "toolchain/rustc"),
                            ("abi", "gnu", ""),
                            ("runtime", "stable", "env/runtime.json"),
                        ],
                    ),
                ),
                ("/bazel-out/k8-fastbuild/bin/env/toolchain/rustc", vec![1]),
                ("/bazel-out/k8-fastbuild/bin/env/env/runtime.json", vec![2]),
            ],
        ),
        output(
            "//env:beta",
            vec![(
                "/out/beta.dxenv.pb",
                shard_bytes("//env:beta", "rust", vec![("extra", "1", "")]),
            )],
        ),
    ];
    let plan = collect_plan(&outputs).expect("plan");
    let projection = &plan.projection;
    let summary: Vec<(&str, &str)> = projection
        .iter()
        .map(|leaf| (leaf.key.as_str(), leaf.artifact.as_str()))
        .collect();
    assert_eq!(
        summary,
        vec![
            (
                "runtime",
                "/bazel-out/k8-fastbuild/bin/env/env/runtime.json"
            ),
            (
                "toolchain",
                "/bazel-out/k8-fastbuild/bin/env/toolchain/rustc"
            ),
        ]
    );
    assert_eq!(projection[0].value, "stable");
    assert_eq!(projection[1].value, "1.89.0");
}

#[test]
fn projection_shares_one_artifact_across_keys() {
    let outputs = vec![output(
        "//env:rust",
        vec![
            (
                "/out/rust.dxenv.pb",
                shard_bytes(
                    "//env:rust",
                    "rust",
                    vec![
                        ("b-key", "2", "shared/rustc"),
                        ("a-key", "1", "shared/rustc"),
                    ],
                ),
            ),
            ("/out/shared/rustc", vec![1]),
        ],
    )];
    let plan = collect_plan(&outputs).expect("plan");
    let projection = &plan.projection;
    assert_eq!(projection.len(), 2);
    assert_eq!(projection[0].key, "a-key");
    assert_eq!(projection[1].key, "b-key");
    assert_eq!(projection[0].artifact, "/out/shared/rustc");
    assert_eq!(projection[1].artifact, "/out/shared/rustc");
}

#[test]
fn projection_rejects_bad_indexes() {
    let missing_records = vec![record(
        "//env:rust",
        "rust",
        vec![entry_with_exec("toolchain", "1.89.0", "toolchain/rustc")],
    )];
    assert!(matches!(
        plan_projection(&missing_records, &[]),
        Err(CollectError::MissingArtifact { .. })
    ));
    let unreported_outputs = vec![output(
        "//env:beta",
        vec![
            (
                "/out/beta.dxenv.pb",
                shard_bytes("//env:beta", "rust", vec![("abi", "gnu", "")]),
            ),
            ("/out/backing.lib.rs", vec![1, 2, 3]),
        ],
    )];
    let logical_records = vec![record("//env:beta", "rust", vec![entry("abi", "gnu")])];
    assert!(matches!(
        plan_projection(&logical_records, &unreported_outputs),
        Err(CollectError::UnreportedArtifact { .. })
    ));
}
