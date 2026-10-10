"""Source fixtures for aspects (synthetic, real)."""

load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources")

def _quality_source_target_impl(ctx):
    direct_sources = {}
    if len(ctx.files.python_srcs) > 0:
        direct_sources["python"] = depset(ctx.files.python_srcs)
    if len(ctx.files.rust_srcs) > 0:
        direct_sources["rust"] = depset(ctx.files.rust_srcs)
    check_direct_sources(direct_sources, str(ctx.label))
    all_files = list(ctx.files.python_srcs) + list(ctx.files.rust_srcs)
    return [
        DefaultInfo(files = depset(all_files)),
        QualitySourcesInfo(direct_sources = direct_sources),
    ]

quality_source_target = rule(
    implementation = _quality_source_target_impl,
    attrs = {
        "python_srcs": attr.label_list(
            allow_files = [".py"],
            default = [],
        ),
        "rust_srcs": attr.label_list(
            allow_files = [".rs"],
            default = [],
        ),
    },
)

def markdown_sibling_error(what, siblings, markdown_srcs):
    """Returns the validation error for markdown_siblings, or "" when valid."""
    if len(siblings) > 0 and len(markdown_srcs) == 0:
        return ("real_source_target (" + what + "): markdown_siblings needs " +
                "markdown_srcs; siblings are link targets for checked markdown, " +
                "never sources")
    return ""

def _real_source_target_impl(ctx):
    direct_sources = {}
    if len(ctx.files.javascript_srcs) > 0:
        direct_sources["javascript"] = depset(ctx.files.javascript_srcs)
    if len(ctx.files.jsx_srcs) > 0:
        direct_sources["jsx"] = depset(ctx.files.jsx_srcs)
    if len(ctx.files.typescript_srcs) > 0:
        direct_sources["typescript"] = depset(ctx.files.typescript_srcs)
    if len(ctx.files.tsx_srcs) > 0:
        direct_sources["tsx"] = depset(ctx.files.tsx_srcs)
    if len(ctx.files.json_srcs) > 0:
        direct_sources["json"] = depset(ctx.files.json_srcs)
    if len(ctx.files.python_srcs) > 0:
        direct_sources["python"] = depset(ctx.files.python_srcs)
    if len(ctx.files.python_stub_srcs) > 0:
        direct_sources["python_stub"] = depset(ctx.files.python_stub_srcs)
    if len(ctx.files.rust_srcs) > 0:
        direct_sources["rust"] = depset(ctx.files.rust_srcs)
    if len(ctx.files.shell_srcs) > 0:
        direct_sources["shell"] = depset(ctx.files.shell_srcs)
    if len(ctx.files.starlark_srcs) > 0:
        direct_sources["starlark"] = depset(ctx.files.starlark_srcs)
    if len(ctx.files.toml_srcs) > 0:
        direct_sources["toml"] = depset(ctx.files.toml_srcs)
    if len(ctx.files.markdown_srcs) > 0:
        direct_sources["markdown"] = depset(ctx.files.markdown_srcs)
    if len(ctx.files.java_srcs) > 0:
        direct_sources["java"] = depset(ctx.files.java_srcs)
    if len(ctx.files.kotlin_srcs) > 0:
        direct_sources["kotlin"] = depset(ctx.files.kotlin_srcs)
    check_direct_sources(direct_sources, str(ctx.label))
    sibling_error = markdown_sibling_error(
        str(ctx.label),
        ctx.attr.markdown_siblings,
        ctx.files.markdown_srcs,
    )
    if sibling_error != "":
        fail(sibling_error)
    all_files = list(ctx.files.javascript_srcs) + list(ctx.files.jsx_srcs) + list(ctx.files.typescript_srcs) + list(ctx.files.tsx_srcs) + list(ctx.files.json_srcs) + list(ctx.files.python_srcs) + list(ctx.files.python_stub_srcs) + list(ctx.files.rust_srcs) + list(ctx.files.shell_srcs) + list(ctx.files.starlark_srcs) + list(ctx.files.toml_srcs) + list(ctx.files.markdown_srcs) + list(ctx.files.java_srcs) + list(ctx.files.kotlin_srcs)
    return [
        DefaultInfo(files = depset(all_files)),
        QualitySourcesInfo(direct_sources = direct_sources),
    ]

real_source_target = rule(
    implementation = _real_source_target_impl,
    attrs = {
        "java_srcs": attr.label_list(
            allow_files = [".java"],
            default = [],
        ),
        "javascript_srcs": attr.label_list(
            allow_files = [".js", ".mjs", ".cjs"],
            default = [],
        ),
        "kotlin_srcs": attr.label_list(
            allow_files = [".kt", ".kts"],
            default = [],
        ),
        "json_srcs": attr.label_list(
            allow_files = [".json"],
            default = [],
        ),
        "jsx_srcs": attr.label_list(
            allow_files = [".jsx"],
            default = [],
        ),
        "markdown_siblings": attr.label_list(
            allow_files = True,
            default = [],
        ),
        "markdown_srcs": attr.label_list(
            allow_files = [".md"],
            default = [],
        ),
        "python_srcs": attr.label_list(
            allow_files = [".py"],
            default = [],
        ),
        "python_stub_srcs": attr.label_list(
            allow_files = [".pyi"],
            default = [],
        ),
        "rust_srcs": attr.label_list(
            allow_files = [".rs"],
            default = [],
        ),
        "shell_srcs": attr.label_list(
            allow_files = [".sh"],
            default = [],
        ),
        "starlark_srcs": attr.label_list(
            allow_files = [".bzl", ".bazel"],
            default = [],
        ),
        "toml_srcs": attr.label_list(
            allow_files = [".toml"],
            default = [],
        ),
        "tsx_srcs": attr.label_list(
            allow_files = [".tsx"],
            default = [],
        ),
        "typescript_srcs": attr.label_list(
            allow_files = [".ts", ".mts", ".cts"],
            default = [],
        ),
    },
)
