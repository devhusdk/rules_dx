"""A consumer repository that proves the supported surface, seeded from the public API snapshot, avoids dev-only repos."""

DEV_ONLY_REPOS = [
    "cosign",
    "dx_qt",
    "dx_yard",
    "firefox",
    "googletest",
    "rules_nodejs",
    "toolchains_llvm",
]

_LABEL_CHARS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-./:"

def _strings(text):
    return [part for i, part in enumerate(text.split('"')) if i % 2 == 1]

def _local_label(current, value):
    """Resolves a same-repository load string against the file that holds it."""
    if value.startswith("//"):
        return value[2:]
    if value.startswith(":"):
        return current.split(":")[0] + value
    return None

def _rules_dx_bzl(text):
    """Returns the `.bzl` files that `text` names as `@rules_dx//pkg:file.bzl`."""
    found = []
    for chunk in text.split("@rules_dx//")[1:]:
        end = len(chunk)
        for i in range(len(chunk)):
            if chunk[i] not in _LABEL_CHARS:
                end = i
                break
        label = chunk[:end]
        if ":" in label and label.endswith(".bzl"):
            found.append(label)
    return found

def _module_extensions(text):
    """Returns the rules_dx files behind each non-dev `use_extension` call."""
    found = []
    for call in text.split("use_extension(")[1:]:
        call = call.split(")")[0]
        if "dev_dependency = True" in call:
            continue
        values = _strings(call)
        if values and values[0].startswith("//"):
            found.append(values[0][2:])
    return found

def _exists(ctx, path):
    # A plain `exists` is not watched; watching makes adding or deleting the file rerun this rule.
    ctx.watch(path)
    return path.exists

def _user_docs(ctx, root):
    """Returns the README files and user doc pages, without the maintainer design records."""
    pages = []
    if _exists(ctx, root.get_child("README.md")):
        pages.append(root.get_child("README.md"))
    for entry in root.readdir():
        # bazel-* convenience symlinks mirror the source tree.
        if entry.basename.startswith(("bazel-", ".")) or not entry.is_dir:
            continue
        readme = entry.get_child("README.md")
        if _exists(ctx, readme):
            pages.append(readme)
    design = str(root.get_child("docs").get_child("design"))
    pending = [root.get_child("docs")]
    for _ in range(10000):
        if not pending:
            break
        current = pending.pop()
        for entry in current.readdir():
            if entry.is_dir:
                if str(entry) != design:
                    pending.append(entry)
            elif entry.basename.endswith(".md"):
                pages.append(entry)
    return pages

def _surface(ctx):
    """Lists the files a consumer can load: API snapshot loads, MODULE.bazel extensions, dx runtime loads and documented loads."""
    root = ctx.path(Label("//:MODULE.bazel")).dirname
    seeds = []
    seeds.extend(_module_extensions(ctx.read(ctx.attr.module)))
    for entry in json.decode(ctx.read(ctx.attr.starlark)):
        label = entry["load"]
        if not label.startswith("@rules_dx//"):
            fail("api snapshot load {} is not a @rules_dx// label".format(label))
        seeds.append(label.removeprefix("@rules_dx//"))
    languages = ctx.read(ctx.attr.languages)
    seeds.extend(_rules_dx_bzl(languages))

    seeds.extend([value for value in _strings(languages) if value.endswith(".bzl") and ":" in value and not value.startswith("@")])
    seeds.extend(_rules_dx_bzl(ctx.read(ctx.attr.generated)))
    for page in _user_docs(ctx, root):
        seeds.extend(_rules_dx_bzl(ctx.read(page)))
    return sorted({seed: True for seed in seeds}.keys())

def _consumer_surface_repo_impl(ctx):
    surface = _surface(ctx)
    pending = list(surface)
    seen = {}
    bad = []
    for _ in range(5000):
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
    if pending:
        fail("load closure did not finish; raise the walk limit")
    if bad:
        fail("supported files load dev-only repositories: " + ", ".join(bad))
    ctx.file("surface.txt", "\n".join(surface) + "\n")
    ctx.file("closure.txt", "\n".join(sorted(seen)) + "\n")
    ctx.file("BUILD.bazel", 'exports_files(["closure.txt", "surface.txt"], visibility = ["//visibility:public"])\n')
    return ctx.repo_metadata(reproducible = True)

consumer_surface_repo = repository_rule(
    implementation = _consumer_surface_repo_impl,
    attrs = {
        "generated": attr.label(doc = "BUILD file dx writes into a consumer.", allow_single_file = True, default = Label("//.dx:BUILD.bazel")),
        "languages": attr.label(doc = "Language manifest with hub extensions and aspect files.", allow_single_file = True, default = Label("//cli/families:languages.json")),
        "module": attr.label(doc = "Module file whose non-dev extensions consumers evaluate.", allow_single_file = True, default = Label("//:MODULE.bazel")),
        "starlark": attr.label(doc = "Public API snapshot whose load paths are the user entry points.", allow_single_file = True, default = Label("//api:starlark.json")),
    },
)
