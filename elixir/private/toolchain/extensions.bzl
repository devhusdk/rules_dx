"""The elixir module extension."""

load("//core:repo.bzl", "hub_repo")
load("//elixir/private:hex.bzl", "hex_package")
load("//elixir/private:mix_lock.bzl", "mix_lock_packages")
load(":pins.bzl", "ELIXIR_CELLS")
load(":repos.bzl", "CREDO_NAME", "DIST_NAME", "HUB_NAME", "credo_repo", "dist_repo", "hub_build", "mix_build", "mix_hub_build")

def _declare_mix(mctx, tag):
    packages = mix_lock_packages(mctx.read(tag.lock))
    for package in packages:
        hex_package(
            name = tag.name + "_" + package["package"],
            build_file_content = mix_build(package, tag.name),
            package = package["package"],
            sha256 = package["sha256"],
            version = package["version"],
        )
    hub_repo(name = tag.name, files = {"BUILD.bazel": mix_hub_build(packages, tag.name)})

def _elixir_impl(mctx):
    dist_repo(name = DIST_NAME)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(ELIXIR_CELLS)})
    credo_repo(name = CREDO_NAME)
    return mctx.extension_metadata(reproducible = True)

elixir = module_extension(
    implementation = _elixir_impl,
    doc = "Creates the pinned Elixir tree and the toolchain hub.",
)

def _hex_impl(mctx):
    for module in mctx.modules:
        for tag in module.tags.lock:
            _declare_mix(mctx, tag)
    return mctx.extension_metadata(reproducible = True)

hex = module_extension(
    implementation = _hex_impl,
    doc = "Builds the Hex packages of mix.lock files.",
    tag_classes = {
        "lock": tag_class(
            doc = "Declares a hex_package per Hex entry of a mix.lock and a hub repo with one alias per package.",
            attrs = {
                "lock": attr.label(mandatory = True, allow_single_file = True, doc = "The mix.lock to read."),
                "name": attr.string(mandatory = True, doc = "Hub repository name; package repos are named <name>_<package>."),
            },
        ),
    },
)
