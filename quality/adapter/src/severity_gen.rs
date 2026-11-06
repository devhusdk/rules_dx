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
];
