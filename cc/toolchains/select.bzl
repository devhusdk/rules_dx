"""Run a test or build a binary under an explicitly selected C++ toolchain."""

load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load("//core:wrapper.bzl", "dx_gcov_tool", "dx_lcov_merger_attr", "dx_symlink_executable", "dx_symlink_executable_name")

_COMPILER = str(Label("//cc/toolchains:compiler"))

_FEATURES = "//command_line_option:features"

_EXTRA_TOOLCHAINS = "//command_line_option:extra_toolchains"

_PLATFORMS = "//command_line_option:platforms"
_MODE = "//command_line_option:compilation_mode"

def _features(inherited, extra):
    features = list(inherited)
    for f in extra:
        if f not in features:
            features.append(f)
    return features

def _impl(settings, attr):
    features = _features(settings[_FEATURES], getattr(attr, "extra_features", []))
    extra_toolchains = list(getattr(attr, "extra_toolchains", []))
    for t in settings[_EXTRA_TOOLCHAINS]:
        if t not in extra_toolchains:
            extra_toolchains.append(t)
    return {
        _COMPILER: attr.compiler or settings[_COMPILER],
        _EXTRA_TOOLCHAINS: extra_toolchains,
        _FEATURES: features,
        _PLATFORMS: getattr(attr, "platforms", []) or settings[_PLATFORMS],
        _MODE: getattr(attr, "compilation_mode", "") or settings[_MODE],
    }

toolchain_transition = transition(
    implementation = _impl,
    inputs = [_COMPILER, _EXTRA_TOOLCHAINS, _FEATURES, _MODE, _PLATFORMS],
    outputs = [_COMPILER, _EXTRA_TOOLCHAINS, _FEATURES, _MODE, _PLATFORMS],
)

def _test_impl(ctx):
    src = ctx.attr.test[0]
    inner = src[DefaultInfo].files_to_run.executable
    upstream_runfiles = src[DefaultInfo].default_runfiles
    dll_srcs = [f for f in upstream_runfiles.files.to_list() if f.extension == "dll"]
    if dll_srcs:
        # Windows loads DLLs from the executable's directory, so give each test its own.
        exe = ctx.actions.declare_file(ctx.label.name + "_bin/" + dx_symlink_executable_name(ctx.label.name, inner))
        ctx.actions.symlink(output = exe, target_file = inner, is_executable = True)
    else:
        exe = dx_symlink_executable(ctx, inner)
    dlls = []
    seen = {}
    for f in dll_srcs:
        if f.basename in seen:
            continue
        seen[f.basename] = True
        dll = ctx.actions.declare_file(f.basename, sibling = exe)
        ctx.actions.symlink(output = dll, target_file = f)
        dlls.append(dll)
    runfiles = ctx.runfiles(files = [inner] + dlls).merge(upstream_runfiles)
    providers = [DefaultInfo(executable = exe, runfiles = runfiles)]
    if RunEnvironmentInfo in src:
        providers.append(src[RunEnvironmentInfo])
    providers.append(coverage_common.instrumented_files_info(ctx, dependency_attributes = ["test"]))
    return providers

cc_toolchain_test = rule(
    implementation = _test_impl,
    doc = "Runs a test built with the selected C++ toolchain.",
    attrs = {
        "compiler": attr.string(doc = "Compiler flag value to build with.", mandatory = True),
        "extra_toolchains": attr.string_list(doc = "Toolchain labels to register ahead of the registered ones.", default = []),
        "compilation_mode": attr.string(doc = "Compilation mode to build with, or empty to keep the command line value.", values = ["", "dbg", "fastbuild", "opt"]),
        "extra_features": attr.string_list(doc = "Features to enable on top of the command line features, such as asan.", default = []),
        "test": attr.label(doc = "Test to rebuild.", executable = True, cfg = toolchain_transition, mandatory = True),
    } | dx_lcov_merger_attr(),
    test = True,
)

def _files_transition_impl(settings, attr):
    features = _features(settings[_FEATURES], getattr(attr, "extra_features", []))
    return {
        _COMPILER: attr.compiler,
        _FEATURES: features,
        _PLATFORMS: attr.platforms,
    }

_files_transition = transition(
    implementation = _files_transition_impl,
    inputs = [_FEATURES],
    outputs = [_COMPILER, _FEATURES, _PLATFORMS],
)

def _files_impl(ctx):
    files = depset(transitive = [src[DefaultInfo].files for src in ctx.attr.srcs])
    return [DefaultInfo(files = files, runfiles = ctx.runfiles(transitive_files = files))]

