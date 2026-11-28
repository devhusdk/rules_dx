"""Creates @dx_prebuilt: every tool from source when rules_dx is root, pinned release assets otherwise."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive", "http_file")
load("//core:repo.bzl", "hub_repo")
load(":hub.bzl", "plan_hub", "plan_root")
load(":pins.bzl", "DX_VERSION", "PINS", "URL_TEMPLATE")
load(":tools.bzl", "JAR_TOOLS", "NATIVE_TOOLS", "PYTHON_BUNDLE_TOOLS")

visibility("public")

ASSET_ATTRS = {
    "cpu": attr.string(doc = "The @platforms//cpu value the binary runs on; \"any\" for a jar tool.", mandatory = True),
    "name": attr.string(doc = "The tool, as named in cli/prebuilt/tools.bzl.", mandatory = True),
    "os": attr.string(doc = "The @platforms//os value the binary runs on; \"any\" for a jar tool.", mandatory = True),
    "sha256": attr.string(doc = "The binary's sha256.", mandatory = True),
    "url": attr.string(doc = "The binary's download URL.", mandatory = True),
}

_asset = tag_class(
    doc = "Adds or replaces one prebuilt asset for one os and cpu; honoured only in the root module.",
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

_BUNDLE_BUILD = """filegroup(
    name = "files",
    srcs = glob(["**"], exclude = ["BUILD.bazel", "REPO.bazel", "WORKSPACE"]),
    visibility = ["//visibility:public"],
)
"""

def _declare_bundles(assets):
    seen = {}
    for asset in assets:
        if asset.repo and asset.repo not in seen:
            seen[asset.repo] = True
            http_archive(
                name = asset.repo,
                build_file_content = _BUNDLE_BUILD,
                sha256 = asset.sha256,
                type = "tar.gz",
                urls = [asset.url],
            )

_from_source = tag_class(
    doc = "Builds the listed tools, or every tool when tools is empty, from source instead of the release pins; honoured only in the root module.",
    attrs = {
        "tools": attr.string_list(doc = "The tools, as named in cli/prebuilt/tools.bzl; empty means every tool."),
    },
)

def _dx_prebuilt_impl(module_ctx):
    plan = plan_root(module_ctx.modules, tools = ["dx", "go"] + NATIVE_TOOLS + JAR_TOOLS + sorted(PYTHON_BUNDLE_TOOLS))
    if plan.error:
        fail(plan.error)
    hub = plan_hub(plan, DX_VERSION, PINS, URL_TEMPLATE)
    declare_downloads(hub.downloads)
    _declare_jars(hub.jars)
    _declare_bundles(hub.bundles)
    hub_repo(name = "dx_prebuilt", files = hub.files)
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = ["dx_prebuilt"] if plan.root else [],
        root_module_direct_dev_deps = [],
    )

dx_prebuilt = module_extension(
    doc = "Creates @dx_prebuilt, the dx binary behind @rules_dx//dx/launch:dx and the native launcher.",
    implementation = _dx_prebuilt_impl,
    tag_classes = {
        "asset": _asset,
        "from_source": _from_source,
    },
)
