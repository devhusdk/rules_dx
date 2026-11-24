"""Serves the launchers of a downloaded tool bundle as executables."""

load("//core:rlocation.bzl", "rlocation_path")

visibility("public")

DxBundleInfo = provider(
    doc = "A bundle linked into one output directory, minus its bin/ launchers.",
    fields = {
        "files": "depset of the linked bundle files",
        "launchers": "dict of bin/ path to the downloaded launcher file",
    },
)

def _dx_bundle_tree_impl(ctx):
    links = []
    launchers = {}
    for src in ctx.files.srcs:
        path = src.owner.name
        if path.startswith("bin/"):
            launchers[path] = src
            continue
        link = ctx.actions.declare_file(ctx.label.name + "/" + path)
        ctx.actions.symlink(output = link, target_file = src)
        links.append(link)
    return [DxBundleInfo(files = depset(links), launchers = launchers)]

dx_bundle_tree = rule(
    implementation = _dx_bundle_tree_impl,
    doc = "Links every file of a bundle under a directory named after the target.",
    attrs = {
        "srcs": attr.label_list(doc = "The bundle's files, from the root package of its archive repo.", allow_files = True, mandatory = True),
    },
)

_WRAPPER = """#!/bin/sh
for root in "$0.runfiles" "${{RUNFILES_DIR:-}}"; do
  if [ -n "$root" ] && [ -x "$root/{launcher}" ]; then
    exec "$root/{launcher}" "$@"
  fi
done
echo "{label}: cannot find {launcher} in the runfiles of $0" >&2
exit 1
"""

def _dx_bundle_tool_impl(ctx):
    bundle = ctx.attr.tree[DxBundleInfo]
    launcher = bundle.launchers.get(ctx.attr.launcher)
    if launcher == None:
        fail("{}: the bundle has no {}; it has {}".format(ctx.label, ctx.attr.launcher, ", ".join(sorted(bundle.launchers.keys()))))

    link = ctx.actions.declare_file(ctx.attr.tree.label.name + "/" + ctx.attr.launcher)
    ctx.actions.symlink(output = link, target_file = launcher, is_executable = True)
    runfiles = ctx.runfiles(files = [link], transitive_files = bundle.files)
    if launcher.extension == "cmd":
        return [DefaultInfo(executable = link, files = depset([link]), runfiles = runfiles)]
    wrapper = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(wrapper, _WRAPPER.format(label = ctx.label, launcher = rlocation_path(ctx, link)), is_executable = True)
    return [DefaultInfo(executable = wrapper, files = depset([wrapper]), runfiles = runfiles.merge(ctx.runfiles(files = [wrapper])))]

dx_bundle_tool = rule(
    implementation = _dx_bundle_tool_impl,
    doc = "Runs one bin/ launcher of a dx_bundle_tree.",
    executable = True,
    attrs = {
        "launcher": attr.string(doc = "The launcher's path in the bundle, such as bin/yamllint.", mandatory = True),
        "tree": attr.label(doc = "The dx_bundle_tree the launcher runs from.", mandatory = True, providers = [DxBundleInfo]),
    },
)
