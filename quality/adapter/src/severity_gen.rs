// Generated from quality/tools.bzl. Run `bazel run //quality:update_tools` to refresh it.

pub const TOOL_SEVERITIES: &[(&str, crate::severity::ToolSeverityMap)] = &[
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
        "jsonnet_lint",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
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
        "tinymist",
        crate::severity::ToolSeverityMap {
            default: Some(crate::ToolSeverity::Warning),
            categories: &[],
        },
    ),
];
