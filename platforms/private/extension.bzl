"""The dx_platforms module extension: families register the platform names they build for."""

load(":hub.bzl", "platforms_hub")
load(":names.bzl", "PLATFORMS")

visibility(["//platforms/..."])

def registration_error(tag_name, hosts, owner_module, registered):
    """Returns why a dx.platform tag cannot be accepted, or "" when it can."""
    for host in hosts:
        if host not in PLATFORMS or PLATFORMS[host].owner != "core":
            return "dx.platform: {} lists host {}, which is not a core platform name".format(tag_name, host)
    if tag_name not in PLATFORMS:
        return "dx.platform: unknown platform {}".format(tag_name)
    if PLATFORMS[tag_name].owner == "core":
        return "dx.platform: {} is a core platform; only family platforms are registered".format(tag_name)
    if tag_name in registered:
        return "dx.platform: {} is registered by both {} and {}".format(tag_name, registered[tag_name]["module"], owner_module)
    return ""

def _dx_platforms_impl(mctx):
    registered = {}
    for mod in mctx.modules:
        for tag in mod.tags.platform:
            error = registration_error(tag.name, tag.hosts, mod.name, registered)
            if error:
                fail(error)
            registered[tag.name] = {
                "constraint_values": [str(label) for label in tag.constraint_values],
                "hosts": tag.hosts,
                "module": mod.name,
                "parent": str(tag.parent) if tag.parent else "",
            }
    platforms_hub(name = "dx_platforms", registered = json.encode(registered))
    return mctx.extension_metadata(reproducible = True)

_platform = tag_class(
    doc = "Registers one family platform name.",
    attrs = {
        "constraint_values": attr.label_list(doc = "Constraints that identify the platform in platform_select."),
        "hosts": attr.string_list(doc = "Host platform names that can build for it."),
        "name": attr.string(mandatory = True, doc = "Platform name from @rules_dx//platforms."),
        "parent": attr.label(doc = "Family platform the hub platform inherits from."),
    },
)

dx_platforms = module_extension(
    implementation = _dx_platforms_impl,
    doc = "Writes the @dx_platforms hub from the family platform registrations.",
    tag_classes = {"platform": _platform},
)
