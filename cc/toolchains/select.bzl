"""Run a test or build a binary under an explicitly selected C++ toolchain."""

load("//libs/starlark:wrapper.bzl", "dx_lcov_merger_attr", "dx_symlink_executable")

_COMPILER = str(Label("//cc/toolchains:compiler"))

_FEATURES = "//command_line_option:features"

_EXTRA_TOOLCHAINS = "//command_line_option:extra_toolchains"

_PLATFORMS = "//command_line_option:platforms"

def _impl(settings, attr):
    features = list(settings[_FEATURES])
    for f in getattr(attr, "extra_features", []):
        if f not in features:
            features.append(f)
    extra_toolchains = list(getattr(attr, "extra_toolchains", []))
    for t in settings[_EXTRA_TOOLCHAINS]:
        if t not in extra_toolchains:
            extra_toolchains.append(t)
    return {
        _COMPILER: attr.compiler or settings[_COMPILER],
        _EXTRA_TOOLCHAINS: extra_toolchains,
        _FEATURES: features,
        _PLATFORMS: getattr(attr, "platforms", []) or settings[_PLATFORMS],
    }

toolchain_transition = transition(
    implementation = _impl,
    inputs = [_COMPILER, _EXTRA_TOOLCHAINS, _FEATURES, _PLATFORMS],
    outputs = [_COMPILER, _EXTRA_TOOLCHAINS, _FEATURES, _PLATFORMS],
)

def _test_impl(ctx):
    src = ctx.attr.test[0]
    inner = src[DefaultInfo].files_to_run.executable
    exe = dx_symlink_executable(ctx, inner)
    runfiles = ctx.runfiles(files = [inner]).merge(src[DefaultInfo].default_runfiles)
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
        "extra_features": attr.string_list(doc = "Features to enable on top of the command line features, such as asan.", default = []),
        "test": attr.label(doc = "Test to rebuild.", executable = True, cfg = toolchain_transition, mandatory = True),
    } | dx_lcov_merger_attr(),
    test = True,
)

def _files_transition_impl(settings, attr):
    features = list(settings[_FEATURES])
    for f in getattr(attr, "extra_features", []):
        if f not in features:
            features.append(f)
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
