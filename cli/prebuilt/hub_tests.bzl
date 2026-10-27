"""Tests for the @dx_prebuilt plan, pins and hub BUILD text."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":hub.bzl", "ASSET_PLATFORMS", "SOURCE_LABEL", "alias_build", "consumer_build", "plan_assets", "repo_name", "valid_sha256")
load(":pins.bzl", "DX_VERSION", "SHA256", "URL_TEMPLATE")

_HASH = "0123456789abcdef" * 4

_PINNED = {
    "dx-linux-arm64": "",
    "dx-linux-x86_64": _HASH,
    "dx-macos-arm64": "",
    "dx-windows-arm64": "",
    "dx-windows-x86_64": _HASH,
}

_ALL_PINNED = {name: _HASH for name in _PINNED}

def _override(url, os = "", cpu = ""):
    return struct(cpu = cpu, os = os, sha256 = _HASH, url = url)

def _checks():
    unpinned = plan_assets("1.2.3", {name: "" for name in _PINNED}, URL_TEMPLATE, {})
    pinned = plan_assets("1.2.3", _PINNED, URL_TEMPLATE, {})
    pinned_build = consumer_build(pinned)
    all_pinned_build = consumer_build(plan_assets("1.2.3", _ALL_PINNED, URL_TEMPLATE, {}))
    linux = [a for a in pinned if a.asset == "dx-linux-x86_64"][0]
    windows = [a for a in pinned if a.asset == "dx-windows-x86_64"][0]
    mirrored = plan_assets("1.2.3", SHA256, URL_TEMPLATE, {
        "dx-linux-riscv64": _override("file:///dx", os = "linux", cpu = "riscv64"),
        "dx-macos-arm64": _override("https://mirror/dx"),
    })
    mirror = [a for a in mirrored if a.asset == "dx-macos-arm64"][0]
    riscv = [a for a in mirrored if a.asset == "dx-linux-riscv64"][0]
    unpinned_build = consumer_build(unpinned)
    return [
        expect_equal("pins name every release asset", sorted(SHA256.keys()), sorted(ASSET_PLATFORMS.keys())),
        expect_equal("pins are empty or a sha256", [k for k, v in SHA256.items() if v and not valid_sha256(v)], []),
        expect_true("the version is semver", len(DX_VERSION.split(".")) == 3),
        expect_contains("the url names the version tag", URL_TEMPLATE, "/download/v{version}/{asset}"),
        expect_false("an uppercase hash is invalid", valid_sha256(_HASH.upper())),
        expect_false("a short hash is invalid", valid_sha256("abc")),
        expect_equal("repo names", repo_name("dx-windows-x86_64"), "dx_prebuilt_windows_x86_64"),
        expect_equal("unpinned assets get no repo", [a.repo for a in unpinned], [""] * 5),
        expect_equal("unpinned assets get no url", [a.url for a in unpinned], [""] * 5),
        expect_equal("a pinned asset downloads its release", linux.url, "https://github.com/devhusdk/rules_dx/releases/download/v1.2.3/dx-linux-x86_64"),
        expect_equal("a pinned asset keeps its hash", linux.sha256, _HASH),
        expect_equal("linux runs dx", linux.exe, "dx"),
        expect_equal("windows runs dx.exe", windows.exe, "dx.exe"),
        expect_equal("an override replaces the url", mirror.url, "https://mirror/dx"),
        expect_equal("an override pins its hash", mirror.repo, "dx_prebuilt_macos_arm64"),
        expect_equal("a new asset takes its platform", riscv.key, "linux_riscv64"),
        expect_contains("the hub selects a platform", pinned_build, '":linux_x86_64": ":dx_linux_x86_64",'),
        expect_contains("the hub runs the download", pinned_build, 'src = "@dx_prebuilt_linux_x86_64//file:dx",'),
        expect_contains("windows keeps .exe", pinned_build, 'out = "windows_x86_64/dx.exe",'),
        expect_contains("other platforms fail", pinned_build, '"//conditions:default": ":unsupported",'),
        expect_contains("the failure names the remedies", pinned_build, "dx_prebuilt.from_source(), which requires Rust and C toolchains"),
        expect_contains("the failure lists supported platforms", pinned_build, "linux x86_64, macos aarch64"),
        expect_equal("a release with no pins builds dx from source", unpinned_build, alias_build(SOURCE_LABEL)),
        expect_contains("an unpinned asset builds dx from source", pinned_build, '":linux_aarch64": "@rules_dx//cli/cli:dx",'),
        expect_false("an unpinned asset downloads nothing", "dx_prebuilt_linux_arm64" in pinned_build),
        expect_false("a fully pinned hub never names the source dx", SOURCE_LABEL in all_pinned_build),
        expect_false("the consumer hub loads nothing from rules_rust", "rules_rust" in all_pinned_build),
        expect_contains("the root hub aliases the source dx", alias_build(SOURCE_LABEL), 'actual = "@rules_dx//cli/cli:dx",'),
    ]

def dx_prebuilt_hub_tests(name):
    """Declares the @dx_prebuilt hub test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
