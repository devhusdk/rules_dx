"""Module extension for fortran/toolchain."""

load("//fortran/rules:fpm_lock.bzl", "fpm_lock_packages")
load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "FORTRAN_CELLS", "RULES_FORTRAN_COMMIT", "RULES_FORTRAN_SHA256", "RULES_FORTRAN_URL")
load(":repos.bzl", "DROPPED_PATHS", "FLANG_PREFIX", "FLANG_RT_NAME", "HUB_NAME", "flang_build", "flang_rt_repo", "fpm_build", "fpm_hub_build", "hub_build")

def _declare_fpm(mctx, tag):
    packages = fpm_lock_packages(mctx.read(tag.lock))
    for package in packages:
        pinned_archive(
            name = tag.name + "_" + package["name"],
            files = {"BUILD.bazel": fpm_build(package, tag.name)},
            sha256 = package["sha256"],
            strip_prefix = package["strip_prefix"],
            urls = [package["url"]],
        )
    hub_repo(name = tag.name, files = {"BUILD.bazel": fpm_hub_build(packages, tag.name)})

def _fortran_impl(mctx):
    pinned_archive(
        name = "rules_fortran",
        dropped = DROPPED_PATHS,
        patches = [
            Label("//patches/rules_fortran:module_map_key.patch"),
        ],
        sha256 = RULES_FORTRAN_SHA256,
        strip_prefix = "rules_fortran-" + RULES_FORTRAN_COMMIT,
        urls = [RULES_FORTRAN_URL],
    )
    for cell in sorted(FORTRAN_CELLS.keys()):
        pinned_archive(
            name = FLANG_PREFIX + cell,
            files = {"BUILD.bazel": flang_build(cell)},
            sha256 = FORTRAN_CELLS[cell].sha256,
            strip_prefix = FORTRAN_CELLS[cell].strip_prefix,
            urls = [FORTRAN_CELLS[cell].url],
        )
    flang_rt_repo(name = FLANG_RT_NAME, build_file = Label("//fortran/toolchain:flang_rt.BUILD.bazel"))
    hub_repo(name = HUB_NAME, files = hub_build())
    for module in mctx.modules:
        for tag in module.tags.fpm:
            _declare_fpm(mctx, tag)
    return mctx.extension_metadata(reproducible = True)


fortran = module_extension(
    implementation = _fortran_impl,
    doc = "Creates rules_fortran, the pinned Flang compilers, flang-rt and the toolchain hub.",
    tag_classes = {
        "fpm": tag_class(
            doc = "Declares a pinned archive per package of an fpm.lock and a hub repo with one fortran_library alias per package.",
            attrs = {
                "lock": attr.label(mandatory = True, allow_single_file = True, doc = "The fpm.lock to read."),
                "name": attr.string(mandatory = True, doc = "Hub repository name; package repos are named <name>_<package>."),
            },
        ),
    },
)
