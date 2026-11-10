"""The elixir module extension."""

load("//elixir/private:hex.bzl", "hex_package")
load("//elixir/private:mix_lock.bzl", "mix_lock_packages")
load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "ELIXIR_CELLS", "RULES_ELIXIR_COMMIT", "RULES_ELIXIR_DROPPED", "RULES_ELIXIR_SHA256", "RULES_ELIXIR_URL")
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
    pinned_archive(
        name = "rules_elixir",
        dropped = RULES_ELIXIR_DROPPED,
        patches = [
            # Applied in this order: the deps patch builds on the name patch.
            Label("//patches/rules_elixir:elixir_app_name.patch"),
            Label("//patches/rules_elixir:elixir_app_deps.patch"),
            Label("//patches/rules_elixir:warnings_as_errors.patch"),
            Label("//patches/rules_elixir:relocatable_tree.patch"),
            Label("//patches/rules_elixir:windows_ex_unit_test.patch"),
        ],
        sha256 = RULES_ELIXIR_SHA256,
        strip_prefix = "rules_elixir-" + RULES_ELIXIR_COMMIT,
        urls = [RULES_ELIXIR_URL],
    )
    dist_repo(name = DIST_NAME)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(ELIXIR_CELLS)})
    credo_repo(name = CREDO_NAME)
    for module in mctx.modules:
        for tag in module.tags.lock:
            _declare_mix(mctx, tag)
    return mctx.extension_metadata(reproducible = True)

elixir = module_extension(
    implementation = _elixir_impl,
    doc = "Creates rules_elixir, the pinned Elixir tree and the toolchain hub.",
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
