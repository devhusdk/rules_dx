"""Tests for the @dx_prebuilt plan, pins and hub BUILD text."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":hub.bzl", "LAUNCHER_SOURCE", "SOURCE_LABEL", "SOURCE_TOOLS", "alias_build", "consumer_build", "plan_assets", "plan_root", "valid_sha256")
load(":pins.bzl", "DX_VERSION", "PINS", "SCHEMA", "URL_TEMPLATE")
load(":tools.bzl", "TOOLS", "asset_name", "registry_lines")

visibility("private")

_HASH = "0123456789abcdef" * 4

_PINNED = {"dx": {
    "linux_x86_64": _HASH,
    "windows_x86_64": _HASH,
}}

_ALL_PINNED = {"dx": {key: _HASH for key in TOOLS["dx"].platforms}}

def _override(url, os = "", cpu = ""):
    return struct(cpu = cpu, os = os, sha256 = _HASH, url = url)

def _mod(name, is_root, assets = [], from_source = False):
    return struct(name = name, is_root = is_root, tags = struct(asset = assets, from_source = [struct()] if from_source else []))

def _tag(sha256 = _HASH, name = "dx", os = "linux", cpu = "x86_64"):
    return struct(cpu = cpu, name = name, os = os, sha256 = sha256, url = "https://mirror/dx")

def _checks():
    middle = plan_root([_mod("app", True), _mod("middle", False, from_source = True)])
    badsha = plan_root([_mod("app", True, assets = [_tag("ABC")])])
    badname = plan_root([_mod("app", True, assets = [_tag(name = "dx-linux-x86_64")])])
    badcpu = plan_root([_mod("app", True, assets = [_tag(cpu = "arm64")])])
    twice = plan_root([_mod("app", True, assets = [_tag(), _tag()])])
    rooted = plan_root([_mod("rules_dx", True)])
    sourced = plan_root([_mod("app", True, from_source = True)])
    pinned_root = plan_root([_mod("app", True, assets = [_tag()]), _mod("rules_dx", False)])
    unpinned = plan_assets("dx", "1.2.3", {}, URL_TEMPLATE, {})
    pinned = plan_assets("dx", "1.2.3", _PINNED, URL_TEMPLATE, {})
    pinned_build = consumer_build(pinned)
    all_pinned_build = consumer_build(plan_assets("dx", "1.2.3", _ALL_PINNED, URL_TEMPLATE, {}))
    linux = [a for a in pinned if a.key == "linux_x86_64"][0]
    windows = [a for a in pinned if a.key == "windows_x86_64"][0]
    windows_arm = [a for a in pinned if a.key == "windows_aarch64"][0]
    mirrored = plan_assets("dx", "1.2.3", PINS, URL_TEMPLATE, {
        "linux_riscv64": _override("file:///dx", os = "linux", cpu = "riscv64"),
        "macos_aarch64": _override("https://mirror/dx", os = "macos", cpu = "aarch64"),
    })
    mirror = [a for a in mirrored if a.key == "macos_aarch64"][0]
    riscv = [a for a in mirrored if a.key == "linux_riscv64"][0]
    unpinned_build = consumer_build(unpinned)
    launchers = plan_assets("launcher", "1.2.3", {"launcher": {"linux_x86_64": _HASH, "windows_x86_64": _HASH}}, URL_TEMPLATE, {}, exe = "dx-launcher", repo_prefix = "dx_launcher_prebuilt_")
    launcher_build = consumer_build(unpinned, launchers)
    both_build = consumer_build(pinned, launchers)
    launcher_root = plan_root([_mod("app", True, assets = [_tag(), _tag(name = "launcher")])], tools = ["dx", "launcher"])
    return [
        expect_equal("pins.bzl is schema 1", SCHEMA, 1),
        expect_equal("pins name only registry tools and platforms", [(t, k) for t, by_key in PINS.items() for k in by_key if t not in TOOLS or k not in TOOLS[t].platforms], []),
        expect_equal("pins are empty or a sha256", [(t, k) for t, by_key in PINS.items() for k, v in by_key.items() if not valid_sha256(v)], []),
        expect_equal("dx assets keep their release names", [asset_name("dx", k) for k in TOOLS["dx"].platforms], ["dx-linux-arm64", "dx-linux-x86_64", "dx-macos-arm64", "dx-windows-arm64", "dx-windows-x86_64"]),
        expect_contains("the registry lists gazelle for fill_pins", registry_lines(), "gazelle windows_x86_64 dx-gazelle-windows-x86_64"),
        expect_equal("a fallback takes the x86_64 download", windows_arm.repo, "dx_prebuilt_windows_x86_64"),
        expect_equal("a fallback keeps its own platform", windows_arm.key, "windows_aarch64"),
        expect_contains("a fallback platform runs the shared download", pinned_build, 'name = "dx_windows_aarch64",\n    src = "@dx_prebuilt_windows_x86_64//file:dx.exe",'),
        expect_true("the version is semver", len(DX_VERSION.split(".")) == 3),
        expect_contains("the url names the version tag", URL_TEMPLATE, "/download/v{version}/{asset}"),
        expect_false("an uppercase hash is invalid", valid_sha256(_HASH.upper())),
        expect_false("a short hash is invalid", valid_sha256("abc")),
        expect_equal("repo names", windows.repo, "dx_prebuilt_windows_x86_64"),
        expect_equal("unpinned assets get no repo", [a.repo for a in unpinned], [""] * 5),
        expect_equal("unpinned assets get no url", [a.url for a in unpinned], [""] * 5),
        expect_equal("a pinned asset downloads its release", linux.url, "https://github.com/devhusdk/rules_dx/releases/download/v1.2.3/dx-linux-x86_64"),
        expect_equal("a pinned asset keeps its hash", linux.sha256, _HASH),
        expect_equal("linux runs dx", linux.exe, "dx"),
        expect_equal("windows runs dx.exe", windows.exe, "dx.exe"),
        expect_equal("an override replaces the url", mirror.url, "https://mirror/dx"),
        expect_equal("an override pins its hash", mirror.repo, "dx_prebuilt_macos_aarch64"),
        expect_equal("a new asset takes its platform", riscv.key, "linux_riscv64"),
        expect_contains("the hub selects a platform", pinned_build, '":linux_x86_64": ":dx_linux_x86_64",'),
        expect_contains("the hub runs the download", pinned_build, 'src = "@dx_prebuilt_linux_x86_64//file:dx",'),
        expect_contains("windows keeps .exe", pinned_build, 'out = "windows_x86_64/dx.exe",'),
        expect_contains("other platforms fail", pinned_build, '"//conditions:default": ":unsupported",'),
        expect_contains("the failure names the remedies", pinned_build, "dx_prebuilt.from_source(), which requires Rust and C toolchains"),
        expect_contains("the failure lists supported platforms", pinned_build, "linux x86_64, macos aarch64"),
        expect_equal("a release with no pins builds dx from source", unpinned_build, alias_build(SOURCE_LABEL)),
        expect_contains("an unpinned asset builds dx from source", pinned_build, '":linux_aarch64": "@rules_dx//cli/cli:dx",'),
        expect_false("an unpinned asset downloads nothing", "dx_prebuilt_linux_aarch64" in pinned_build),
        expect_false("a fully pinned hub never names the source dx", SOURCE_LABEL in all_pinned_build),
        expect_equal("a fully pinned hub never names a source tool", [t for t in SOURCE_TOOLS.values() if t in all_pinned_build], []),
        expect_contains("a pinned platform runs dx as the runner", pinned_build, '":linux_x86_64": ":quality_runner_linux_x86_64",'),
        expect_contains("the runner is the downloaded dx", pinned_build, 'name = "quality_runner_linux_x86_64",\n    src = "@dx_prebuilt_linux_x86_64//file:dx",\n    out = "linux_x86_64/quality_runner",'),
        expect_contains("a windows tool keeps .exe", pinned_build, 'out = "windows_x86_64/quality_notebook.exe",'),
        expect_contains("the rule helpers are the downloaded dx", pinned_build, 'name = "env_shard_writer_linux_x86_64",\n    src = "@dx_prebuilt_linux_x86_64//file:dx",\n    out = "linux_x86_64/env_shard_writer",'),
        expect_contains("a windows rule helper keeps .exe", pinned_build, 'out = "windows_x86_64/codegen_shard_writer.exe",'),
        expect_contains("the coverage collector is the downloaded dx", pinned_build, 'out = "linux_x86_64/collect_native",'),
        expect_contains("the lsp driver is the downloaded dx", pinned_build, 'out = "linux_x86_64/lsp_check",'),
        expect_contains("an unpinned hub aliases the source shard writers", unpinned_build, 'actual = "@rules_dx//cli/codegen_shard:codegen_shard_writer",'),
        expect_contains("an unpinned platform builds the tool from source", pinned_build, '":linux_aarch64": "@rules_dx//core/quality/markdown:quality_markdown",'),
        expect_contains("the root hub aliases the source runner", alias_build(SOURCE_LABEL), 'name = "quality_runner",\n    actual = "@rules_dx//core/quality/runner:quality_runner",'),
        expect_contains("an unpinned hub aliases the source notebook tool", unpinned_build, 'actual = "@rules_dx//core/quality/notebook:quality_notebook",'),
        expect_contains("an unpinned hub aliases the python quality tools", unpinned_build, 'actual = "@rules_dx//core/quality/tools/python:yamllint",'),
        expect_contains("a pinned hub aliases the python quality tools", pinned_build, 'name = "python_tool_yamllint",'),
        expect_false("the consumer hub loads nothing from rules_rust", "rules_rust" in all_pinned_build),
        expect_contains("the root hub aliases the source dx", alias_build(SOURCE_LABEL), 'actual = "@rules_dx//cli/cli:dx",'),
        expect_contains("a non-root module cannot set tags", middle.error, "only the root module may set asset or from_source tags; middle sets them"),
        expect_contains("a bad sha names its asset", badsha.error, 'dx_prebuilt.asset(name = "dx", os = "linux", cpu = "x86_64"): sha256 must be 64 lowercase hex digits'),
        expect_contains("an asset name is the tool", badname.error, 'name is one of the tools ["dx"]'),
        expect_contains("cpu uses @platforms spelling", badcpu.error, "cpu one of"),
        expect_contains("one asset per os and cpu", twice.error, "a second asset for the same os and cpu"),
        expect_true("from_source builds from source", sourced.source),
        expect_true("rules_dx as root builds from source", rooted.source),
        expect_false("a consumer root does not build from source", pinned_root.source),
        expect_equal("a root asset is an override", pinned_root.overrides["dx"].keys(), ["linux_x86_64"]),
        expect_equal("a root launcher asset overrides only the launcher", sorted(launcher_root.overrides.keys()), ["dx", "launcher"]),
        expect_equal("the launcher is a target-platform asset", TOOLS["launcher"].role, "target"),
        expect_equal("launcher assets", [asset_name("launcher", k) for k in TOOLS["launcher"].platforms], ["dx-launcher-linux-arm64", "dx-launcher-linux-x86_64", "dx-launcher-macos-arm64", "dx-launcher-windows-arm64", "dx-launcher-windows-x86_64"]),
        expect_contains("the root hub aliases the source launcher", alias_build(SOURCE_LABEL), 'name = "launcher",\n    actual = "@rules_dx//cli/launcher:exec_launcher",'),
        expect_contains("an unpinned hub builds the launcher from source", unpinned_build, 'actual = "@rules_dx//cli/launcher:exec_launcher",'),
        expect_contains("a pinned launcher runs its download", launcher_build, 'name = "launcher_linux_x86_64",\n    src = "@dx_launcher_prebuilt_linux_x86_64//file:dx-launcher",\n    out = "linux_x86_64/dx-launcher",'),
        expect_contains("a windows launcher keeps .exe", launcher_build, 'out = "windows_x86_64/dx-launcher.exe",'),
        expect_contains("the launcher selects its platform", launcher_build, '":linux_x86_64": ":launcher_linux_x86_64",'),
        expect_contains("other targets build the launcher from source", launcher_build, '"//conditions:default": "{}",'.format(LAUNCHER_SOURCE)),
        expect_false("an unpinned launcher platform downloads nothing", "dx_launcher_prebuilt_linux_aarch64" in launcher_build),
        expect_equal("dx and launcher share one config_setting per platform", both_build.count('name = "linux_x86_64",\n    constraint_values'), 1),
        expect_equal("a valid root has no error", pinned_root.error, ""),
    ]

def dx_prebuilt_hub_tests(name):
    """Declares the @dx_prebuilt hub test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
