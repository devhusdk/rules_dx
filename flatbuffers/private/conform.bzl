"""FlatBuffers breaking-change check (flatc --conform)."""

load("@bazel_skylib//rules:build_test.bzl", "build_test")
load("//core:wrapper.bzl", "dx_quality_sources")
load(":defs.bzl", "FlatBuffersInfo", "resolve_root")

def _roots(files):
    return sorted({f.owner.workspace_root: True for f in files if f.owner and f.owner.workspace_root}.keys())

def _conform_impl(ctx):
    lib = ctx.attr.library[FlatBuffersInfo]
    base = ctx.attr.baseline[FlatBuffersInfo]
    root = resolve_root("flatbuffers_conform", ctx.label, ctx.attr.library, ctx.file.root)
    baseline_root = resolve_root("flatbuffers_conform baseline", ctx.label, ctx.attr.baseline, ctx.file.baseline_root)
    stem = root.basename[:-len("." + root.extension)]
    out = ctx.actions.declare_file(ctx.label.name + "_conform/" + stem + ".bfbs")
    args = ["--binary", "--schema", "-I", "."]
    for r in _roots(lib.transitive_srcs.to_list()):
        args += ["-I", r]
    args += ["--conform", baseline_root.path, "--conform-includes", "."]
    for r in _roots(base.transitive_srcs.to_list()):
        args += ["--conform-includes", r]
    args += ["-o", out.dirname, root.path]
    ctx.actions.run(
        executable = ctx.executable._flatc,
        inputs = depset(transitive = [lib.transitive_srcs, base.transitive_srcs]),
        outputs = [out],
        arguments = args,
        mnemonic = "FlatcConform",
        progress_message = "flatc --conform " + root.short_path,
    )
    return [
        DefaultInfo(files = depset([out])),
        dx_quality_sources([], [], str(ctx.label)),
    ]

_flatbuffers_conform = rule(
    implementation = _conform_impl,
    doc = "Runs flatc --conform of a library root against a baseline root and writes the binary schema.",
    attrs = {
        "library": attr.label(providers = [FlatBuffersInfo], mandatory = True, doc = "flatbuffers_library to check."),
        "root": attr.label(allow_single_file = [".fbs"], doc = "Root schema of library; optional when it owns one schema."),
        "baseline": attr.label(providers = [FlatBuffersInfo], mandatory = True, doc = "flatbuffers_library holding the previous schema."),
        "baseline_root": attr.label(allow_single_file = [".fbs"], doc = "Root schema of baseline; optional when it owns one schema."),
        "_flatc": attr.label(default = Label("@dx_flatbuffers//:flatc"), executable = True, cfg = "exec"),
    },
)

flatbuffers_conform = _flatbuffers_conform

def flatbuffers_conform_test(name, library, baseline, root = None, baseline_root = None, **kwargs):
    """Fails the build when library breaks compatibility with baseline."""
    _flatbuffers_conform(
        name = name + "_conform",
        library = library,
        baseline = baseline,
        root = root,
        baseline_root = baseline_root,
        tags = ["manual"],
        testonly = True,
    )
    build_test(name = name, targets = [":" + name + "_conform"], **kwargs)
