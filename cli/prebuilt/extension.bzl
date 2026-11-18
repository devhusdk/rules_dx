"""Creates @dx_prebuilt: the source dx when rules_dx is root, a pinned release binary otherwise."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")
load("//core:repo.bzl", "hub_repo")
load(":hub.bzl", "SOURCE_LABEL", "alias_build", "consumer_build", "plan_assets", "plan_root")
load(":pins.bzl", "DX_VERSION", "PINS", "URL_TEMPLATE")

# Loaded from outside rules_dx (consumer fixtures, generated BUILD files or docs).
visibility("public")

# The asset tag of every prebuilt hub: name is the tool, os and cpu the
# @platforms values the binary runs on.
ASSET_ATTRS = {
    "cpu": attr.string(doc = "The @platforms//cpu value the binary runs on.", mandatory = True),
    "name": attr.string(doc = "The tool, as named in cli/prebuilt/tools.bzl.", mandatory = True),
    "os": attr.string(doc = "The @platforms//os value the binary runs on.", mandatory = True),
    "sha256": attr.string(doc = "The binary's sha256.", mandatory = True),
    "url": attr.string(doc = "The binary's download URL.", mandatory = True),
}

_asset = tag_class(
    doc = "Adds or replaces dx or the launcher for one os and cpu; honoured only in the root module.",
    attrs = ASSET_ATTRS,
)

def declare_downloads(assets):
    """Declares one http_file per pinned download; platforms sharing a fallback share it."""
    seen = {}
    for asset in assets:
        if asset.repo and asset.repo not in seen:
            seen[asset.repo] = True
            http_file(
                name = asset.repo,
                downloaded_file_path = asset.exe,
                executable = True,
                sha256 = asset.sha256,
                urls = [asset.url],
            )

_from_source = tag_class(
    doc = "Builds dx and the launcher from source instead; honoured only in the root module.",
    attrs = {},
)

def _dx_prebuilt_impl(module_ctx):
    plan = plan_root(module_ctx.modules, tools = ["dx", "launcher"])
    if plan.error:
        fail(plan.error)
    rules_dx_is_root = any([m.name == "rules_dx" and m.is_root for m in module_ctx.modules])
    if plan.source:
        hub_repo(name = "dx_prebuilt", files = {"BUILD.bazel": alias_build(SOURCE_LABEL)})
    else:
        assets = plan_assets("dx", DX_VERSION, PINS, URL_TEMPLATE, plan.overrides.get("dx", {}))
        launcher_assets = plan_assets(
            "launcher",
            DX_VERSION,
            PINS,
            URL_TEMPLATE,
            plan.overrides.get("launcher", {}),
            exe = "dx-launcher",
            repo_prefix = "dx_launcher_prebuilt_",
        )
        declare_downloads(assets + launcher_assets)
        hub_repo(name = "dx_prebuilt", files = {"BUILD.bazel": consumer_build(assets, launcher_assets)})
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = ["dx_prebuilt"] if rules_dx_is_root else [],
        root_module_direct_dev_deps = [],
    )

dx_prebuilt = module_extension(
    doc = "Creates @dx_prebuilt, the dx binary behind @rules_dx//:dx and the native launcher.",
    implementation = _dx_prebuilt_impl,
    tag_classes = {
        "asset": _asset,
        "from_source": _from_source,
    },
)
