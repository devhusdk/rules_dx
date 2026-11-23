"""The @dx_platforms hub: one platform and one config_setting per platform name."""

load(":names.bzl", "PLATFORMS", "STUB_MARKS")

visibility(["//platforms/..."])

_FAMILY = "@rules_dx//platforms:"

_PLATFORM = """
platform(
    name = "{name}",{parents}
    constraint_values = {constraints},
)
"""

_CONFIG = """
config_setting(
    name = "{name}_config",
    constraint_values = {constraints},
)
"""

def _list(items):
    return "[" + ", ".join([json.encode(item) for item in items]) + "]"

def hub_entries(registered, host):
    """Returns {name: struct(parent, platform, config)} for every platform name; registered maps a name to its tag."""
    entries = {}
    for name in sorted(PLATFORMS):
        info = PLATFORMS[name]
        tag = registered.get(name)
        if info.owner == "core":
            entries[name] = struct(parent = "", platform = info.constraints, config = info.constraints)
        elif tag == None:
            stub = info.constraints + [_FAMILY + STUB_MARKS[info.owner]]
            entries[name] = struct(parent = "", platform = stub, config = stub)
        else:
            mark = "family_ok" if host in tag["hosts"] else "needs_macos_host"
            own = [] if tag["parent"] else tag["constraint_values"]
            entries[name] = struct(
                parent = tag["parent"],
                platform = own + [_FAMILY + mark],
                config = tag["constraint_values"],
            )
    return entries

def render_hub(registered, host):
    """Returns the hub BUILD file text."""
    out = ['package(default_visibility = ["//visibility:public"])\n\nexports_files(["host.bzl"])\n']
    for name, entry in hub_entries(registered, host).items():
        parents = "\n    parents = [{}],".format(json.encode(entry.parent)) if entry.parent else ""
        out.append(_PLATFORM.format(name = name, parents = parents, constraints = _list(entry.platform)))
        out.append(_CONFIG.format(name = name, constraints = _list(entry.config)))
    return "".join(out)

def _hub_impl(rctx):
    host = rctx.attr.host
    registered = json.decode(rctx.attr.registered)
    rctx.file("BUILD.bazel", render_hub(registered, host))
    rctx.file("host.bzl", '"""The platform name of the host this hub was written on."""\n\nHOST = {}\n'.format(json.encode(host)))
    return rctx.repo_metadata(reproducible = True)

platforms_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Writes one platform and config_setting per platform name for the current host.",
    attrs = {
        "host": attr.string(doc = "Platform name of the host, or \"\" when the host has none."),
        "registered": attr.string(doc = "JSON map from a family platform name to its dx.platform tag."),
    },
)
