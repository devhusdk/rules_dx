"""Pinned Ubuntu 22.04 runtime .deb download URL tests."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core:jammy_runtime.bzl", "JAMMY_POOL_MIRRORS", "JAMMY_RUNTIME", "JAMMY_RUNTIME_MIRROR", "jammy_deb_urls")

def _checks():
    checks = [
        expect_equal("libtinfo6 amd64 urls", jammy_deb_urls(JAMMY_RUNTIME["linux_x86_64"][0]), [
            "https://launchpad.net/ubuntu/+archive/primary/+files/libtinfo6_6.3-2ubuntu0.3_amd64.deb",
            "https://archive.ubuntu.com/ubuntu/pool/main/n/ncurses/libtinfo6_6.3-2ubuntu0.3_amd64.deb",
        ]),
        expect_equal("libtinfo6 arm64 urls", jammy_deb_urls(JAMMY_RUNTIME["linux_arm64"][0]), [
            "https://launchpad.net/ubuntu/+archive/primary/+files/libtinfo6_6.3-2ubuntu0.3_arm64.deb",
            "https://ports.ubuntu.com/ubuntu-ports/pool/main/n/ncurses/libtinfo6_6.3-2ubuntu0.3_arm64.deb",
        ]),
    ]
    for cell, arch in [("linux_x86_64", "amd64"), ("linux_arm64", "arm64")]:
        for package in JAMMY_RUNTIME[cell]:
            urls = jammy_deb_urls(package)
            checks.append(expect_equal(
                cell + " " + package.package + " mirrors",
                [urls[0].startswith(JAMMY_RUNTIME_MIRROR + "/"), urls[1].startswith(JAMMY_POOL_MIRRORS[arch] + "/main/")],
                [True, True],
            ))
            checks.append(expect_equal(
                cell + " " + package.package + " files",
                [url.rpartition("/")[2] for url in urls],
                [package.deb, package.deb],
            ))
    return checks

def jammy_runtime_tests(name, size):
    """Declares the pinned runtime .deb URL test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )
