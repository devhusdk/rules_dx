// Generated from quality/tools.bzl. Run `bazel run //quality:update_tools` to refresh it.

pub const TOOL_SEVERITIES: &[(&str, crate::severity::ToolSeverityMap)] = &[
    (
        "ansible_lint",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[
                ("internal-error", crate::ToolSeverity::Error),
                ("load-failure", crate::ToolSeverity::Error),
                ("syntax-check", crate::ToolSeverity::Error),
            ],
        },
    ),
    (
        "buf",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[("COMPILE", crate::ToolSeverity::Error)],
        },
    ),
    (
        "chktex",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "clang_tidy",
        crate::severity::ToolSeverityMap {
            default: None,
            categories: &[
                ("bugprone-", crate::ToolSeverity::Warning),
                ("cert-", crate::ToolSeverity::Warning),
                ("clang-analyzer-", crate::ToolSeverity::Warning),
                ("clang-diagnostic-error", crate::ToolSeverity::Error),
                ("cppcoreguidelines-", crate::ToolSeverity::Warning),
                ("misc-", crate::ToolSeverity::Warning),
                ("modernize-", crate::ToolSeverity::Warning),
                ("performance-", crate::ToolSeverity::Warning),
                ("readability-", crate::ToolSeverity::Warning),
            ],
        },
    ),
    (
        "clippy",
        crate::severity::ToolSeverityMap {
            default: None,
            categories: &[
                ("clippy::", crate::ToolSeverity::Warning),
                (
                    "clippy::absurd_extreme_comparisons",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::almost_swapped", crate::ToolSeverity::Error),
                ("clippy::approx_constant", crate::ToolSeverity::Error),
                ("clippy::async_yields_async", crate::ToolSeverity::Error),
                ("clippy::bad_bit_mask", crate::ToolSeverity::Error),
                (
                    "clippy::cast_slice_different_sizes",
                    crate::ToolSeverity::Error,
                ),
                (
                    "clippy::char_indices_as_byte_indices",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::deprecated_semver", crate::ToolSeverity::Error),
                (
                    "clippy::derive_ord_xor_partial_ord",
                    crate::ToolSeverity::Error,
                ),
                (
                    "clippy::derived_hash_with_manual_eq",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::eager_transmute", crate::ToolSeverity::Error),
                (
                    "clippy::enum_clike_unportable_variant",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::eq_op", crate::ToolSeverity::Error),
                ("clippy::erasing_op", crate::ToolSeverity::Error),
                ("clippy::if_let_mutex", crate::ToolSeverity::Error),
                ("clippy::ifs_same_cond", crate::ToolSeverity::Error),
                (
                    "clippy::impl_hash_borrow_with_str_and_bytes",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::impossible_comparisons", crate::ToolSeverity::Error),
                ("clippy::ineffective_bit_mask", crate::ToolSeverity::Error),
                ("clippy::infinite_iter", crate::ToolSeverity::Error),
                (
                    "clippy::inherent_to_string_shadow_display",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::inline_fn_without_body", crate::ToolSeverity::Error),
                ("clippy::invalid_regex", crate::ToolSeverity::Error),
                (
                    "clippy::inverted_saturating_sub",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::invisible_characters", crate::ToolSeverity::Error),
                ("clippy::iter_next_loop", crate::ToolSeverity::Error),
                ("clippy::iter_skip_zero", crate::ToolSeverity::Error),
                ("clippy::iterator_step_by_zero", crate::ToolSeverity::Error),
                ("clippy::let_underscore_lock", crate::ToolSeverity::Error),
                ("clippy::lint_groups_priority", crate::ToolSeverity::Error),
                (
                    "clippy::match_str_case_mismatch",
                    crate::ToolSeverity::Error,
                ),
                (
                    "clippy::mem_replace_with_uninit",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::min_max", crate::ToolSeverity::Error),
                (
                    "clippy::mistyped_literal_suffixes",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::modulo_one", crate::ToolSeverity::Error),
                ("clippy::mut_from_ref", crate::ToolSeverity::Error),
                ("clippy::never_loop", crate::ToolSeverity::Error),
                (
                    "clippy::non_octal_unix_permissions",
                    crate::ToolSeverity::Error,
                ),
                (
                    "clippy::nonsensical_open_options",
                    crate::ToolSeverity::Error,
                ),
                (
                    "clippy::not_unsafe_ptr_arg_deref",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::option_env_unwrap", crate::ToolSeverity::Error),
                ("clippy::out_of_bounds_indexing", crate::ToolSeverity::Error),
                (
                    "clippy::panicking_overflow_checks",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::panicking_unwrap", crate::ToolSeverity::Error),
                ("clippy::possible_missing_comma", crate::ToolSeverity::Error),
                ("clippy::read_line_without_trim", crate::ToolSeverity::Error),
                ("clippy::recursive_format_impl", crate::ToolSeverity::Error),
                ("clippy::redundant_comparisons", crate::ToolSeverity::Error),
                ("clippy::reversed_empty_ranges", crate::ToolSeverity::Error),
                ("clippy::self_assignment", crate::ToolSeverity::Error),
                ("clippy::serde_api_misuse", crate::ToolSeverity::Error),
                (
                    "clippy::size_of_in_element_count",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::suspicious_splitn", crate::ToolSeverity::Error),
                ("clippy::transmute_null_to_fn", crate::ToolSeverity::Error),
                ("clippy::transmuting_null", crate::ToolSeverity::Error),
                ("clippy::uninit_assumed_init", crate::ToolSeverity::Error),
                ("clippy::uninit_vec", crate::ToolSeverity::Error),
                ("clippy::unit_cmp", crate::ToolSeverity::Error),
                ("clippy::unit_hash", crate::ToolSeverity::Error),
                (
                    "clippy::unit_return_expecting_ord",
                    crate::ToolSeverity::Error,
                ),
                (
                    "clippy::unsound_collection_transmute",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::unused_io_amount", crate::ToolSeverity::Error),
                ("clippy::useless_attribute", crate::ToolSeverity::Error),
                ("clippy::vec_resize_to_zero", crate::ToolSeverity::Error),
                (
                    "clippy::while_immutable_condition",
                    crate::ToolSeverity::Error,
                ),
                ("clippy::wrong_transmute", crate::ToolSeverity::Error),
                ("clippy::zst_offset", crate::ToolSeverity::Error),
            ],
        },
    ),
    (
        "cmakelint",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "credo",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "dialyzer",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Error),
            categories: &[],
        },
    ),
    (
        "elvis",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "errcheck",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "fortitude",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "gotype",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Error),
            categories: &[],
        },
    ),
    (
        "govet",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "gplint",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "hadolint",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[("DL1", crate::ToolSeverity::Error)],
        },
    ),
    (
        "jsonnet_lint",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "kubeconform",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Error),
            categories: &[],
        },
    ),
    (
        "lychee",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "markdown_check",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "perlcritic",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "phpcs",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "phpstan",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Error),
            categories: &[("phpstan/public.", crate::ToolSeverity::Info)],
        },
    ),
    (
        "pkl_lsp",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "pydoclint",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "redocly",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "regal",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "rubocop",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[("Security/", crate::ToolSeverity::Error)],
        },
    ),
    (
        "ruff",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[
                ("E9", crate::ToolSeverity::Error),
                ("F82", crate::ToolSeverity::Error),
                ("S1", crate::ToolSeverity::Error),
                ("S2", crate::ToolSeverity::Error),
                ("S3", crate::ToolSeverity::Error),
                ("S4", crate::ToolSeverity::Error),
                ("S5", crate::ToolSeverity::Error),
                ("S6", crate::ToolSeverity::Error),
                ("S7", crate::ToolSeverity::Error),
            ],
        },
    ),
    (
        "sqlfluff",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[
                ("AM", crate::ToolSeverity::Error),
                ("LXR", crate::ToolSeverity::Error),
                ("PRS", crate::ToolSeverity::Error),
                ("RF", crate::ToolSeverity::Error),
                ("ST", crate::ToolSeverity::Error),
                ("TMP", crate::ToolSeverity::Error),
            ],
        },
    ),
    (
        "staticcheck",
        crate::severity::ToolSeverityMap {
            default: None,
            categories: &[
                ("QF", crate::ToolSeverity::Info),
                ("S", crate::ToolSeverity::Warning),
                ("U1000", crate::ToolSeverity::Warning),
            ],
        },
    ),
    (
        "tflint",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[("tflint-diagnostic", crate::ToolSeverity::Error)],
        },
    ),
    (
        "tinymist",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
    (
        "ty",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Error),
            categories: &[],
        },
    ),
    (
        "yamllint",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[
                ("anchors", crate::ToolSeverity::Error),
                ("key-duplicates", crate::ToolSeverity::Error),
                ("syntax", crate::ToolSeverity::Error),
            ],
        },
    ),
];