cc_toolchain_files = rule(
    implementation = _files_impl,
    doc = "Exposes the outputs of targets built with the selected compiler for the given platforms.",
    attrs = {
        "compiler": attr.string(doc = "Compiler flag value to build with.", mandatory = True),
        "platforms": attr.string_list(doc = "Target platform labels.", mandatory = True),
        "srcs": attr.label_list(doc = "Targets to build.", cfg = _files_transition),
    },
)

def _binary_impl(ctx):
    inner = ctx.attr.binary[0][DefaultInfo].files_to_run.executable
    exe = dx_symlink_executable(ctx, inner)
    return [DefaultInfo(executable = exe, files = depset([exe]), runfiles = ctx.runfiles(files = [inner]))]

cc_toolchain_binary = rule(
    implementation = _binary_impl,
    doc = "Exposes one executable built with the selected compiler for the given platforms, so consumer_archive can package it.",
    attrs = {
        "binary": attr.label(doc = "Executable to build.", executable = True, cfg = _files_transition, mandatory = True),
        "compiler": attr.string(doc = "Compiler flag value to build with.", mandatory = True),
        "extra_features": attr.string_list(doc = "Features to enable on top of the command line features, such as dynamic_libstdcxx.", default = []),
        "platforms": attr.string_list(doc = "Target platform labels.", mandatory = True),
    },
    executable = True,
)

_COVERAGE = "//command_line_option:collect_code_coverage"
_FILTER = "//command_line_option:instrumentation_filter"

def _gcov_transition_impl(_settings, attr):
    return {
        _COMPILER: "gcc",
        _COVERAGE: True,
        _FILTER: attr.instrumentation_filter,
    }

_gcov_transition = transition(
    implementation = _gcov_transition_impl,
    inputs = [],
    outputs = [_COMPILER, _COVERAGE, _FILTER],
)

def _gcov_tool_impl(ctx):
    cc_toolchain = find_cc_toolchain(ctx)
    gcov = dx_gcov_tool(cc_toolchain)
    return [DefaultInfo(
        files = depset([gcov]),
        runfiles = ctx.runfiles(transitive_files = cc_toolchain.all_files),
    )]

gcov_tool = rule(
    implementation = _gcov_tool_impl,
    doc = "Exposes the gcov that ships with the selected C++ toolchain.",
    attrs = {"_cc_toolchain": attr.label(default = Label("@rules_cc//cc:current_cc_toolchain"))},
    toolchains = use_cc_toolchain(),
    fragments = ["cpp"],
)

def _gcov_fixture_impl(ctx):
    binaries = [t[DefaultInfo].files_to_run.executable for t in ctx.attr.tests]
    notes = [
        f
        for t in ctx.attr.tests + ctx.attr.libraries
        for f in t[InstrumentedFilesInfo].metadata_files.to_list()
        if f.extension == "gcno"
    ]
    gcov = ctx.attr._gcov[0][DefaultInfo].files.to_list()[0]
    lines = (
        ["test=" + b.short_path for b in binaries] +
        ["gcno=" + f.path + "=" + f.short_path for f in notes] +
        ["gcov=" + gcov.short_path]
    )
    paths = ctx.actions.declare_file(ctx.label.name + ".paths")
    ctx.actions.write(paths, "\n".join(lines) + "\n")
    runfiles = ctx.runfiles(files = binaries + notes + [gcov, paths])
    runfiles = runfiles.merge(ctx.attr._gcov[0][DefaultInfo].default_runfiles)
    for t in ctx.attr.tests:
        runfiles = runfiles.merge(t[DefaultInfo].default_runfiles)
    return [DefaultInfo(files = depset([paths]), runfiles = runfiles)]

cc_gcov_fixture = rule(
    implementation = _gcov_fixture_impl,
    doc = "Builds tests with GCC coverage instrumentation and hands over the binaries, their .gcno files and the toolchain gcov.",
    attrs = {
        "instrumentation_filter": attr.string(doc = "Regex of the labels to instrument.", default = "^//cc/tests/fixtures/(gcc_coverage|gcc_shared)[/:]"),
        "libraries": attr.label_list(doc = "Libraries a test loads dynamically, so their .gcno files are handed over too.", cfg = _gcov_transition),
        "tests": attr.label_list(doc = "Tests to instrument.", cfg = _gcov_transition, mandatory = True),
        "_gcov": attr.label(default = Label(":gcov_tool"), cfg = _gcov_transition),
    },
)
