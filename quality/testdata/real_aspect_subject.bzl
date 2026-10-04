"""Analysis subject observing real-aspect dx_results."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load(
    "//quality:real_aspects.bzl",
    "real_cpp_format_aspect",
    "real_format_aspect",
    "real_go_format_aspect",
    "real_shell_format_aspect",
    "real_jsonnet_format_aspect",
    "real_yaml_format_aspect",
    "real_shell_lint_aspect",
    "real_go_lint_aspect",
    "real_js_format_aspect",
    "real_js_lint_aspect",
    "real_jvm_format_aspect",
    "real_jvm_lint_aspect",
    "real_lint_aspect",
    "real_python_lint_aspect",
    "real_rust_format_aspect",
    "real_rust_lint_aspect",
)
load("//quality:sources.bzl", "QualitySourcesInfo")

def _label_text(label):
    return strip_canonical(str(label))

def _real_aspect_subject_impl(ctx):
    target = ctx.attr.target
    dx_files = []
    if OutputGroupInfo in target:
        groups = target[OutputGroupInfo]
        if "dx_results" in groups:
            dx_files = sorted(
                groups["dx_results"].to_list(),
                key = lambda f: f.basename,
            )
    basenames = [f.basename for f in dx_files]
    return [
        DefaultInfo(files = depset([])),
        OutputGroupInfo(dx_results = depset(dx_files)),
        DxSubjectInfo(fields = {
            "dx_count": str(len(basenames)),
            "dx_results": ",".join(basenames) if len(basenames) > 0 else "(none)",
            "has_quality_sources": str(QualitySourcesInfo in target),
            "label": _label_text(ctx.attr.target.label),
        }),
    ]

real_aspect_subject = rule(
    implementation = _real_aspect_subject_impl,
    attrs = {
        "target": attr.label(
            aspects = [
                real_lint_aspect,
                real_format_aspect,
                real_js_lint_aspect,
                real_js_format_aspect,
                real_python_lint_aspect,
                real_jvm_lint_aspect,
                real_jvm_format_aspect,
                real_cpp_format_aspect,
                real_go_format_aspect,
                real_shell_format_aspect,
                real_jsonnet_format_aspect,
                real_yaml_format_aspect,
                real_shell_lint_aspect,
                real_go_lint_aspect,
                real_rust_lint_aspect,
                real_rust_format_aspect,
            ],
            mandatory = True,
        ),
    },
)
