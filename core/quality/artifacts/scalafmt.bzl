"""scalafmt standalone artifact metadata -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACTS = {
    "linux_arm64": {
        "abi_floor": {
            "kernel": "3.7.0",
            "libc": "2.34",
            "libstdcxx": None,
        },
        "archive": {
            "format": "zip",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o744",
                    "name": "scalafmt",
                    "size": 54401512,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "scalafmt",
        "executable_sha256": "799e9d7412971b254a2ff216c7313673c8e4ecd162a228208dd9c93ba66cf70b",
        "interpreter": "/lib/ld-linux-aarch64.so.1",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/scalameta/scalafmt/blob/v3.11.4/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [
            "libc.so.6",
            "libm.so.6",
        ],
        "os": "linux",
        "runtime_files": [],
        "sha256": "07fd593f3efb887a71f8400db48a9424279d37bf712630ebe1ea337ccc1945e7",
        "size": 14356691,
        "tool": "scalafmt",
        "upstream_version": "3.11.4",
        "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.4/scalafmt-aarch64-pc-linux.zip",
    },
    "linux_x86_64": {
        "abi_floor": {
            "kernel": "3.2.0",
            "libc": "2.34",
            "libstdcxx": None,
        },
        "archive": {
            "format": "zip",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o744",
                    "name": "scalafmt",
                    "size": 57205024,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "scalafmt",
        "executable_sha256": "88526f9f4d64c2fb023d54578812419f49e2ec09e30e4fb77443a05f1a59cac0",
        "interpreter": "/lib64/ld-linux-x86-64.so.2",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/scalameta/scalafmt/blob/v3.11.4/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [
            "libc.so.6",
            "libm.so.6",
        ],
        "os": "linux",
        "runtime_files": [],
        "sha256": "e7d43a5621074a63a46d5b287d0b0bb0650033deeb836af2b27515b2127476f2",
        "size": 16466224,
        "tool": "scalafmt",
        "upstream_version": "3.11.4",
        "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.4/scalafmt-x86_64-pc-linux.zip",
    },
    "macos_arm64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "zip",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o744",
                    "name": "scalafmt",
                    "size": 82708136,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "scalafmt",
        "executable_sha256": "f3c1d89ff2a5afdbb5d1ce695b32989b503d826d0ef785ff68f663ab110daa4d",
        "interpreter": None,
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/scalameta/scalafmt/blob/v3.11.4/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "e5c173f98e20439bbb725a94c73d9bc30ba9d6cc14c15f4476a7132514f61b08",
        "size": 16749925,
        "tool": "scalafmt",
        "upstream_version": "3.11.4",
        "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.4/scalafmt-aarch64-apple-darwin.zip",
    },
    "windows_x86_64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "zip",
            "members": [
                {
                    "is_executable": True,
                    "mode": "0o0",
                    "name": "scalafmt.exe",
                    "size": 46400000,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "scalafmt.exe",
        "executable_sha256": "c2f473698f40b0d0bd9b427ee0064eceb9f13a8163a53b0b931378577ca0beb3",
        "interpreter": None,
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/scalameta/scalafmt/blob/v3.11.4/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "db1edbf50a045e2e56559d4a80601c4d6445db653594bea9617ebcfbbfcd6e9a",
        "size": 12062685,
        "tool": "scalafmt",
        "upstream_version": "3.11.4",
        "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.4/scalafmt-x86_64-pc-win32.zip",
    },
}
