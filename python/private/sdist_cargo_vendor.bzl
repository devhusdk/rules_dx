"""Vendors the crates of a pinned Rust-backed sdist into an offline cargo source."""

_CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"

def _blocks(text):
    blocks = []
    for part in text.split("[[package]]")[1:]:
        fields = {}
        for line in part.split("\n"):
            if line.startswith("["):
                break
            if " = " in line:
                key, value = line.split(" = ", 1)
                fields[key.strip()] = value.strip()
        blocks.append(fields)
    return blocks

def _quoted(value):
    start = value.find("\"")
    if start < 0:
        return None
    end = value.find("\"", start + 1)
    return value[start + 1:end]

def _inline(value, key):
    marker = key + " = "
    start = value.find(marker)
    if start < 0:
        return None
    return _quoted(value[start + len(marker):])

def parse_uv_lock_sdist(text, package):
    """Returns {url, sha256} of the named package's sdist, or None."""
    for fields in _blocks(text):
        if _quoted(fields.get("name", "")) != package or "sdist" not in fields:
            continue
        url = _inline(fields["sdist"], "url")
        digest = _inline(fields["sdist"], "hash")
        if url == None or digest == None or not digest.startswith("sha256:"):
            return None
        return {"sha256": digest[len("sha256:"):], "url": url}
    return None

def parse_cargo_lock(text):
    """Returns [(name, version, checksum)] for registry crates, or an error string."""
    crates = []
    for fields in _blocks(text):
        if "source" not in fields:
            continue
        name = _quoted(fields.get("name", ""))
        source = _quoted(fields["source"])
        if source != _CRATES_IO:
            return "crate %s has unsupported source %s; only crates.io is vendored." % (name, source)
        checksum = _quoted(fields.get("checksum", ""))
        if checksum == None:
            return "crate %s has no checksum in Cargo.lock." % name
        crates.append((name, _quoted(fields.get("version", "")), checksum))
    return crates

_CONFIG = """[source.crates-io]
replace-with = "dx-vendored"

[source.dx-vendored]
directory = "vendor"

[net]
offline = true
"""

_BUILD = """load(":layer.bzl", "cargo_vendor_layer")

exports_files([".cargo/config.toml"], visibility = ["//visibility:public"])

cargo_vendor_layer(
    name = "layer",
    config = ".cargo/config.toml",
    vendor = glob(["vendor/**"]),
    visibility = ["//visibility:public"],
)
"""

_LAYER = """def _impl(ctx):
    return [
        DefaultInfo(files = depset([ctx.file.config] + ctx.files.vendor)),
        platform_common.TemplateVariableInfo({
            "DX_CARGO_CONFIG": ctx.file.config.path,
            "DX_CARGO_VENDOR": ctx.label.workspace_root + "/vendor",
        }),
    ]

cargo_vendor_layer = rule(
    implementation = _impl,
    attrs = {
        "config": attr.label(allow_single_file = True, mandatory = True),
        "vendor": attr.label_list(allow_files = True),
    },
)
"""

def _stem(url):
    name = url.rsplit("/", 1)[-1]
    for suffix in (".tar.gz", ".zip"):
        if name.endswith(suffix):
            return name[:-len(suffix)]
    return name

def _repo_impl(rctx):
    sdist = parse_uv_lock_sdist(rctx.read(rctx.attr.lock), rctx.attr.package)
    if sdist == None:
        fail("%s has no sdist in %s." % (rctx.attr.package, rctx.attr.lock))
    rctx.download_and_extract(url = sdist["url"], sha256 = sdist["sha256"], output = "sdist", strip_prefix = _stem(sdist["url"]))
    crates = parse_cargo_lock(rctx.read("sdist/" + rctx.attr.cargo_lock_path))
    if type(crates) == "string":
        fail(crates)
    for name, version, checksum in crates:
        rctx.download_and_extract(
            url = "https://static.crates.io/crates/%s/%s-%s.crate" % (name, name, version),
            sha256 = checksum,
            output = "vendor",
            type = "tar.gz",
        )
        rctx.file("vendor/%s-%s/.cargo-checksum.json" % (name, version), json.encode({"files": {}, "package": checksum}))
    rctx.file(".cargo/config.toml", _CONFIG)
    rctx.file("layer.bzl", _LAYER)
    rctx.file("BUILD.bazel", _BUILD)

sdist_cargo_vendor_repo = repository_rule(
    implementation = _repo_impl,
    attrs = {
        "cargo_lock_path": attr.string(doc = "Cargo.lock path inside the sdist.", default = "Cargo.lock"),
        "lock": attr.label(doc = "Lock file the vendored crates come from.", mandatory = True, allow_single_file = True),
        "package": attr.string(doc = "Name of the sdist package.", mandatory = True),
    },
)

_crates = tag_class(
    doc = "One Rust-backed sdist pinned in a uv.lock.",
    attrs = {
        "cargo_lock_path": attr.string(default = "Cargo.lock", doc = "Path of Cargo.lock inside the sdist."),
        "lock": attr.label(mandatory = True, allow_single_file = True, doc = "The uv.lock that pins the sdist."),
        "name": attr.string(mandatory = True, doc = "The vendor repository name."),
        "package": attr.string(mandatory = True, doc = "The package name in uv.lock."),
    },
)

def _ext_impl(module_ctx):
    for mod in module_ctx.modules:
        for tag in mod.tags.crates:
            sdist_cargo_vendor_repo(
                name = tag.name,
                lock = tag.lock,
                package = tag.package,
                cargo_lock_path = tag.cargo_lock_path,
            )

sdist_cargo_vendor = module_extension(
    doc = "Vendors a sdist's Cargo.lock crates so wheel builds never reach crates.io.",
    implementation = _ext_impl,
    tag_classes = {"crates": _crates},
)
