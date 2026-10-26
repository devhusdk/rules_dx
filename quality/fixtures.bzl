"""Source fixtures for real aspects."""

load("//quality:sources.bzl", "QualitySourcesInfo", "check_direct_sources", "dockerfile_name_error")

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
    if len(ctx.files.graphql_srcs) > 0:
        direct_sources["graphql"] = depset(ctx.files.graphql_srcs)
    if len(ctx.files.html_srcs) > 0:
        direct_sources["html"] = depset(ctx.files.html_srcs)
    if len(ctx.files.json5_srcs) > 0:
        direct_sources["json5"] = depset(ctx.files.json5_srcs)
    if len(ctx.files.jsonc_srcs) > 0:
        direct_sources["jsonc"] = depset(ctx.files.jsonc_srcs)
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
    for dockerfile in ctx.files.dockerfile_srcs:
        error = dockerfile_name_error(dockerfile.basename)
        if error != "":
            fail("real_source_target %s: %s" % (ctx.label, error))
    if len(ctx.files.dockerfile_srcs) > 0:
        direct_sources["dockerfile"] = depset(ctx.files.dockerfile_srcs)
    if len(ctx.files.starlark_srcs) > 0:
        direct_sources["starlark"] = depset(ctx.files.starlark_srcs)
    if len(ctx.files.cue_srcs) > 0:
        direct_sources["cue"] = depset(ctx.files.cue_srcs)
    if len(ctx.files.jsonnet_srcs) > 0:
        direct_sources["jsonnet"] = depset(ctx.files.jsonnet_srcs)
    if len(ctx.files.pkl_srcs) > 0:
        direct_sources["pkl"] = depset(ctx.files.pkl_srcs)
    if len(ctx.files.yaml_srcs) > 0:
        direct_sources["yaml"] = depset(ctx.files.yaml_srcs)
    if len(ctx.files.html_template_srcs) > 0:
        direct_sources["html_template"] = depset(ctx.files.html_template_srcs)
    if len(ctx.files.terraform_srcs) > 0:
        direct_sources["terraform"] = depset(ctx.files.terraform_srcs)
    if len(ctx.files.text_srcs) > 0:
        direct_sources["text"] = depset(ctx.files.text_srcs)
    if len(ctx.files.toml_srcs) > 0:
        direct_sources["toml"] = depset(ctx.files.toml_srcs)
    if len(ctx.files.markdown_srcs) > 0:
        direct_sources["markdown"] = depset(ctx.files.markdown_srcs)
    if len(ctx.files.c_srcs) > 0:
        direct_sources["c"] = depset(ctx.files.c_srcs)
    if len(ctx.files.cpp_srcs) > 0:
        direct_sources["cpp"] = depset(ctx.files.cpp_srcs)
    if len(ctx.files.java_srcs) > 0:
        direct_sources["java"] = depset(ctx.files.java_srcs)
    if len(ctx.files.kotlin_srcs) > 0:
        direct_sources["kotlin"] = depset(ctx.files.kotlin_srcs)
    if len(ctx.files.scala_srcs) > 0:
        direct_sources["scala"] = depset(ctx.files.scala_srcs)
    check_direct_sources(direct_sources, str(ctx.label))
    sibling_error = markdown_sibling_error(
        str(ctx.label),
        ctx.attr.markdown_siblings,
        ctx.files.markdown_srcs,
    )
    if sibling_error != "":
        fail(sibling_error)
    all_files = list(ctx.files.c_srcs) + list(ctx.files.cpp_srcs) + list(ctx.files.javascript_srcs) + list(ctx.files.jsx_srcs) + list(ctx.files.typescript_srcs) + list(ctx.files.tsx_srcs) + list(ctx.files.json_srcs) + list(ctx.files.json5_srcs) + list(ctx.files.jsonc_srcs) + list(ctx.files.graphql_srcs) + list(ctx.files.html_srcs) + list(ctx.files.python_srcs) + list(ctx.files.python_stub_srcs) + list(ctx.files.rust_srcs) + list(ctx.files.shell_srcs) + list(ctx.files.dockerfile_srcs) + list(ctx.files.starlark_srcs) + list(ctx.files.cue_srcs) + list(ctx.files.jsonnet_srcs) + list(ctx.files.pkl_srcs) + list(ctx.files.yaml_srcs) + list(ctx.files.html_template_srcs) + list(ctx.files.terraform_srcs) + list(ctx.files.text_srcs) + list(ctx.files.toml_srcs) + list(ctx.files.markdown_srcs) + list(ctx.files.java_srcs) + list(ctx.files.kotlin_srcs) + list(ctx.files.scala_srcs)
    return [
        DefaultInfo(files = depset(all_files)),
        QualitySourcesInfo(direct_sources = direct_sources),
    ]

real_source_target = rule(
    implementation = _real_source_target_impl,
    doc = "Sources by language for real quality aspects.",
    attrs = {
        "c_srcs": attr.label_list(
            allow_files = [".c", ".h"],
            default = [],
        ),
        "cpp_srcs": attr.label_list(
            allow_files = [".cc", ".cpp", ".cxx", ".hh", ".hpp", ".hxx"],
            default = [],
        ),
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
        "dockerfile_srcs": attr.label_list(
            allow_files = True,
            default = [],
        ),
        "graphql_srcs": attr.label_list(
            allow_files = [".graphql", ".gql"],
            default = [],
        ),
        "html_srcs": attr.label_list(
            allow_files = [".html", ".htm"],
            default = [],
        ),
        "json5_srcs": attr.label_list(
            allow_files = [".json5"],
            default = [],
        ),
        "jsonc_srcs": attr.label_list(
            allow_files = [".jsonc"],
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
        "scala_srcs": attr.label_list(
            allow_files = [".scala"],
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
        "cue_srcs": attr.label_list(
            allow_files = [".cue"],
            default = [],
        ),
        "jsonnet_srcs": attr.label_list(
            allow_files = [".jsonnet", ".libsonnet"],
            default = [],
        ),
        "html_template_srcs": attr.label_list(
            allow_files = [".html", ".htm", ".jinja", ".jinja2", ".j2", ".njk", ".hbs", ".tmpl", ".gohtml"],
            default = [],
        ),
        "yaml_srcs": attr.label_list(
            allow_files = [".yaml", ".yml"],
            default = [],
        ),
        "pkl_srcs": attr.label_list(
            allow_files = [".pkl"],
            default = [],
        ),
        "terraform_srcs": attr.label_list(
            allow_files = [".tf", ".tfvars", ".hcl"],
            default = [],
        ),
        "text_srcs": attr.label_list(
            allow_files = [".txt"],
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
