"""A consumer repository that proves the supported surface avoids dev-only repos."""

# Repositories declared with dev_dependency = True in MODULE.bazel. A consumer
# module cannot see them, so no supported file may load from one.
DEV_ONLY_REPOS = [
    "cosign",
    "dx_rustdoc",
    "firefox",
    "googletest",
    "rules_nodejs",
    "toolchains_llvm",
]

def _strings(text):
    return [part for i, part in enumerate(text.split('"')) if i % 2 == 1]

def _local_label(current, value):
    """Resolves a same-repository load string against the file that holds it."""
    if value.startswith("//"):
        return value[2:]
    if value.startswith(":"):
        return current.split(":")[0] + value
    return None

def _consumer_surface_repo_impl(ctx):
    surface = ctx.read(ctx.attr.surface).splitlines()
    pending = [line for line in surface if line]
    seen = {}
    bad = []
    for _ in range(2000):
        if not pending:
            break
        current = pending.pop()
        if current in seen:
            continue
        seen[current] = True
        pkg, name = current.split(":")
        text = ctx.read(Label("//" + pkg + ":" + name))
        for value in _strings(text):
            if value.startswith("@"):
                repo = value[1:].split("//")[0].lstrip("@")
                if repo in DEV_ONLY_REPOS and value.endswith(".bzl"):
                    bad.append("{} loads {}".format(current, value))
            elif value.endswith(".bzl"):
                local = _local_label(current, value)
                if local:
                    pending.append(local)
    if bad:
        fail("supported files load dev-only repositories: " + ", ".join(bad))
    ctx.file("closure.txt", "\n".join(sorted(seen)) + "\n")
    ctx.file("BUILD.bazel", 'exports_files(["closure.txt"], visibility = ["//visibility:public"])\n')
    return ctx.repo_metadata(reproducible = True)

consumer_surface_repo = repository_rule(
    implementation = _consumer_surface_repo_impl,
    attrs = {"surface": attr.label(allow_single_file = True, default = Label("//cli/entrypoint:consumer_surface.txt"))},
)
