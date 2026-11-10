"""The dx_pins module extension."""

load(":repo.bzl", "hub_repo")
load(":toolchain.bzl", "cell_archive_rule", "cell_repo", "host_archive_rule", "pins_hub")

def _dx_pins_impl(mctx):
    data = {}
    for mod in mctx.modules:
        for tag in mod.tags.pin:
            pins = json.decode(mctx.read(tag.pins))
            build = mctx.read(tag.build)
            data[tag.name] = pins
            if tag.toolchain_type == None:
                host_archive_rule(
                    name = tag.name,
                    aliases = tag.aliases,
                    build = build,
                    cells = json.encode(pins["cells"]),
                    tool = tag.name,
                )
                continue
            for cell, pin in pins["cells"].items():
                cell_archive_rule(name = cell_repo(tag.name, cell), build = build, pin = json.encode(pin), tool = tag.name)
            hub_repo(name = tag.name, files = {"BUILD.bazel": pins_hub(tag.name, pins["cells"].keys(), str(tag.toolchain_type), tag.compatible_with)})
    hub_repo(name = "dx_pins_data", files = {"BUILD.bazel": "", "pins.bzl": "PINS = " + repr(data) + "\n"})
    return mctx.extension_metadata(reproducible = True)

dx_pins = module_extension(
    implementation = _dx_pins_impl,
    doc = "Creates the repositories of every pins.json: one per cell and a toolchain hub, or one host-selected repository.",
    tag_classes = {
        "pin": tag_class(
            doc = "One pins.json. With a toolchain_type it makes a repository per cell and a hub of toolchain() targets; without, one repository of the host cell.",
            attrs = {
                "aliases": attr.string_dict(doc = "Host cell to the pinned cell that serves it."),
                "build": attr.label(mandatory = True, allow_single_file = True, doc = "BUILD file template; {binary} is replaced by the cell's binary path."),
                "compatible_with": attr.string(default = "exec", values = ["exec", "target"], doc = "Which toolchain() constraint attribute the cell platform fills."),
                "name": attr.string(mandatory = True, doc = "Hub, or sole, repository name."),
                "pins": attr.label(mandatory = True, allow_single_file = True, doc = "JSON file of version and cells."),
                "toolchain_type": attr.label(doc = "Toolchain type the hub registers; unset for a host-selected repository."),
            },
        ),
    },
)
