"""In-graph compiler fixtures: the starter sources built by one selected C++ toolchain."""

load("//cc:defs.bzl", "cc_binary", "cc_library", "cc_test")
load("//cc/private/toolchains:select.bzl", "cc_toolchain_test", "toolchain_transition")
load("//rust:defs.bzl", "rust_test")
load(":defs.bzl", "COMPILER_PRODUCERS")

_STARTERS = "//tools/testing/starters/fixtures:"

def _renamed_impl(ctx):
    ctx.actions.symlink(output = ctx.outputs.out, target_file = ctx.file.src)
    return [DefaultInfo(files = depset([ctx.outputs.out]))]

_renamed = rule(
    implementation = _renamed_impl,
    doc = "Gives one file another name.",
    attrs = {
        "out": attr.output(doc = "The new name.", mandatory = True),
        "src": attr.label(doc = "File to rename.", allow_single_file = True, mandatory = True),
    },
)

def _built_with_impl(ctx):
    files = depset(transitive = [src[DefaultInfo].files for src in ctx.attr.srcs])
    return [DefaultInfo(files = files, runfiles = ctx.runfiles(transitive_files = files))]

_built_with = rule(
    implementation = _built_with_impl,
    doc = "Exposes the outputs of targets built with the selected compiler for the target platform.",
    attrs = {
        "compiler": attr.string(doc = "Compiler flag value to build with.", mandatory = True),
        "srcs": attr.label_list(doc = "Targets to build.", cfg = toolchain_transition),
    },
)

_CompileLogInfo = provider(doc = "The captured compile of one target.", fields = {"log": "File with the exit code and output."})

def _dirname(file):
    return file.dirname

def _compile_log_aspect_impl(target, ctx):
    upstream = getattr(ctx.rule.attr, "upstream", None)
    if type(upstream) == "list":
        upstream = upstream[0]
    if upstream and _CompileLogInfo in upstream:
        return [upstream[_CompileLogInfo]]
    compiles = [action for action in target.actions if action.mnemonic == "CppCompile"]
    if len(compiles) != 1:
        fail("%s: want one CppCompile action, got %d" % (target.label, len(compiles)))
    compile = compiles[0]
    log = ctx.actions.declare_file(target.label.name + ".compile_log")
    args = ctx.actions.args()
    args.add(log)
    args.add_all(compile.outputs, map_each = _dirname, uniquify = True)
    args.add("--")
    args.add_all(compile.argv)
    ctx.actions.run(
        executable = ctx.executable._capture,
        arguments = [args],
        inputs = compile.inputs,
        outputs = [log],
        env = compile.env,
        mnemonic = "CompileLog",
    )
    return [_CompileLogInfo(log = log)]

_compile_log_aspect = aspect(
    implementation = _compile_log_aspect_impl,
    doc = "Runs the compile action of a target through the capture tool.",
    attr_aspects = ["upstream"],
    attrs = {
        "_capture": attr.label(
            default = Label("//tools/testing/hermetic:compile_capture"),
            cfg = "exec",
            executable = True,
        ),
    },
)

def _compile_log_impl(ctx):
    log = ctx.attr.target[0][_CompileLogInfo].log
    return [DefaultInfo(files = depset([log]), runfiles = ctx.runfiles(files = [log]))]

compile_log = rule(
    implementation = _compile_log_impl,
    doc = "Compiles one single-source library with the selected compiler and records the exit code and output.",
    attrs = {
        "compiler": attr.string(doc = "Compiler flag value to build with.", mandatory = True),
        "target": attr.label(
            doc = "Library with one source file.",
            aspects = [_compile_log_aspect],
            cfg = toolchain_transition,
            mandatory = True,
        ),
    },
)

def starter_sources(name, language, files):
    """Declares the starter library, binary and test of a language, and its two probe libraries."""
    for file in [files["lib"], files["hdr"], files["main"], files["test"]]:
        _renamed(
            name = "%s_%s" % (name, file.replace(".", "_")),
            testonly = True,
            src = "%s%s/%s.fixture" % (_STARTERS, language, file),
            out = "%s/%s" % (name, file),
        )
    cc_library(
        name = name + "_lib",
        testonly = True,
        srcs = ["%s/%s" % (name, files["lib"])],
        hdrs = ["%s/%s" % (name, files["hdr"])],
    )
    cc_binary(
        name = name + "_bin",
        testonly = True,
        srcs = ["%s/%s" % (name, files["main"])],
        deps = [name + "_lib"],
    )
    cc_test(
        name = name + "_test",
        srcs = ["%s/%s" % (name, files["test"])],
        deps = [name + "_lib"],
    )
    cc_library(
        name = name + "_accepted",
        testonly = True,
        srcs = ["consumers/cc_fixture/" + files["accepted"]],
    )

    # manual: the compile fails; compile_log records it.
    cc_library(
        name = name + "_rejected",
        testonly = True,
        srcs = ["consumers/cc_fixture/" + files["rejected"]],
        tags = ["manual"],
    )

def compiler_fixture(name, sources, compiler, rejected, target_compatible_with):
    """Declares the in-graph fixture of one compiler over one starter_sources."""
    cc_toolchain_test(
        name = name + "_starter_test",
        compiler = compiler,
        target_compatible_with = target_compatible_with,
        test = sources + "_test",
    )
    _built_with(
        name = name + "_artifacts",
        testonly = True,
        srcs = [sources + "_accepted", sources + "_bin", sources + "_lib", sources + "_test"],
        compiler = compiler,
        target_compatible_with = target_compatible_with,
    )
    compile_log(
        name = name + "_rejected_log",
        testonly = True,
        compiler = compiler,
        target = sources + "_rejected",
        target_compatible_with = target_compatible_with,
    )
    rust_test(
        name = name + "_test",
        size = "small",
        srcs = ["//tools/testing/hermetic:compiler_fixture_test.rs"],
        crate_root = "//tools/testing/hermetic:compiler_fixture_test.rs",
        data = [name + "_artifacts", name + "_rejected_log"],
        env = {
            "DX_ARTIFACTS": "$(rootpaths %s_artifacts)" % name,
            "DX_PRODUCER": COMPILER_PRODUCERS[compiler],
            "DX_REJECTED": rejected,
            "DX_REJECTED_LOG": "$(rootpath %s_rejected_log)" % name,
        },
        target_compatible_with = target_compatible_with,
        deps = [
            "//cli/binfmt:dx_binfmt",
            "//tools/testing:dx_testing",
        ],
    )
