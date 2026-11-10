"""Creates @dx_gazelle: a pinned release gazelle, or one gazelle_binary of the registered languages plus dispatch.

The release gazelle holds the languages rules_dx registers. It builds from
source when rules_dx is root, when the root sets from_source, when another
module registers a language, or when the release has no pins.
"""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_file")
load("//cli/prebuilt:hub.bzl", "GAZELLE_ASSET_PLATFORMS", "plan_assets", "plan_root")
load("//cli/prebuilt:pins.bzl", "DX_VERSION", "GAZELLE_SHA256", "URL_TEMPLATE")
load("//core:repo.bzl", "hub_repo")
load(":hub.bzl", "builds_from_source", "prebuilt_build", "render_hub_build")

_language = tag_class(
    doc = "Adds a Gazelle language package to the dx gazelle binary. A module other than rules_dx that adds one makes gazelle build from source.",
    attrs = {
        "library": attr.label(doc = "The go_library of the language.", mandatory = True),
    },
)

_asset = tag_class(
    doc = "Adds or replaces one gazelle binary; honoured only in the root module.",
    attrs = {
        "cpu": attr.string(doc = "The @platforms//cpu value; defaults from a release asset name."),
        "name": attr.string(doc = "The release asset name, or a new name with os and cpu.", mandatory = True),
        "os": attr.string(doc = "The @platforms//os value; defaults from a release asset name."),
        "sha256": attr.string(doc = "The binary's sha256.", mandatory = True),
        "url": attr.string(doc = "The binary's download URL.", mandatory = True),
    },
)

_from_source = tag_class(
    doc = "Builds gazelle from source instead; honoured only in the root module.",
    attrs = {},
)

def _dx_gazelle_impl(module_ctx):
    plan = plan_root(module_ctx.modules, "dx_gazelle")
    if plan.error:
        fail(plan.error)
    labels = [tag.library for mod in module_ctx.modules for tag in mod.tags.language]
    assets = [] if builds_from_source(plan, module_ctx.modules) else plan_assets(
        DX_VERSION,
        GAZELLE_SHA256,
        URL_TEMPLATE,
        plan.overrides,
        platforms = GAZELLE_ASSET_PLATFORMS,
        exe = "gazelle",
        repo_prefix = "dx_gazelle_",
        strip = "dx-gazelle-",
    )
    if not [a for a in assets if a.repo]:
        hub_repo(name = "dx_gazelle", files = {"BUILD.bazel": render_hub_build(labels)})
    else:
        for asset in assets:
            if asset.repo:
                http_file(
                    name = asset.repo,
                    downloaded_file_path = asset.exe,
                    executable = True,
                    sha256 = asset.sha256,
                    urls = [asset.url],
                )
        hub_repo(name = "dx_gazelle", files = {"BUILD.bazel": prebuilt_build(assets, labels)})
    return module_ctx.extension_metadata(reproducible = True)

dx_gazelle = module_extension(
    doc = "Creates @dx_gazelle, the gazelle binary behind @rules_dx//dx:generate.",
    implementation = _dx_gazelle_impl,
    tag_classes = {
        "asset": _asset,
        "from_source": _from_source,
        "language": _language,
    },
)
