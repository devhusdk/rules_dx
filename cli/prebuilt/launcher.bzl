"""Creates a rule set's native launcher hub: the pinned rules_dx launcher for each host, built from source where none is pinned."""

load("//core:repo.bzl", "hub_repo")
load(":extension.bzl", "declare_downloads")
load(":hub.bzl", "plan_assets", "with_native_tools")
load(":pins.bzl", "DX_VERSION", "PINS", "URL_TEMPLATE")

visibility("public")

FROM_SOURCE = tag_class(
    doc = "Builds the native launcher from source instead; honoured only in the root module.",
    attrs = {},
)

def launcher_hub(name, modules, ext, pins):
    """Returns struct(error, assets, build): the launcher downloads of hub name and its BUILD text."""
    root = False
    from_source = False
    for mod in modules:
        if mod.name == "rules_dx" and mod.is_root:
            root = True
        if mod.tags.from_source:
            if not mod.is_root:
                return struct(error = ext + ": only the root module may set from_source tags; {} sets them".format(mod.name), assets = [], build = "")
            from_source = True
    assets = plan_assets("launcher", DX_VERSION, {} if root or from_source else pins, URL_TEMPLATE, {}, exe = "launcher", repo_prefix = name + "_")
    build = with_native_tools("", {"launcher": assets}, consumer = not root, guard = root)
    return struct(error = "", assets = assets, build = build)

def native_launcher_repos(name, modules, ext):
    """Declares hub name, whose :launcher is the native launcher, and its pinned downloads name_<os>_<cpu>."""
    hub = launcher_hub(name, modules, ext, PINS)
    if hub.error:
        fail(hub.error)
    declare_downloads(hub.assets)
    hub_repo(name = name, files = {"BUILD.bazel": hub.build})
