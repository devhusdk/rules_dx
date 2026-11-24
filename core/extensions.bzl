"""The core module extensions: dx_pins, dx_licenses, dx_prebuilt and dx_gazelle."""

load("//cli/prebuilt:extension.bzl", _dx_prebuilt = "dx_prebuilt")
load("//cli/prebuilt:gazelle.bzl", _dx_gazelle = "dx_gazelle")
load(":license.bzl", "LICENSE_IDS", "TERMS_LABEL", "accepted_terms", "terms_errors", "unknown_ids")
load(":repo.bzl", "ARCHIVE_TYPES", "hub_repo", "pinned_archive")
load(":root_only.bzl", "warn_dropped_tags")
load(":toolchain.bzl", "cell_archive_rule", "cell_repo", "host_archive_rule", "pins_hub")

_FILE_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "file",
    srcs = ["{path}"],
)
"""

_JAR_BUILD = """load("@rules_java//java:defs.bzl", "java_import")

package(default_visibility = ["//visibility:public"])

java_import(
    name = "jar",
    jars = ["downloaded.jar"],
)
"""

def _versioned(name, build, pin):
    if "{version}" not in build:
        return build
    if "version" not in pin:
        fail("{}: the BUILD file names {{version}} but the pin has no version".format(name))
    return build.replace("{version}", pin["version"])

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
            hub_repo(name = tag.name, files = {"BUILD.bazel": pins_hub(tag.name, pins["cells"].keys(), str(tag.toolchain_type), tag.compatible_with, tag.exec_aliases)})
        for tag in mod.tags.archive:
            pin = json.decode(mctx.read(tag.pins))["archives"][tag.key or tag.name]
            archives[tag.name] = pin
            kind = pin.get("type", "")
            files = {"BUILD.bazel": _versioned(tag.name, mctx.read(tag.build), pin)} if tag.build else {}
            for path, label in tag.files.items():
                files[path] = mctx.read(label)
            download_as = ""
            archive_type = ""
            if kind == "file":
                download_as = "file/" + pin.get("path", "downloaded")
                files["file/BUILD.bazel"] = _FILE_BUILD.format(path = pin.get("path", "downloaded"))
            elif kind == "jar":
                download_as = "jar/downloaded.jar"
                files["jar/BUILD.bazel"] = _JAR_BUILD
            elif kind in ARCHIVE_TYPES:
                archive_type = kind
            elif kind:
                fail("{}: type '{}' is not file, jar or an archive type".format(tag.name, kind))
            pinned_archive(
                name = tag.name,
                bzl_srcs = tag.bzl_srcs,
                crlf = tag.crlf,
                download_as = download_as,
                dropped = tag.dropped,
                executable = pin.get("executable", False),
                files = files,
                patches = tag.patches,
                required = tag.required,
                sha256 = pin["sha256"],
                strip_prefix = pin.get("strip_prefix", ""),
                type = archive_type,
                urls = pin["urls"],
            )
    hub_repo(name = "dx_pins_data", files = {"BUILD.bazel": "exports_files([\"pins.bzl\"])\n", "pins.bzl": "PINS = " + repr(data) + "\n\nDATA = " + repr(sources) + "\n\nARCHIVES = " + repr(archives) + "\n"})
    return mctx.extension_metadata(reproducible = True)

dx_pins = module_extension(
    implementation = _dx_pins_impl,
    doc = "Creates the repositories of every pins.json: one per cell and a toolchain hub, one host-selected repository, or one patched source archive.",
    tag_classes = {
        "archive": tag_class(
            doc = "One source archive from an archives entry of a pins.json, pruned and patched. The entry is exported as ARCHIVES[name] from @dx_pins_data.",
            attrs = {
                "build": attr.label(allow_single_file = True, doc = "BUILD file written at the repository root; {version} is replaced by the entry's version."),
                "bzl_srcs": attr.bool(doc = "Adds @<name>//:dx_bzl_srcs, a public filegroup of every Starlark file, for starlark_doc_extract deps."),
                "crlf": attr.string_list(doc = "Paths rewritten with CRLF line endings after patching."),
                "dropped": attr.string_list(doc = "Paths deleted after extraction."),
                "files": attr.string_keyed_label_dict(allow_files = True, doc = "Repository path to a file written after pruning, before patching."),
                "key": attr.string(doc = "Entry under archives; defaults to name."),
                "name": attr.string(mandatory = True, doc = "Repository name."),
                "patches": attr.label_list(doc = "Patches applied in order with strip 1."),
                "pins": attr.label(mandatory = True, allow_single_file = True, doc = "JSON file with an archives object; an entry with type file or jar is downloaded without extraction, any other type names the archive format."),
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
                "compatible_with": attr.string(default = "exec", values = ["both", "exec", "target"], doc = "Which toolchain() constraint attributes the cell platform fills."),
                "exec_aliases": attr.string_list(doc = "Targets of each cell repository the hub exposes as aliases selecting the exec platform's cell."),
                "name": attr.string(mandatory = True, doc = "Hub, or sole, repository name."),
                "pins": attr.label(mandatory = True, allow_single_file = True, doc = "JSON file of version and cells."),
                "toolchain_type": attr.label(doc = "Toolchain type the hub registers; unset for a host-selected repository."),
            },
        ),
    },
)

def _dx_licenses_impl(mctx):
    ids = []
    tags = []
    warn_dropped_tags("dx_licenses", mctx.modules, ["accept"])
    for mod in mctx.modules:
        if not mod.is_root:
            continue
        for tag in mod.tags.accept:
            unknown = unknown_ids(tag.ids)
            if unknown:
                fail("dx_licenses: unknown license id " + ", ".join(unknown) + "; the ids are " + ", ".join(LICENSE_IDS))
            ids.extend([name for name in tag.ids if name not in ids])
            tags.append(tag)
    terms = json.decode(mctx.read(Label(TERMS_LABEL)))
    errors = terms_errors(terms, tags)
    if errors:
        fail("\n".join(errors))
    hub_repo(name = "dx_licenses", files = {"BUILD.bazel": "", "accepted.json": json.encode(accepted_terms(terms, ids))})
    return mctx.extension_metadata(reproducible = True)

dx_licenses = module_extension(
    implementation = _dx_licenses_impl,
    doc = "Accepts licenses for restricted downloads. Only root-module tags count.",
    tag_classes = {
        "accept": tag_class(
            doc = "Accepts the listed license ids.",
            attrs = {
                "ids": attr.string_list(mandatory = True, doc = "License ids: " + ", ".join(LICENSE_IDS) + "."),
                "terms_sha256": attr.string_dict(doc = "The terms hash each id was accepted at, as `dx init --accept-license` writes it. A hash that no longer matches the license's current terms fails with the tag to re-accept."),
            },
        ),
    },
)

def _dx_toolchains_impl(mctx):
    files = {"BUILD.bazel": ""}
    for mod in mctx.modules:
        for tag in mod.tags.package:
            if mod.name != "rules_dx":
                fail("dx_toolchains: {} sets package {}; only rules_dx declares the packages of @dx_toolchains".format(mod.name, tag.name))
            path = tag.name + "/BUILD.bazel"
            if path in files:
                fail("dx_toolchains: package {} is declared twice".format(tag.name))
            files[path] = mctx.read(tag.build)
    hub_repo(name = "dx_toolchains", files = files)
    return mctx.extension_metadata(reproducible = True)

# One hub for every toolchain() rules_dx declares itself. Registering a hub
# fetches it and the load closure of its extension, and dx_pins already loads
# this file, so these toolchains cost every consumer one repository and no
# ruleset (cuda/extensions.bzl, for one, loads rules_cuda).
dx_toolchains = module_extension(
    implementation = _dx_toolchains_impl,
    doc = "Declares the `dx_toolchains` hub that rules_dx registers for consumers: one package per `package` tag.",
    tag_classes = {
        "package": tag_class(
            doc = "One package of the hub, registered as `@dx_toolchains//<name>:all`.",
            attrs = {
                "build": attr.label(mandatory = True, allow_single_file = True, doc = "BUILD file of the package; its labels resolve as in rules_dx."),
                "name": attr.string(mandatory = True, doc = "Package name."),
            },
        ),
    },
)

dx_prebuilt = _dx_prebuilt

dx_gazelle = _dx_gazelle
