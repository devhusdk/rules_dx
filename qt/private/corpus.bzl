"""Source corpus targets for the quality aspects."""

load("//core/quality:sources.bzl", "quality_sources_info")

_CLASSES = {
    "markdown": [".md"],
    "starlark": [".bzl", ".bazel"],
    "toml": [".toml"],
}

def _qt_corpus_impl(ctx):
    classes = {class_id: getattr(ctx.files, class_id + "_srcs") for class_id in _CLASSES}
    return [
        DefaultInfo(files = depset(ctx.files.markdown_srcs + ctx.files.starlark_srcs + ctx.files.toml_srcs)),
        quality_sources_info(classes, str(ctx.label)),
    ]

qt_corpus = rule(
    implementation = _qt_corpus_impl,
    doc = "Carries qt source files for the quality aspects.",
    attrs = {
        class_id + "_srcs": attr.label_list(allow_files = extensions, default = [])
        for class_id, extensions in _CLASSES.items()
    },
)
