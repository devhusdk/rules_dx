"""Creates @dx_prebuilt: the source dx when rules_dx is root, a pinned release binary otherwise.

A consumer platform whose release asset has no pinned sha256 falls back to the source dx.
"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")
load(":hub.bzl", "SOURCE_LABEL", "alias_build", "consumer_build", "plan_assets", "valid_sha256")
load(":pins.bzl", "DX_VERSION", "SHA256", "URL_TEMPLATE")

def _hub_impl(rctx):
    rctx.file("BUILD.bazel", rctx.attr.build)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Writes the hub BUILD file the extension rendered.",
    attrs = {"build": attr.string(doc = "The BUILD file content.", mandatory = True)},
)

_asset = tag_class(
    doc = "Adds or replaces one dx binary; honoured only in the root module.",
    attrs = {
        "cpu": attr.string(doc = "The @platforms//cpu value; defaults from a release asset name."),
        "name": attr.string(doc = "The release asset name, or a new name with os and cpu.", mandatory = True),
        "os": attr.string(doc = "The @platforms//os value; defaults from a release asset name."),
        "sha256": attr.string(doc = "The binary's sha256.", mandatory = True),
        "url": attr.string(doc = "The binary's download URL.", mandatory = True),
    },
)

_from_source = tag_class(
    doc = "Builds dx from source instead; honoured only in the root module.",
    attrs = {},
)

def _dx_prebuilt_impl(module_ctx):
    rules_dx_is_root = False
    from_source = False
    overrides = {}
    for mod in module_ctx.modules:
        if mod.name == "rules_dx" and mod.is_root:
            rules_dx_is_root = True
        if not mod.is_root:
            if mod.tags.asset or mod.tags.from_source:
                fail("dx_prebuilt: only the root module may set asset or from_source tags; {} sets them".format(mod.name))
            continue
        if mod.tags.from_source:
            from_source = True
        for tag in mod.tags.asset:
            if not valid_sha256(tag.sha256):
                fail("dx_prebuilt.asset(name = \"{}\"): sha256 must be 64 lowercase hex digits".format(tag.name))
            overrides[tag.name] = struct(cpu = tag.cpu, os = tag.os, sha256 = tag.sha256, url = tag.url)
    if rules_dx_is_root or from_source:
        _hub(name = "dx_prebuilt", build = alias_build(SOURCE_LABEL))
    else:
        assets = plan_assets(DX_VERSION, SHA256, URL_TEMPLATE, overrides)
        for asset in assets:
            if asset.repo:
                http_file(
                    name = asset.repo,
                    downloaded_file_path = asset.exe,
                    executable = True,
                    sha256 = asset.sha256,
                    urls = [asset.url],
                )
        _hub(name = "dx_prebuilt", build = consumer_build(assets))
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = ["dx_prebuilt"] if rules_dx_is_root else [],
        root_module_direct_dev_deps = [],
    )

dx_prebuilt = module_extension(
    doc = "Creates @dx_prebuilt, the dx binary behind @rules_dx//:dx.",
    implementation = _dx_prebuilt_impl,
    tag_classes = {
        "asset": _asset,
        "from_source": _from_source,
    },
)
