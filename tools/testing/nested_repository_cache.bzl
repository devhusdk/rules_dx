"""Repository cache sets for nested Bazel tests."""

def _nested_repository_cache_impl(ctx):
    """Writes the manifest of hex digest to runfile and URLs and carries every file it names."""
    lines = []
    files = []
    for target, sha256 in ctx.attr.entries.items():
        srcs = target.files.to_list()
        if len(srcs) != 1:
            fail("%s must name exactly one file for %s" % (target.label, sha256))
        files.append(srcs[0])
        urls = ctx.attr.urls.get(sha256, [])
        lines.append(" ".join([sha256, _rlocationpath(ctx, srcs[0])] + urls) + "\n")
    out = ctx.actions.declare_file(ctx.label.name + ".manifest")
    ctx.actions.write(out, "".join(sorted(lines)))
    return [DefaultInfo(files = depset([out]), runfiles = ctx.runfiles(files = [out] + files))]

def _rlocationpath(ctx, file):
    if file.short_path.startswith("../"):
        return file.short_path[len("../"):]
    return ctx.workspace_name + "/" + file.short_path

nested_repository_cache = rule(
    implementation = _nested_repository_cache_impl,
    doc = "One repository cache set of a nested Bazel test: a manifest of hex digest to runfile and URLs.",
    attrs = {
        "entries": attr.label_keyed_string_dict(
            allow_files = True,
            mandatory = True,
            doc = "Downloaded files mapped to their hex digest.",
        ),
        "urls": attr.string_list_dict(
            doc = "Hex digest to the URLs the download is fetched from, each a canonical id of the cache entry, and `id-<hex>` canonical id hashes.",
        ),
    },
)

def _nested_repository_cache_set_impl(rctx):
    entries = json.decode(rctx.attr.entries)
    urls = json.decode(rctx.attr.urls)
    lines = ["load(%r, \"nested_repository_cache\")\n\n" % str(rctx.attr.rule_bzl), "nested_repository_cache(\n    name = \"cache\",\n    testonly = True,\n    entries = {\n"]
    for sha256 in sorted(entries):
        lines.append("        \"@%s//file\": \"%s\",\n" % (entries[sha256], sha256))
    lines.append("    },\n    urls = {\n")
    for sha256 in sorted(urls):
        lines.append("        \"%s\": %s,\n" % (sha256, json.encode(urls[sha256])))
    lines.append("    },\n    visibility = [\"//visibility:public\"],\n)\n")
    rctx.file("BUILD.bazel", "".join(lines))

nested_repository_cache_set = repository_rule(
    implementation = _nested_repository_cache_set_impl,
    doc = "Declares the `cache` target of one repository cache set.",
    attrs = {
        "entries": attr.string(mandatory = True, doc = "JSON object of hex digest to the repository that holds the file."),
        "urls": attr.string(mandatory = True, doc = "JSON object of hex digest to the URLs of the download."),
        "rule_bzl": attr.label(mandatory = True, doc = "The .bzl file that defines nested_repository_cache."),
    },
)

_HEX = "0123456789abcdef"
_BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
_ALGORITHMS = {64: "sha256", 96: "sha384", 128: "sha512"}

def digest_algorithm(digest):
    """The checksum algorithm of a lowercase hex digest, or None."""
    if not all([c in _HEX for c in digest.elems()]):
        return None
    return _ALGORITHMS.get(len(digest))

def digest_integrity(digest):
    """The Subresource Integrity string of a hex digest."""
    data = [int(digest[i:i + 2], 16) for i in range(0, len(digest), 2)]
    encoded = []
    for i in range(0, len(data), 3):
        chunk = data[i:i + 3]
        bits = 0
        for byte in chunk + [0] * (3 - len(chunk)):
            bits = bits * 256 + byte
        for n, shift in enumerate((18, 12, 6, 0)):
            encoded.append(_BASE64[(bits >> shift) & 63] if n <= len(chunk) else "=")
    return "%s-%s" % (digest_algorithm(digest), "".join(encoded))

def parse_cache_list(text, origin):
    """Parses `<url> <hex digest>` or `id-<hex> <hex digest>` lines into digest to URLs and canonical id hashes."""
    entries = {}
    for number, line in enumerate(text.splitlines(), 1):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(" ")
        if len(parts) != 2:
            fail("%s:%d: want `<url> <digest>`, got %r" % (origin, number, line))
        url, digest = parts
        algorithm = digest_algorithm(digest)
        if not algorithm:
            fail("%s:%d: bad digest %r" % (origin, number, digest))
        if url.startswith("id-"):
            if digest_algorithm(url[len("id-"):]) != algorithm:
                fail("%s:%d: want `id-<%s hex>`, got %r" % (origin, number, algorithm, url))
        elif not url.startswith("https://"):
            fail("%s:%d: want an https URL, got %r" % (origin, number, url))
        entries.setdefault(digest, []).append(url)
    for digest, urls in entries.items():
        if not [url for url in urls if url.startswith("https://")]:
            fail("%s: %s has no https URL" % (origin, digest))
    return entries
