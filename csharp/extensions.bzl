"""Public C# module extensions."""

load("//core:pins.bzl", "lock_tag")
load("//csharp/private/lock:hub.bzl", "lock_hubs")
load("//dotnet:extensions.bzl", "dotnet_nuget_repo")

def _nuget_impl(mctx):
    direct = []
    dev = []
    seen = {}
    for module in mctx.modules:
        for tag in module.tags.lock:
            if tag.name in seen:
                fail("nuget hub %r is declared by both %s and %s" % (tag.name, seen[tag.name], module.name))
            seen[tag.name] = module.name
            for name, packages in sorted(lock_hubs(tag.name, json.decode(mctx.read(tag.lock))).items()):
                dotnet_nuget_repo(name = name, packages = packages)
                if module.is_root:
                    if mctx.is_dev_dependency(tag):
                        dev.append(name)
                    else:
                        direct.append(name)
    return mctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = direct,
        root_module_direct_dev_deps = dev,
    )

nuget = module_extension(
    doc = "Builds one rules_dotnet NuGet hub per Paket group of each lock tag: `<name>` for main, `<name>.<group>` for the others.",
    implementation = _nuget_impl,
    tag_classes = {"lock": lock_tag},
)
