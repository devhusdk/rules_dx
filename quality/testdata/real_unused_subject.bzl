"""Analysis subject observing the real unused dx_results."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load(
    "//quality:real_aspects.bzl",
    "real_cc_unused_aspect",
    "real_elixir_unused_aspect",
    "real_erlang_unused_aspect",
    "real_js_unused_aspect",
    "real_php_unused_aspect",
    "real_python_unused_aspect",
    "real_ruby_unused_aspect",
)

def _real_unused_subject_impl(ctx):
    target = ctx.attr.target
    dx_files = []
    if OutputGroupInfo in target and "dx_results" in target[OutputGroupInfo]:
        dx_files = sorted(target[OutputGroupInfo]["dx_results"].to_list(), key = lambda f: f.basename)
    basenames = [f.basename for f in dx_files]
    return [
        DefaultInfo(files = depset([])),
        OutputGroupInfo(dx_results = depset(dx_files)),
        DxSubjectInfo(fields = {
            "dx_count": str(len(basenames)),
            "dx_results": ",".join(basenames) if len(basenames) > 0 else "(none)",
            "label": strip_canonical(str(ctx.attr.target.label)),
        }),
    ]

real_unused_subject = rule(
    implementation = _real_unused_subject_impl,
    attrs = {
        "target": attr.label(
            aspects = [
                real_cc_unused_aspect,
                real_elixir_unused_aspect,
                real_erlang_unused_aspect,
                real_js_unused_aspect,
                real_php_unused_aspect,
                real_python_unused_aspect,
                real_ruby_unused_aspect,
            ],
            mandatory = True,
        ),
    },
)
