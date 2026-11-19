"""Rules that generate CXX-Qt bridge C++ and stage the CXX-Qt runtime."""

load(":constants.bzl", "QT_TOOL_ENV")
load(":text.bzl", "qt_package_path", "qt_relative", "qt_stem")

_CXX_QT_FILES = Label("//qt/private/tools/rust:cxx_qt_runtime_files")

# cxx-qt formats the C++ it writes with the clang-format this names, read
# in-process. Empty means none, so a host clang-format never changes the output.
_BRIDGE_ENV = QT_TOOL_ENV | {"CLANG_FORMAT_BINARY": ""}

def _generator(ctx):
    if ctx.executable.generator:
        return ctx.executable.generator, []
    return ctx.executable._dx, ["qt_rust_bridge"]

def _bridge(ctx, mode, src, outputs, include = None):
    executable, prefix = _generator(ctx)
    args = ctx.actions.args()
    args.add_all(prefix)
    args.add(mode)
    args.add(src)
    if include != None:
        args.add(include)
    args.add_all(outputs)
    ctx.actions.run(
        executable = executable,
        arguments = [args],
        inputs = [src],
        outputs = outputs,
        env = _BRIDGE_ENV,
        mnemonic = "QtRustBridge",
        progress_message = "Generating C++ for %{input}",
    )

def _qt_rust_bridge_impl(ctx):
    headers = []
    sources = []
    qobjects = []
    for src in ctx.files.bridges:
        stem = qt_relative(ctx, src, "bridge")[:-len(".rs")]
        if "/" in stem:
            fail(str(ctx.label) + ": bridge '" + stem + ".rs' must sit in the package directory, not a subdirectory")
        qobject_header = ctx.actions.declare_file(stem + ".cxxqt.h")
        qobject_source = ctx.actions.declare_file(stem + ".cxxqt.cpp")
        header = ctx.actions.declare_file(stem + ".cxx.h")
        source = ctx.actions.declare_file(stem + ".cxx.cpp")
        include = qt_package_path(src.short_path, "")[:-len(".rs")]
        _bridge(ctx, "qt", src, [qobject_header, qobject_source, header, source], include)
        headers.extend([qobject_header, header])
        sources.extend([qobject_source, source])
        qobjects.append(qobject_header)
    return [
        DefaultInfo(files = depset(headers + sources)),
        OutputGroupInfo(
            qt_rust_hdrs = depset(headers),
            qt_rust_moc_hdrs = depset(qobjects),
            qt_rust_srcs = depset(sources),
        ),
    ]

qt_rust_bridge = rule(
    implementation = _qt_rust_bridge_impl,
    doc = "Generates the C++ side of CXX-Qt bridges.",
    attrs = {
        "bridges": attr.label_list(allow_files = [".rs"], allow_empty = False, mandatory = True, doc = "Rust files holding one #[cxx_qt::bridge] module each."),
        "generator": attr.label(cfg = "exec", executable = True, doc = "Bridge generator; defaults to `dx qt_rust_bridge`."),
        "_dx": attr.label(default = Label("@dx_prebuilt//:dx"), executable = True, cfg = "exec"),
    },
)

def _qt_rust_runtime_files_impl(ctx):
    root = ctx.label.name + "/"
    headers = []
    sources = []
    cxx_h = ctx.actions.declare_file(root + "rust/cxx.h")
    executable, prefix = _generator(ctx)
    args = ctx.actions.args()
    args.add_all(prefix)
    args.add("header")
    args.add(cxx_h)
    ctx.actions.run(
        executable = executable,
        arguments = [args],
        outputs = [cxx_h],
        env = QT_TOOL_ENV,
        mnemonic = "QtRustBridge",
        progress_message = "Writing rust/cxx.h",
    )
    headers.append(cxx_h)
    for f in ctx.files.files:
        if f.extension == "h":
            out = ctx.actions.declare_file(root + "cxx-qt/" + f.basename)
            ctx.actions.symlink(output = out, target_file = f)
            headers.append(out)
        elif f.extension == "cpp":
            out = ctx.actions.declare_file(root + f.basename)
            ctx.actions.symlink(output = out, target_file = f)
            sources.append(out)
        else:
            header = ctx.actions.declare_file(root + qt_stem(f) + ".cxx.h")
            source = ctx.actions.declare_file(root + qt_stem(f) + ".cxx.cpp")
            _bridge(ctx, "cxx", f, [header, source])
            headers.append(header)
            sources.append(source)
    return [
        DefaultInfo(files = depset(headers + sources)),
        OutputGroupInfo(
            qt_rust_hdrs = depset(headers),
            qt_rust_srcs = depset(sources),
        ),
    ]

qt_rust_runtime_files = rule(
    implementation = _qt_rust_runtime_files_impl,
    doc = "Stages the CXX-Qt runtime headers and generates its CXX bridges.",
    attrs = {
        "files": attr.label(allow_files = [".h", ".cpp", ".rs"], default = _CXX_QT_FILES, doc = "CXX-Qt crate headers, C++ sources and bridges."),
        "generator": attr.label(cfg = "exec", executable = True, doc = "Bridge generator; defaults to `dx qt_rust_bridge`."),
        "_dx": attr.label(default = Label("@dx_prebuilt//:dx"), executable = True, cfg = "exec"),
    },
)
