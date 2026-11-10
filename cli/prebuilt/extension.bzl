"""Creates @dx_prebuilt: the source dx when rules_dx is root, a pinned release binary otherwise.

A consumer platform whose release asset has no pinned sha256 falls back to the source dx.
"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")
load("//core:repo.bzl", "hub_repo")
load(":hub.bzl", "SOURCE_LABEL", "alias_build", "consumer_build", "plan_assets", "plan_root")
load(":pins.bzl", "DX_VERSION", "SHA256", "URL_TEMPLATE")

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
    plan = plan_root(module_ctx.modules)
    if plan.error:
        fail(plan.error)
    rules_dx_is_root = any([m.name == "rules_dx" and m.is_root for m in module_ctx.modules])
    if plan.source:
        hub_repo(name = "dx_prebuilt", files = {"BUILD.bazel": alias_build(SOURCE_LABEL)})
    else:
        assets = plan_assets(DX_VERSION, SHA256, URL_TEMPLATE, plan.overrides)
        for asset in assets:
            if asset.repo:
                http_file(
                    name = asset.repo,
                    downloaded_file_path = asset.exe,
                    executable = True,
                    sha256 = asset.sha256,
                    urls = [asset.url],
                )
        hub_repo(name = "dx_prebuilt", files = {"BUILD.bazel": consumer_build(assets)})
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
