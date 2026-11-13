"""The ocaml module extension."""

load("//core:pins.bzl", "lock_problems", "lock_repo")
load("//core:repo.bzl", "hub_repo", "pinned_archive")
load(":opam.bzl", "pick_annotation")
load(":pins.bzl", "DUNE", "OCAML_CELLS", "OPAM_REPOSITORY", "RULES_OCAML")
load(":repos.bzl", "DIST_PREFIX", "DROPPED_PATHS", "DUNE_BUILD", "HUB_NAME", "dist_repo", "dune_package_build", "hub_build")

def _package_build(mctx, tag, package, annotation):
    name = package["name"]
    if annotation and annotation.build_file:
        return mctx.read(annotation.build_file).replace("{hub}", tag.name)
    unpinned = package.get("extra", {}).get("opam_unpinned", [])
    if unpinned:
        fail("{} {} depends on {}, which {} does not lock".format(name, package["version"], ", ".join(unpinned), tag.lock))
    library = annotation.library if annotation and annotation.library else package["provides"][0]
    return dune_package_build(name, library, str(Label("//ocaml/private/toolchain:dune")), str(Label("//ocaml/private:dune.bzl")), package.get("deps", []))

def _declare_packages(mctx, tag, annotations):
    text = mctx.read(tag.lock)
    lock = json.decode(text)
    problems = lock_problems(lock)
    if problems:
        fail("%s %s" % (tag.lock, "; ".join(problems)))
    builds = {}
    for package in lock["packages"]:
        annotation = pick_annotation(package["name"], annotations)
        if annotation and annotation.strip_prefix:
            for archive in package["archives"]:
                archive["strip_prefix"] = annotation.strip_prefix
        builds[package["name"]] = _package_build(mctx, tag, package, annotation)
    lock_repo(name = tag.name, lock = json.encode(lock), builds = builds)

def _ocaml_impl(mctx):
    pinned_archive(
        name = "rules_ocaml",
        dropped = DROPPED_PATHS,
        patches = [
            Label("//patches/rules_ocaml:hermetic_ocaml.patch"),
            Label("//patches/rules_ocaml:native_cc_env.patch"),
            Label("//patches/rules_ocaml:platform_alias.patch"),
            Label("//patches/rules_ocaml:static_cc_deps.patch"),
        ],
        sha256 = RULES_OCAML.sha256,
        strip_prefix = RULES_OCAML.strip_prefix,
        urls = RULES_OCAML.urls,
    )
    for cell in sorted(OCAML_CELLS.keys()):
        dist_repo(name = DIST_PREFIX + cell, cell = cell)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    pinned_archive(
        name = "dx_ocaml_dune",
        files = {"BUILD.bazel": DUNE_BUILD},
        sha256 = DUNE.sha256,
        strip_prefix = DUNE.strip_prefix,
        urls = DUNE.urls,
    )
    pinned_archive(
        name = "dx_opam_repository",
        files = {"BUILD.bazel": 'exports_files(["repo"])'},
        sha256 = OPAM_REPOSITORY.sha256,
        strip_prefix = OPAM_REPOSITORY.strip_prefix,
        urls = OPAM_REPOSITORY.urls,
    )
    return mctx.extension_metadata(reproducible = True)

ocaml = module_extension(
    implementation = _ocaml_impl,
    doc = "Creates rules_ocaml, the pinned OCaml compilers and the toolchain hub.",
)

def _opam_impl(mctx):
    annotations = {tag.package: tag for module in mctx.modules for tag in module.tags.annotation}
    for module in mctx.modules:
        for tag in module.tags.lock:
            _declare_packages(mctx, tag, annotations)
    return mctx.extension_metadata(reproducible = True)

opam = module_extension(
    implementation = _opam_impl,
    doc = "Creates opam package hubs.",
    tag_classes = {
        "annotation": tag_class(
            doc = "Adjusts how one opam package is built; without a build_file it is built with dune.",
            attrs = {
                "build_file": attr.label(allow_single_file = True, doc = "BUILD file placed in the package repository, for a package not built with dune. `{hub}` in it becomes the name of the hub the package is declared for."),
                "library": attr.string(doc = "The dune library name when it differs from the package name with - and . as _."),
                "package": attr.string(mandatory = True, doc = "Opam package name."),
                "strip_prefix": attr.string(doc = "Archive directory prefix to strip."),
            },
        ),
        "lock": tag_class(
            doc = "Creates a hub repository with one package per entry of an opam lock.",
            attrs = {
                "lock": attr.label(mandatory = True, allow_single_file = [".json"], doc = "The lock file the opam lock step writes."),
                "name": attr.string(mandatory = True, doc = "Hub repository name."),
            },
        ),
    },
)
