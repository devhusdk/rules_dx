r"""Platform-correct launchers for the generated JavaScript tool binaries.

`rules_nodejs` writes its Windows launcher as a batch file that hands the tool's script to
`C:/WINDOWS/system32/bash.exe`. That is WSL's bash, which cannot read a `C:/...` path, so
every runfile lookup inside the script fails on a Windows host. Git's `sh` reads those paths
and ships with every Windows image that has a toolchain, and this repository already relies
on it for `starlark_test`.

The generated script is what runs on every host. Only the wrapper differs, and only where the
generated launcher cannot be used at all.
"""

load("@bazel_lib//lib:directory_path.bzl", "DirectoryPathInfo")
load("//quality/artifacts:platforms.bzl", "execution_is_windows")

_NODE_OPTIONS = ["--preserve-symlinks-main"]

def _execution_is_windows(ctx):
    """Reports whether the wrapped tool executes on Windows."""
    return execution_is_windows(ctx, ctx.attr._windows_os[platform_common.ConstraintValueInfo])

def _generated_script(executable):
    """Returns the script the generated launcher runs.

    `rules_nodejs` keeps the shell script beside the batch file under the same name, so the
    script is the executable without its suffix, and both are in the runfiles. Where there is
    no suffix the executable already is the script.
    """
    short_path = executable.short_path
    for suffix in (".bat", ".exe", ".cmd"):
        if short_path.endswith(suffix):
            return short_path[:-len(suffix)]
    return short_path

def _entry_point_path(entry_point):
    """Returns the normalized runfiles-relative path of the node entry point."""
    if DirectoryPathInfo in entry_point:
        info = entry_point[DirectoryPathInfo]
        path = info.directory.short_path + "/" + info.path
    else:
        files = entry_point[DefaultInfo].files.to_list()
        if len(files) != 1:
            fail("js_tool_binary entry_point must be a single file or provide DirectoryPathInfo")
        path = files[0].short_path
    return path.replace("/./", "/")

def _key(ctx, path):
    """Returns the runfiles key naming one short path."""
    if path.startswith("../"):
        return path.removeprefix("../")
    return (ctx.workspace_name + "/" + path).replace("\\", "/")

def _one_key(ctx, files, matches, what):
    """Returns the runfiles key of the single file matching a predicate."""
    found = []
    for f in files:
        if matches(f):
            key = _key(ctx, f.short_path)
            if key not in found:
                found.append(key)
    if len(found) != 1:
        fail("js_tool_binary " + ctx.label.name + ": want one " + what + ", have " + str(len(found)))
    return found[0]

def _node_key(ctx, target):
    """Returns the runfiles key naming the node runtime."""
    files = target[DefaultInfo].default_runfiles.files.to_list()
    return _one_key(ctx, files, lambda f: f.basename == "node.exe", "node runtime")

def _patches_key(ctx, target):
    """Returns the runfiles key naming the node bootstrap patches."""
    files = target[DefaultInfo].default_runfiles.files.to_list()
    return _one_key(ctx, files, lambda f: f.basename == "bootstrap.cjs", "node bootstrap")

def _wrapper_key(ctx, target):
    """Returns the runfiles key naming the node wrapper."""
    files = target[DefaultInfo].default_runfiles.files.to_list()
    return _one_key(ctx, files, lambda f: f.short_path.replace("\\", "/").endswith("node_bin/node"), "node wrapper")

def _js_tool_binary_impl(ctx):
    target = ctx.attr.js_binary[DefaultInfo]
    generated = target.files_to_run
    if not _execution_is_windows(ctx):
        script = _generated_script(generated.executable)
        link = ctx.actions.declare_symlink(ctx.label.name + ".sh")
        ctx.actions.symlink(
            output = link,
            target_path = script.split(ctx.label.package + "/", 1)[-1],
        )
        return [DefaultInfo(
            executable = link,
            files = depset([link, generated.executable]),
            runfiles = target.default_runfiles,
        )]

    entry = _entry_point_path(ctx.attr.entry_point)
    lines = [
        "entry=" + _key(ctx, entry),
        "node=" + _node_key(ctx, ctx.attr.js_binary),
        "require=" + _patches_key(ctx, ctx.attr.js_binary),
        "wrapper=" + _wrapper_key(ctx, ctx.attr.js_binary),
    ]
    lines.extend(["node_option=" + option for option in _NODE_OPTIONS])
    descriptor = ctx.actions.declare_file(ctx.label.name + ".launcher.txt")
    ctx.actions.write(descriptor, "\n".join(lines) + "\n")
    launcher = ctx.attr._windows_launcher[DefaultInfo].files_to_run.executable
    link = ctx.actions.declare_file(ctx.label.name + ".exe")
    ctx.actions.symlink(output = link, target_file = launcher, is_executable = True)
    return [DefaultInfo(
        executable = link,
        runfiles = target.default_runfiles.merge(ctx.runfiles(files = [descriptor])),
    )]

js_tool_binary = rule(
    doc = "Runs a generated JavaScript tool binary on every host.",
    implementation = _js_tool_binary_impl,
    attrs = {
        "entry_point": attr.label(
            allow_files = True,
            doc = "Single file or DirectoryPathInfo naming the node entry point.",
            mandatory = True,
        ),
        "js_binary": attr.label(
            cfg = "exec",
            doc = "Generated JavaScript tool binary to wrap.",
            mandatory = True,
            allow_files = True,
        ),
        "_windows_launcher": attr.label(
            cfg = "exec",
            default = Label("//quality/tools/javascript/launcher:js_launcher"),
            doc = "Launcher run on Windows hosts.",
            executable = True,
        ),
        "_windows_os": attr.label(
            default = "@platforms//os:windows",
            doc = "Constraint marking Windows hosts.",
        ),
    },
    executable = True,
)

def js_tool(name, js_binary, entry_point, **kwargs):
    """Declares one platform-correct wrapper around a generated JavaScript tool."""
    js_tool_binary(
        name = name,
        entry_point = entry_point,
        js_binary = js_binary,
        **kwargs
    )
