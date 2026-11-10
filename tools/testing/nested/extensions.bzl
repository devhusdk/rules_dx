"""Module extension that declares the downloads nested Bazel tests read offline."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")

_DEFS = str(Label("//tools/testing/nested:defs.bzl"))

def _entries(mctx, file):
    entries = {}
    for line in mctx.read(file).splitlines():
        if not line or line.startswith("#"):
            continue
        parts = line.split(" ")
        sha256 = parts[0]
        valid = len(sha256) == 64 and all([c in "0123456789abcdef" for c in sha256.elems()])
        if not valid or len(parts) < 2:
            fail("%s: want '<sha256> <url>...', got %r" % (file, line))
        entries[sha256] = parts[1:]
    return entries

def _hub_impl(rctx):
    build = ['load("%s", "nested_repository_cache")\n' % _DEFS]
    for name, shas in rctx.attr.lists.items():
        build.append("nested_repository_cache(\n    name = %r,\n    testonly = True,\n    entries = {\n" % name)
        for sha256 in shas:
            build.append('        "@dx_nested_%s//file": "%s",\n' % (sha256, sha256))
        build.append('    },\n    visibility = ["//visibility:public"],\n)\n')
    rctx.file("BUILD.bazel", "".join(build))

_hub = repository_rule(
    implementation = _hub_impl,
    attrs = {"lists": attr.string_list_dict(mandatory = True)},
)

def _nested_cache_impl(mctx):
    """Declares one http_file per list entry and one manifest target per list."""
    urls = {}
    lists = {}
    for module in mctx.modules:
        for tag in module.tags.list:
            entries = _entries(mctx, tag.file)
            lists[tag.file.package + "/" + tag.file.name] = sorted(entries)
            urls.update(entries)
    for sha256, entry_urls in urls.items():
        http_file(
            name = "dx_nested_" + sha256,
            sha256 = sha256,
            urls = entry_urls,
        )
    _hub(name = "nested_repository_cache", lists = lists)
    return mctx.extension_metadata(reproducible = True)

nested_cache = module_extension(
    implementation = _nested_cache_impl,
    doc = "Creates @nested_repository_cache with one target per checked-in cache list.",
    tag_classes = {
        "list": tag_class(
            attrs = {"file": attr.label(mandatory = True, doc = "Cache list: one '<sha256> <url>...' line per download.")},
            doc = "Registers the cache list of one nested test.",
        ),
    },
)
