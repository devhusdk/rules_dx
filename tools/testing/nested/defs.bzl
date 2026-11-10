"""The one way to declare a test that runs a nested Bazel."""

load("//rust:defs.bzl", "rust_test")

_BAZEL = str(Label("//tools/testing:nested_bazel_bin"))
_SHIM = str(Label("//tools/testing:nested_bazel_shim"))
_CACHE_REPO = "@nested_repository_cache//:"

NESTED_TAGS = [
    "resources:cpu:2",
    "resources:memory:2560",
    "resources:nested_bazel:1",
]

def _rlocation(ctx, file):
    if file.short_path.startswith("../"):
        return file.short_path[3:]
    return ctx.workspace_name + "/" + file.short_path

def _nested_repository_cache_impl(ctx):
    lines = []
    files = []
    for target, sha256 in ctx.attr.entries.items():
        found = target.files.to_list()
        if len(found) != 1:
            fail("%s must be one file" % target.label)
        files.append(found[0])
        lines.append("%s %s\n" % (sha256, _rlocation(ctx, found[0])))
    manifest = ctx.actions.declare_file(ctx.label.name + ".manifest")
    ctx.actions.write(manifest, "".join(sorted(lines)))
    return [DefaultInfo(
        files = depset([manifest]),
        runfiles = ctx.runfiles(files = files + [manifest]),
    )]

nested_repository_cache = rule(
    implementation = _nested_repository_cache_impl,
    doc = "Carries downloaded files and a manifest of sha256 to runfile for a nested Bazel repository cache.",
    attrs = {
        "entries": attr.label_keyed_string_dict(
            allow_files = True,
            doc = "Downloaded files mapped to their sha256.",
        ),
    },
)

def _nested_update_impl(ctx):
    test = ctx.attr.test
    suffix = ".exe" if ctx.executable.test.extension == "exe" else ""
    exe = ctx.actions.declare_file(ctx.label.name + suffix)
    ctx.actions.symlink(output = exe, target_file = ctx.executable.test, is_executable = True)
    env = dict(test[RunEnvironmentInfo].environment)
    env.pop("DX_NESTED_CACHE", None)
    env["DX_NESTED_UPDATE"] = ctx.attr.cache
    return [
        DefaultInfo(executable = exe, runfiles = ctx.runfiles().merge(test[DefaultInfo].default_runfiles)),
        RunEnvironmentInfo(environment = env),
    ]

nested_update = rule(
    implementation = _nested_update_impl,
    doc = "Runs a nested test online and rewrites its cache list and consumer locks in the source tree.",
    attrs = {
        "cache": attr.string(mandatory = True, doc = "Workspace-relative path of the cache list."),
        "test": attr.label(mandatory = True, cfg = "target", executable = True, doc = "The nested test."),
    },
    executable = True,
)

def nested_bazel_test(name, *, cache = None, data = None, env = None, tags = None, **kwargs):
    """Declares a rust_test that runs the pinned Bazel offline in the test sandbox.

    Args:
        name: Test name. `<name>.update` regenerates `cache` and the staged locks under `bazel run`.
        cache: Checked-in cache list in this package, registered with `nested_cache.list` in MODULE.bazel.
        data: Runfiles of the test.
        env: Environment of the test.
        tags: Tags added to the shared resource tags.
        **kwargs: Passed to rust_test.
    """
    data = (data or []) + [_BAZEL, _SHIM]
    env = {
        "DX_NESTED_BAZEL_PINNED": "$(rlocationpath %s)" % _BAZEL,
        "DX_NESTED_BAZEL_SHIM": "$(rlocationpath %s)" % _SHIM,
        "RUST_TEST_THREADS": "1",
    } | (env or {})
    path = "%s/%s" % (native.package_name(), cache)
    rust_test(
        name = name,
        size = kwargs.pop("size", "large"),
        timeout = kwargs.pop("timeout", "moderate"),
        data = data + ([_CACHE_REPO + path] if cache else []),
        env = env | ({"DX_NESTED_CACHE": "$(rlocationpath %s%s)" % (_CACHE_REPO, path)} if cache else {}),
        tags = NESTED_TAGS + (tags or []),
        **kwargs
    )
    if cache:
        nested_update(
            name = name + ".update",
            testonly = True,
            cache = path,
            test = ":" + name,
        )
