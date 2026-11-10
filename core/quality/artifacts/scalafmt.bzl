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
                    "size": 53053544,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "scalafmt",
        "executable_sha256": "845ca3f043297d5bf81c711abcf91da39be8c88c3b27285f5e556cd3f88e087c",
        "interpreter": "/lib/ld-linux-aarch64.so.1",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/scalameta/scalafmt/blob/v3.11.5/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [
            "libc.so.6",
            "libm.so.6",
        ],
        "os": "linux",
        "runtime_files": [],
        "sha256": "691576a60e00f4bca150d251395196a382f0828697a9b1b4c9c44815eaea80bc",
        "size": 13711539,
        "tool": "scalafmt",
        "upstream_version": "3.11.5",
        "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.5/scalafmt-aarch64-pc-linux.zip",
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
                    "size": 55488392,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "scalafmt",
        "executable_sha256": "43ed63e02ea8982d6b2d49eeaf8877bc252699bf33815feb0ba418691fccc4de",
        "interpreter": "/lib64/ld-linux-x86-64.so.2",
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/scalameta/scalafmt/blob/v3.11.5/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [
            "libc.so.6",
            "libm.so.6",
        ],
        "os": "linux",
        "runtime_files": [],
        "sha256": "7b0e44cc193eeff06daa14b394dc0489d8eaa4537eb945522f7f267641ed8c2b",
        "size": 15496517,
        "tool": "scalafmt",
        "upstream_version": "3.11.5",
        "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.5/scalafmt-x86_64-pc-linux.zip",
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
                    "size": 85134328,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "scalafmt",
        "executable_sha256": "0204ac05641b08ca8159a56d5a766a4547ac8eb027f1fb7b49adc9bd25df2da1",
        "interpreter": None,
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/scalameta/scalafmt/blob/v3.11.5/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "8ccd72f826dd33b5c7ab5e0e01944044d041aa8c3c49cbda3caac31930483d5d",
        "size": 16360968,
        "tool": "scalafmt",
        "upstream_version": "3.11.5",
        "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.5/scalafmt-aarch64-apple-darwin.zip",
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
                    "size": 44546048,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "scalafmt.exe",
        "executable_sha256": "ea6bccf36c2e907599a22e997e5f63ae1703e650bda6cfcf1caf3ed715d58fd5",
        "interpreter": None,
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/scalameta/scalafmt/blob/v3.11.5/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "d420e6a7205cd87520dc09160e3c420dac7a9782150054b1c42cf2e6c98030db",
        "size": 11370500,
        "tool": "scalafmt",
        "upstream_version": "3.11.5",
        "url": "https://github.com/scalameta/scalafmt/releases/download/v3.11.5/scalafmt-x86_64-pc-win32.zip",
    },
}
