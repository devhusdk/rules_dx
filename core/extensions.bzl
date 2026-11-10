"""The dx_pins module extension."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load(":repo.bzl", "hub_repo", "pinned_archive")
load(":toolchain.bzl", "cell_archive_rule", "cell_repo", "host_archive_rule", "pins_hub")

def _dx_pins_impl(mctx):
    archives = {}
    data = {}
    sources = {}
    for mod in mctx.modules:
        for tag in mod.tags.data:
            sources[tag.name] = json.decode(mctx.read(tag.pins))
    for mod in mctx.modules:
        for tag in mod.tags.pin:
            pins = json.decode(mctx.read(tag.pins))
            data[tag.name] = pins
            if tag.build == None:
                continue
            build = mctx.read(tag.build)
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
        for tag in mod.tags.archive:
            pin = json.decode(mctx.read(tag.pins))["archives"][tag.key or tag.name]
            archives[tag.name] = pin
            pinned_archive(
                name = tag.name,
                crlf = tag.crlf,
                dropped = tag.dropped,
                files = {"BUILD.bazel": mctx.read(tag.build)} if tag.build else {},
                patches = tag.patches,
                required = tag.required,
                sha256 = pin["sha256"],
                strip_prefix = pin.get("strip_prefix", ""),
                urls = pin["urls"],
            )
    hub_repo(name = "dx_pins_data", files = {"BUILD.bazel": "", "pins.bzl": "PINS = " + repr(data) + "\n\nDATA = " + repr(sources) + "\n\nARCHIVES = " + repr(archives) + "\n"})
    return mctx.extension_metadata(reproducible = True)

dx_pins = module_extension(
    implementation = _dx_pins_impl,
    doc = "Creates the repositories of every pins.json: one per cell and a toolchain hub, one host-selected repository, or one patched source archive.",
    tag_classes = {
        "archive": tag_class(
            doc = "One source archive from an archives entry of a pins.json, pruned and patched. The entry is exported as ARCHIVES[name] from @dx_pins_data.",
            attrs = {
                "build": attr.label(allow_single_file = True, doc = "BUILD file written at the repository root."),
                "crlf": attr.string_list(doc = "Paths rewritten with CRLF line endings after patching."),
                "dropped": attr.string_list(doc = "Paths deleted after extraction."),
                "key": attr.string(doc = "Entry under archives; defaults to name."),
                "name": attr.string(mandatory = True, doc = "Repository name."),
                "patches": attr.label_list(doc = "Patches applied in order with strip 1."),
                "pins": attr.label(mandatory = True, allow_single_file = True, doc = "JSON file with an archives object."),
                "required": attr.string_list(doc = "Paths the archive must ship."),
            },
        ),
        "data": tag_class(
            doc = "One pins.json read at load time only: no repository is made and the content is exported as DATA[name] from @dx_pins_data.",
            attrs = {
                "name": attr.string(mandatory = True, doc = "Key of the content in DATA."),
                "pins": attr.label(mandatory = True, allow_single_file = True, doc = "JSON file of version, cells and archives."),
            },
        ),
        "pin": tag_class(
            doc = "One pins.json. With a toolchain_type it makes a repository per cell and a hub of toolchain() targets; without, one repository of the host cell.",
            attrs = {
                "aliases": attr.string_dict(doc = "Host cell to the pinned cell that serves it."),
                "build": attr.label(allow_single_file = True, doc = "BUILD file template; {binary} is replaced by the cell's binary path. Unset records the pins in dx_pins_data only."),
                "compatible_with": attr.string(default = "exec", values = ["exec", "target"], doc = "Which toolchain() constraint attribute the cell platform fills."),
                "name": attr.string(mandatory = True, doc = "Hub, or sole, repository name."),
                "pins": attr.label(mandatory = True, allow_single_file = True, doc = "JSON file of version and cells."),
                "toolchain_type": attr.label(doc = "Toolchain type the hub registers; unset for a host-selected repository."),
            },
        ),
    },
)

def _dx_source_archives_impl(mctx):
    for mod in mctx.modules:
        for tag in mod.tags.archive:
            pin = json.decode(mctx.read(tag.pins))[tag.name]
            http_archive(
                name = tag.name,
                build_file = tag.build_file,
                patch_args = tag.patch_args,
                patch_cmds = tag.patch_cmds,
                patches = tag.patches,
                sha256 = pin["sha256"],
                strip_prefix = pin["strip_prefix"],
                type = tag.archive_type,
                urls = pin["urls"],
            )
    return mctx.extension_metadata(reproducible = True)

dx_source_archives = module_extension(
    implementation = _dx_source_archives_impl,
    doc = "Creates one http_archive per tag from the entry of the same name in a source pins.json.",
    tag_classes = {
        "archive": tag_class(
            doc = "One http_archive whose sha256, strip_prefix and urls come from the pins.json entry named like the repository.",
            attrs = {
                "archive_type": attr.string(doc = "Archive type override for urls without a recognised extension."),
                "build_file": attr.label(allow_single_file = True, doc = "BUILD file overlaid on the archive."),
                "name": attr.string(mandatory = True, doc = "Repository name and pins.json key."),
                "patch_args": attr.string_list(doc = "Arguments to the patch tool."),
                "patch_cmds": attr.string_list(doc = "Shell commands run after patching."),
                "patches": attr.label_list(doc = "Patch files applied to the archive."),
                "pins": attr.label(mandatory = True, allow_single_file = True, doc = "JSON file keyed by repository name."),
            },
        ),
    },
)
