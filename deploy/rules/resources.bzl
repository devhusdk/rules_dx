"""Logical-path resource collections with optional declared processing."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo", "display_label")

DxResourcesInfo = provider(
    doc = "Staged resource identity for dx resource consumers.",
    fields = {
        "paths": "List of Strings: staged logical paths, sorted.",
        "processor": "String: processor label text or empty when staged directly.",
    },
)

def resource_path_error(path):
    """Validates one logical resource path, returning an error or empty."""
    if type(path) != "string" or path == "":
        return "must not be empty"
    if path.startswith("/"):
        return "must be relative"
    if "\\" in path:
        return "must use '/' separators"
    for segment in path.split("/"):
        if segment == "":
            return "must not contain empty segments"
        if segment == "." or segment == "..":
            return "must not contain '.' or '..'"
    return ""

def resource_mapping_error(paths):
    """Validates staged logical paths, returning a duplicate or collision error or empty."""
    seen = {}
    for path in paths:
        if path in seen:
            return "duplicate path '" + path + "'"
        seen[path] = True
    segmented = [(path, path.split("/")) for path in paths]
    for i in range(len(segmented)):
        for j in range(i + 1, len(segmented)):
            (a, parts_a) = segmented[i]
            (b, parts_b) = segmented[j]
            if len(parts_a) <= len(parts_b):
                (first, short, second, long) = (a, parts_a, b, parts_b)
            else:
                (first, short, second, long) = (b, parts_b, a, parts_a)
            if long[:len(short)] == short:
                return "path '" + first + "' collides with '" + second + "'"
    return ""

def _dx_resources_impl(ctx):
    """Stages one file per logical path, optionally through a declared processor."""
    entries = []
    for target in ctx.attr.mappings:
        logical = ctx.attr.mappings[target]
        path_error = resource_path_error(logical)
        if path_error != "":
            fail("dx_resources " + str(ctx.label) + ": invalid path '" + logical +
                 "' from " + display_label(target.label) + ": " + path_error)
        entries.append((target, logical))
    if len(entries) == 0:
        fail("dx_resources " + str(ctx.label) + ": mappings must not be empty")
    mapping_error = resource_mapping_error([entry[1] for entry in entries])
    if mapping_error != "":
        fail("dx_resources " + str(ctx.label) + ": " + mapping_error)
    by_path = {}
    for entry in entries:
        by_path[entry[1]] = entry[0]
    ordered = sorted(by_path.keys())
    processor = None
    processor_text = ""
    if ctx.attr.processor:
        exe = ctx.attr.processor[DefaultInfo].files_to_run.executable
        if exe == None:
            fail("dx_resources " + str(ctx.label) + ": processor " +
                 display_label(ctx.attr.processor.label) + " has no executable")
        processor = exe
        processor_text = display_label(ctx.attr.processor.label)
    staged = []
    sources = []
    for logical in ordered:
        target = by_path[logical]
        files = target[DefaultInfo].files.to_list()
        if len(files) == 0:
            fail("dx_resources " + str(ctx.label) + ": " + display_label(target.label) +
                 " provides no files")
        if len(files) != 1:
            fail("dx_resources " + str(ctx.label) + ": " + display_label(target.label) +
                 " must provide exactly one file")
        src = files[0]
        sources.append(src)
        out = ctx.actions.declare_file(ctx.label.name + "/" + logical)
        if processor == None:
            ctx.actions.symlink(output = out, target_file = src)
        else:
            tool_runfiles = ctx.attr.processor[DefaultInfo].default_runfiles.files
            ctx.actions.run(
                executable = processor,
                inputs = depset([src], transitive = [tool_runfiles]),
                outputs = [out],
                arguments = ctx.attr.processor_args + [src.path, out.path],
                mnemonic = "DxResources",
                progress_message = "Dx stage resource %{label}",
            )
        staged.append(out)
    return [
        DefaultInfo(
            files = depset(staged),
            runfiles = ctx.runfiles(files = staged + sources),
        ),
        DxResourcesInfo(paths = ordered, processor = processor_text),
        DxSubjectInfo(fields = {
            "count": str(len(ordered)),
            "paths": ",".join(ordered),
            "processor": processor_text,
        }),
    ]

dx_resources = rule(
    implementation = _dx_resources_impl,
    attrs = {
        "mappings": attr.label_keyed_string_dict(
            allow_files = True,
        ),
        "processor": attr.label(
            cfg = "exec",
            executable = True,
        ),
        "processor_args": attr.string_list(),
    },
)
