"""The ocaml module extension."""

load("//core:repo.bzl", "hub_repo", "pinned_archive")
load(":opam.bzl", "is_toolchain_package", "parse_depend_names", "parse_depends", "parse_url")
load(":pins.bzl", "DUNE_SHA256", "DUNE_URL", "DUNE_VERSION", "OCAML_CELLS", "OPAM_REPOSITORY_COMMIT", "OPAM_REPOSITORY_SHA256", "OPAM_REPOSITORY_URL", "RULES_OCAML_COMMIT", "RULES_OCAML_SHA256", "RULES_OCAML_URL")
load(":repos.bzl", "DIST_PREFIX", "DROPPED_PATHS", "DUNE_BUILD", "HUB_NAME", "dist_repo", "dune_package_build", "hub_build", "package_repo_name", "packages_hub_build")

def _fact_key(package, version):
    return "{}/{}.{}".format(OPAM_REPOSITORY_COMMIT, package, version)

def _snapshot_fact(mctx, package, version):
    path = "opam_snapshot/packages/{p}/{p}.{v}/opam".format(p = package, v = version)
    if not mctx.path(path).exists:
        fail("opam-repository {} has no {} {}".format(OPAM_REPOSITORY_COMMIT, package, version))
    opam = mctx.read(path)
    url = parse_url(opam)
    return {
        "needs": [n for n in parse_depend_names(opam) if not is_toolchain_package(n)],
        "integrity": url.integrity,
        "sha256": url.sha256,
        "src": url.src,
    }

def _declare_packages(mctx, tag, locked, annotations, facts):
    packages = {}
    for package, version in locked.items():
        if is_toolchain_package(package):
            continue
        fact = facts[_fact_key(package, version)]
        annotation = annotations.get(package)
        if annotation and annotation.build_file:
            build_file = mctx.read(annotation.build_file).replace("{hub}", tag.name)
        else:
            needs = fact["needs"]
            missing = [n for n in needs if n not in locked]
            if missing:
                fail("{} {} depends on {}, which {} does not lock".format(package, version, ", ".join(missing), tag.lock))
            library = annotation.library if annotation and annotation.library else package.replace("-", "_").replace(".", "_")
            build_file = dune_package_build(package, library, str(Label("//ocaml/toolchain:dune")), str(Label("//ocaml/rules:dune.bzl")), needs, tag.name)
        pinned_archive(
            name = package_repo_name(tag.name, package),
            files = {"BUILD.bazel": build_file},
            sha256 = fact["sha256"],
            integrity = fact.get("integrity", ""),
            strip_prefix = annotation.strip_prefix if annotation and annotation.strip_prefix else "{}-{}".format(package, version),
            urls = [fact["src"]],
        )
        packages[package] = version
    hub_repo(name = tag.name, files = {"BUILD.bazel": packages_hub_build(tag.name, packages)})

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
        sha256 = RULES_OCAML_SHA256,
        strip_prefix = "rules_ocaml-" + RULES_OCAML_COMMIT,
        urls = [RULES_OCAML_URL],
    )
    for cell in sorted(OCAML_CELLS.keys()):
        dist_repo(name = DIST_PREFIX + cell, cell = cell)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    pinned_archive(
        name = "dx_ocaml_dune",
        files = {"BUILD.bazel": DUNE_BUILD},
        sha256 = DUNE_SHA256,
        strip_prefix = "dune-" + DUNE_VERSION,
        urls = [DUNE_URL],
    )
    tags = [tag for module in mctx.modules for tag in module.tags.lock]
    locks = [parse_depends(mctx.read(tag.lock)) for tag in tags]
    wanted = {}
    for locked in locks:
        for package, version in locked.items():
            if not is_toolchain_package(package):
                wanted[_fact_key(package, version)] = (package, version)
    facts = {key: mctx.facts.get(key) for key in wanted}
    if None in facts.values():
        mctx.download_and_extract(
            url = [OPAM_REPOSITORY_URL],
            output = "opam_snapshot",
            sha256 = OPAM_REPOSITORY_SHA256,
            strip_prefix = "opam-repository-" + OPAM_REPOSITORY_COMMIT,
        )
        for key, (package, version) in wanted.items():
            if not facts[key]:
                facts[key] = _snapshot_fact(mctx, package, version)
    annotations = {tag.package: tag for module in mctx.modules for tag in module.tags.annotation}
    for tag, locked in zip(tags, locks):
        _declare_packages(mctx, tag, locked, annotations, facts)
    return mctx.extension_metadata(reproducible = True, facts = facts)

ocaml = module_extension(
    implementation = _ocaml_impl,
    doc = "Creates rules_ocaml, the pinned OCaml compilers, the toolchain hub and opam package hubs.",
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
            doc = "Creates a hub repository with one alias per package pinned in an opam lock file.",
            attrs = {
                "lock": attr.label(mandatory = True, allow_single_file = True, doc = "The `.opam.locked` file."),
                "name": attr.string(mandatory = True, doc = "Hub repository name."),
            },
        ),
    },
)
