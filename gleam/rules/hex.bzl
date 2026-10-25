"""Pinned Gleam package repositories from repo.hex.pm."""

HEX_REPO = "https://repo.hex.pm/tarballs/"

DEFS = str(Label("//gleam/rules:defs.bzl"))

def hex_tarball_url(package, version):
    """Returns the repo.hex.pm tarball URL for one package release."""
    return HEX_REPO + package + "-" + version + ".tar"

def hex_metadata_entries(key, value, listed = False):
    """Returns the metadata.config terms that declare one key and value."""
    term = "[{}]" if listed else "{}"
    return ['{<<"' + key + '">>, ' + term.format('<<"' + value + suffix + ">>") + "}." for suffix in ['"', '"/utf8']]

def declares(metadata, key, value, listed = False):
    """Reports whether metadata.config declares one key and value."""
    return len([entry for entry in hex_metadata_entries(key, value, listed) if entry in metadata]) > 0

def generated_erlang_module(path):
    """Returns the Erlang module name Gleam generates for one src/ .gleam path."""
    return path.removeprefix("src/").removesuffix(".gleam").replace("/", "@")

def ffi_sources(paths):
    """Returns the hand-written .erl sources, dropping Erlang that Gleam generated."""
    generated = [generated_erlang_module(p) for p in paths if p.endswith(".gleam")]
    return sorted([p for p in paths if p.endswith(".erl") and p.split("/")[-1].removesuffix(".erl") not in generated])

def js_label(label):
    """Returns the JavaScript target label that pairs with one Erlang dep label."""
    if ":" in label:
        return label + "_js"
    target = label.split("/")[-1].removeprefix("@")
    if label.startswith("@") and "//" not in label:
        return "{}//:{}_js".format(label, target)
    return "{}:{}_js".format(label, target)

def package_build(package, gleam_srcs, erl_srcs, mjs_srcs, deps):
    """Returns the BUILD file for one Gleam package release, for Erlang and JavaScript."""
    lines = [
        'load("{}", "GLEAM_COMPATIBLE_WITH", "gleam_erl_library", "gleam_js_library", "gleam_library")'.format(DEFS),
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
    ]
    lib_deps = list(deps)
    if erl_srcs:
        lines.extend([
            "gleam_erl_library(",
            '    name = "{}_ffi",'.format(package),
            "    srcs = {},".format(repr(erl_srcs)),
            "    target_compatible_with = GLEAM_COMPATIBLE_WITH,",
            "    warnings_as_errors = False,",
            ")",
            "",
        ])
        lib_deps = [":{}_ffi".format(package)] + lib_deps
    lines.extend([
        "gleam_library(",
        '    name = "{}",'.format(package),
        "    srcs = {},".format(repr(gleam_srcs)),
        "    deps = {},".format(repr(lib_deps)),
        "    target_compatible_with = GLEAM_COMPATIBLE_WITH,",
        "    warnings_as_errors = False,",
        ")",
        "",
        "gleam_js_library(",
        '    name = "{}_js",'.format(package),
        '    package = "{}",'.format(package),
        "    srcs = {},".format(repr(sorted(gleam_srcs + mjs_srcs))),
        "    deps = {},".format(repr([js_label(dep) for dep in deps])),
        "    target_compatible_with = GLEAM_COMPATIBLE_WITH,",
        "    warnings_as_errors = False,",
        ")",
    ])
    return "\n".join(lines) + "\n"

def _walk(root, prefix):
    out = []
    pending = [(root, prefix)]
    for _ in range(10000):
        if not pending:
            return out
        path, base = pending.pop()
        for child in path.readdir():
            if child.is_dir:
                pending.append((child, base + child.basename + "/"))
            else:
                out.append(base + child.basename)
    fail("too many directories below " + str(root))

def _gleam_hex_package_impl(rctx):
    rctx.download_and_extract(
        url = hex_tarball_url(rctx.attr.package, rctx.attr.version),
        sha256 = rctx.attr.sha256,
        type = "tar",
        output = "_hex",
    )
    metadata = rctx.read("_hex/metadata.config")
    for key, value in [("name", rctx.attr.package), ("version", rctx.attr.version)]:
        if not declares(metadata, key, value):
            fail("hex package {} {} does not declare {} {}".format(rctx.attr.package, rctx.attr.version, key, value))
    if not declares(metadata, "build_tools", "gleam", listed = True):
        fail("hex package {} {} is not built with gleam".format(rctx.attr.package, rctx.attr.version))
    rctx.extract("_hex/contents.tar.gz")
    rctx.delete("_hex")
    srcs = _walk(rctx.path("src"), "src/")
    gleam_srcs = sorted([p for p in srcs if p.endswith(".gleam")])
    erl_srcs = ffi_sources(srcs)
    for path in srcs:
        if path.endswith(".erl") and path not in erl_srcs:
            rctx.delete(path)
    rctx.file("REPO.bazel", "")
    mjs_srcs = sorted([p for p in srcs if p.endswith(".mjs")])
    rctx.file("BUILD.bazel", package_build(rctx.attr.package, gleam_srcs, erl_srcs, mjs_srcs, rctx.attr.deps))
    return rctx.repo_metadata(reproducible = True)

gleam_hex_package = repository_rule(
    implementation = _gleam_hex_package_impl,
    doc = "Fetches one pinned Gleam package release from repo.hex.pm.",
    attrs = {
        "deps": attr.string_list(doc = "Labels of the Gleam packages this release requires."),
        "package": attr.string(mandatory = True, doc = "Hex package name."),
        "sha256": attr.string(mandatory = True, doc = "Digest of the release tarball."),
        "version": attr.string(mandatory = True, doc = "Exact release version."),
    },
)
