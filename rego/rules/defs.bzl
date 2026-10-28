"""Rego source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "RegoInfo")

def _rego_library_impl(ctx):
    transitive_srcs = depset(
        ctx.files.srcs,
        transitive = [dep[RegoInfo].transitive_srcs for dep in ctx.attr.deps],
    )
    transitive_data = depset(
        ctx.files.data,
        transitive = [dep[RegoInfo].transitive_data for dep in ctx.attr.deps],
    )
    return [
        DefaultInfo(files = depset(transitive = [transitive_srcs, transitive_data])),
        RegoInfo(transitive_srcs = transitive_srcs, transitive_data = transitive_data),
        dx_quality_sources(ctx.files.srcs, [("rego", ["rego"])], str(ctx.label)),
    ]

rego_library = rule(
    implementation = _rego_library_impl,
    doc = "Owns the Rego files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = [".rego"], doc = "Rego files of this package."),
        "data": attr.label_list(allow_files = [".json", ".yaml", ".yml"], doc = "Data files loaded with the Rego files; not checked by dx quality."),
        "deps": attr.label_list(providers = [RegoInfo], doc = "Rego libraries this package depends on."),
    },
)

def _runfiles_key(ctx, f):
    """Returns the runfiles key naming one file."""
    if f.short_path.startswith("../"):
        return f.short_path[3:]
    return ctx.workspace_name + "/" + f.short_path

def _staged_dest(f):
    """Returns the repository-relative path a file is staged at."""
    path = f.short_path
    if path.startswith("../"):
        parts = path[3:].split("/", 1)
        if len(parts) != 2:
            fail("rego_test: cannot stage " + path)
        path = parts[1]
    if "\t" in path or "\n" in path:
        fail("rego_test: unsupported characters in " + path)
    return path

def _rego_test_impl(ctx):
    if len(ctx.files.srcs) == 0:
        fail("rego_test: srcs must be non-empty")
    transitive_srcs = depset(
        ctx.files.srcs,
        transitive = [dep[RegoInfo].transitive_srcs for dep in ctx.attr.deps],
    )
    transitive_data = depset(
        ctx.files.data,
        transitive = [dep[RegoInfo].transitive_data for dep in ctx.attr.deps],
    )
    staged = {}
    for f in sorted(depset(transitive = [transitive_srcs, transitive_data]).to_list(), key = lambda f: f.short_path):
        dest = _staged_dest(f)
        if dest in staged and staged[dest] != f:
            fail("rego_test: {} and {} both stage as {}".format(staged[dest].path, f.path, dest))
        staged[dest] = f
    opa = ctx.file._opa
    lines = ["version=1", "opa=" + _runfiles_key(ctx, opa)]
    for dest in sorted(staged.keys()):
        lines.append("file=" + _runfiles_key(ctx, staged[dest]) + "\t" + dest)
    manifest = ctx.actions.declare_file(ctx.label.name + "_rego_test.manifest")
    ctx.actions.write(manifest, "\n".join(lines) + "\n")
    if ctx.target_platform_has_constraint(ctx.attr._windows_os[platform_common.ConstraintValueInfo]):
        executable = ctx.actions.declare_file(ctx.label.name + ".exe")
    else:
        executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = executable, target_file = ctx.executable._launcher, is_executable = True)
    runfiles = ctx.runfiles(files = staged.values() + [manifest, opa])
    runfiles = runfiles.merge(ctx.attr._launcher[DefaultInfo].default_runfiles)
    return [
        DefaultInfo(executable = executable, runfiles = runfiles),
        RunEnvironmentInfo({"DX_REGO_TEST_MANIFEST": _runfiles_key(ctx, manifest)}),
        OutputGroupInfo(rego_test_manifest = depset([manifest])),
        RegoInfo(transitive_srcs = transitive_srcs, transitive_data = transitive_data),
        dx_quality_sources(ctx.files.srcs, [("rego", ["rego"])], str(ctx.label)),
    ]

rego_test = rule(
    implementation = _rego_test_impl,
    doc = "Runs opa test over Rego files and their library dependencies.",
    test = True,
    attrs = {
        "srcs": attr.label_list(allow_files = [".rego"], mandatory = True, doc = "Rego test files."),
        "data": attr.label_list(allow_files = [".json", ".yaml", ".yml"], doc = "Data files loaded with the Rego files."),
        "deps": attr.label_list(providers = [RegoInfo], doc = "Rego libraries the tests import."),
        "_opa": attr.label(default = "//config:tool_opa", allow_single_file = True, cfg = "target"),
        "_launcher": attr.label(default = "//rego/launcher:opa_test_launcher", executable = True, cfg = "target"),
        "_windows_os": attr.label(default = "@platforms//os:windows"),
    },
)
