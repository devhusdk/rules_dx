"""Module extension for erlang/toolchain."""

load("//elixir/rules:hex.bzl", "hex_package")
load("//erlang/rules:rebar_lock.bzl", "rebar_lock_packages")
load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "CELLS", "HEX_SOURCES", "RULES_ERLANG_COMMIT", "RULES_ERLANG_SHA256", "RULES_ERLANG_URL", "TOOL_SOURCES")
load(":repos.bzl", "HEX_BUILD", "HUB_NAME", "PRIVATE_BUILD", "ROOT_BUILD", "TOOL_BUILDS", "cell_repository_name", "hub_build", "otp_repo", "rebar_build", "rebar_hub_build")


HEX_DEPS = {"dx_erlperf": ["@dx_erlang_argparse//:erlang_app"]}


def _declare_rebar(mctx, tag):
    packages = rebar_lock_packages(mctx.read(tag.lock))
    for package in packages:
        hex_package(
            name = tag.name + "_" + package["package"],
            build_file_content = rebar_build(package, packages, tag.name),
            package = package["package"],
            sha256 = package["sha256"],
            version = package["version"],
        )
    hub_repo(name = tag.name, files = {"BUILD.bazel": rebar_hub_build(packages, tag.name)})

def _erlang_impl(mctx):
    pinned_archive(
        name = "rules_erlang",
        dropped = ["MODULE.bazel", "gazelle", "test"],
        files = {
            "BUILD.bazel": ROOT_BUILD,
            "private/BUILD.bazel": PRIVATE_BUILD,
        },
        patches = [
            Label("//patches/rules_erlang:01_xrl_yrl.patch"),
            Label("//patches/rules_erlang:02_tcw_tags.patch"),
            Label("//patches/rules_erlang:03_auto_sname.patch"),
            Label("//patches/rules_erlang:04_otp_tree.patch"),
        ],
        sha256 = RULES_ERLANG_SHA256,
        strip_prefix = "rules_erlang-" + RULES_ERLANG_COMMIT,
        type = "tar.gz",
        urls = [RULES_ERLANG_URL],
    )
    for cell in sorted(CELLS.keys()):
        otp_repo(name = cell_repository_name(cell), cell = cell)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    for tool in sorted(TOOL_SOURCES.keys()):
        source = TOOL_SOURCES[tool]
        pinned_archive(
            name = tool,
            dropped = source.dropped,
            files = {"BUILD.bazel": TOOL_BUILDS[tool]},
            patches = source.patches,
            sha256 = source.sha256,
            strip_prefix = source.strip_prefix,
            urls = [source.url],
        )
    for name, hex in sorted(HEX_SOURCES.items()):
        hex_package(
            name = name,
            build_file_content = HEX_BUILD.format(app = hex.package, version = hex.version, deps = repr(HEX_DEPS.get(name, []))),
            package = hex.package,
            sha256 = hex.sha256,
            version = hex.version,
        )
    for module in mctx.modules:
        for tag in module.tags.rebar:
            _declare_rebar(mctx, tag)
    return mctx.extension_metadata(reproducible = True)


erlang = module_extension(
    implementation = _erlang_impl,
    doc = "Creates rules_erlang, the pinned OTP trees, the toolchain hub and the erlfmt and elvis sources.",
    tag_classes = {
        "rebar": tag_class(
            doc = "Declares a hex_package per entry of a rebar.lock and a hub repo with one erlang_app alias per package.",
            attrs = {
                "lock": attr.label(mandatory = True, allow_single_file = True, doc = "The rebar.lock to read."),
                "name": attr.string(mandatory = True, doc = "Hub repository name; package repos are named <name>_<package>."),
            },
        ),
    },
)
