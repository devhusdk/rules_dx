"""Creates @dx_prebuilt: the source dx, jars and Haskell tools when rules_dx is root, pinned release assets otherwise."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")
load("//core:repo.bzl", "hub_repo")
load(":hub.bzl", "SOURCE_LABEL", "alias_build", "consumer_build", "plan_assets", "plan_jars", "plan_native_tools", "plan_root", "with_jars", "with_native_tools")
load(":pins.bzl", "DX_VERSION", "PINS", "URL_TEMPLATE")
load(":tools.bzl", "JAR_TOOLS", "NATIVE_TOOLS")

# Loaded from outside rules_dx (consumer fixtures, generated BUILD files or docs).
visibility("public")

# The asset tag of every prebuilt hub: name is the tool, os and cpu the
# @platforms values the binary runs on, or "any" and "any" for a jar tool.
ASSET_ATTRS = {
    "cpu": attr.string(doc = "The @platforms//cpu value the binary runs on; \"any\" for a jar tool.", mandatory = True),
    "name": attr.string(doc = "The tool, as named in cli/prebuilt/tools.bzl.", mandatory = True),
    "os": attr.string(doc = "The @platforms//os value the binary runs on; \"any\" for a jar tool.", mandatory = True),
    "sha256": attr.string(doc = "The binary's sha256.", mandatory = True),
    "url": attr.string(doc = "The binary's download URL.", mandatory = True),
}

_asset = tag_class(
    doc = "Adds or replaces dx, the launcher, the Go lint tools, a native tool for one os and cpu, or a jar tool; honoured only in the root module.",
    attrs = ASSET_ATTRS,
)

def _declare_jars(jars):
    for jar in jars:
        if jar.repo:
            http_file(
                name = jar.repo,
                downloaded_file_path = jar.asset,
                sha256 = jar.sha256,
                urls = [jar.url],
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
                executable = asset.executable,
                sha256 = asset.sha256,
                urls = [asset.url],
            )

_from_source = tag_class(
    doc = "Builds dx, the launcher and the Go lint tools from source instead; honoured only in the root module.",
    attrs = {},
)

def _dx_prebuilt_impl(module_ctx):
    plan = plan_root(module_ctx.modules, tools = ["dx", "go"] + NATIVE_TOOLS + JAR_TOOLS)
    if plan.error:
        fail(plan.error)
    rules_dx_is_root = any([m.name == "rules_dx" and m.is_root for m in module_ctx.modules])
    if plan.source:
        jars = plan_jars(DX_VERSION, {}, URL_TEMPLATE)
        native_tools = plan_native_tools(DX_VERSION, {}, URL_TEMPLATE, {})
        hub_repo(name = "dx_prebuilt", files = {"BUILD.bazel": with_jars(with_native_tools(alias_build(SOURCE_LABEL), native_tools, consumer = not rules_dx_is_root), jars)})
    else:
        assets = plan_assets("dx", DX_VERSION, PINS, URL_TEMPLATE, plan.overrides.get("dx", {}))
        go_assets = plan_assets(
            "go",
            DX_VERSION,
            PINS,
            URL_TEMPLATE,
            plan.overrides.get("go", {}),
            exe = "dx-go",
            repo_prefix = "dx_go_prebuilt_",
        )
        jars = plan_jars(DX_VERSION, PINS, URL_TEMPLATE, plan.overrides)
        native_tools = plan_native_tools(DX_VERSION, PINS, URL_TEMPLATE, plan.overrides)
        declare_downloads(assets + go_assets + [a for tool in sorted(native_tools) for a in native_tools[tool]])
        _declare_jars(jars)
        build = with_native_tools(consumer_build(assets, go_assets), native_tools, consumer = True)
        hub_repo(name = "dx_prebuilt", files = {"BUILD.bazel": with_jars(build, jars)})
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
