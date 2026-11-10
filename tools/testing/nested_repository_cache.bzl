"""Repository cache sets for nested Bazel tests."""

def _nested_repository_cache_impl(ctx):
    """Writes the manifest of sha256 to runfile and carries every file it names."""
    lines = []
    files = []
    for target, sha256 in ctx.attr.entries.items():
        srcs = target.files.to_list()
        if len(srcs) != 1:
            fail("%s must name exactly one file for %s" % (target.label, sha256))
        files.append(srcs[0])
        lines.append("%s %s\n" % (sha256, _rlocationpath(ctx, srcs[0])))
    out = ctx.actions.declare_file(ctx.label.name + ".manifest")
    ctx.actions.write(out, "".join(sorted(lines)))
    return [DefaultInfo(files = depset([out]), runfiles = ctx.runfiles(files = [out] + files))]

def _rlocationpath(ctx, file):
    if file.short_path.startswith("../"):
        return file.short_path[len("../"):]
    return ctx.workspace_name + "/" + file.short_path

nested_repository_cache = rule(
    implementation = _nested_repository_cache_impl,
    doc = "One repository cache set of a nested Bazel test: a manifest of sha256 to runfile.",
    attrs = {
        "entries": attr.label_keyed_string_dict(
            allow_files = True,
            mandatory = True,
            doc = "Downloaded files mapped to their sha256.",
        ),
    },
)

def _nested_repository_cache_set_impl(rctx):
    entries = json.decode(rctx.attr.entries)
    lines = ["load(%r, \"nested_repository_cache\")\n\n" % str(rctx.attr.rule_bzl), "nested_repository_cache(\n    name = \"cache\",\n    testonly = True,\n    entries = {\n"]
    for sha256 in sorted(entries):
        lines.append("        \"@%s//file\": \"%s\",\n" % (entries[sha256], sha256))
    lines.append("    },\n    visibility = [\"//visibility:public\"],\n)\n")
    rctx.file("BUILD.bazel", "".join(lines))

nested_repository_cache_set = repository_rule(
    implementation = _nested_repository_cache_set_impl,
    doc = "Declares the `cache` target of one repository cache set.",
    attrs = {
        "entries": attr.string(mandatory = True, doc = "JSON object of sha256 to the repository that holds the file."),
        "rule_bzl": attr.label(mandatory = True, doc = "The .bzl file that defines nested_repository_cache."),
    },
)

def parse_cache_list(text, origin):
    """Parses a cache list: one `<url> <sha256>` per line, `#` comments."""
    entries = {}
    for number, line in enumerate(text.splitlines(), 1):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(" ")
        if len(parts) != 2:
            fail("%s:%d: want `<url> <sha256>`, got %r" % (origin, number, line))
        url, sha256 = parts
        if len(sha256) != 64 or not all([c in "0123456789abcdef" for c in sha256.elems()]):
            fail("%s:%d: bad sha256 %r" % (origin, number, sha256))
        if not url.startswith("https://"):
            fail("%s:%d: want an https URL, got %r" % (origin, number, url))
        entries.setdefault(sha256, url)
    return entries
