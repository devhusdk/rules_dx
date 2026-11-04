"""latexindent standalone artifact metadata -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACTS = {
    "linux_arm64": {
        "abi_floor": {
            "kernel": "3.7.0",
            "libc": "2.34",
            "libstdcxx": None,
        },
        "archive": {
            "format": "none",
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "latexindent-linux-arm64",
        "executable_sha256": "d95dab15896d7088ec59c76b8f9595c4e8f8d53190a7587402b114748a2ca110",
        "interpreter": "/lib/ld-linux-aarch64.so.1",
        "licenses": [
            {
                "name": "GPL-3.0-only",
                "source": "https://github.com/cmhughes/latexindent.pl/blob/V4.0.3/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [
            "ld-linux-aarch64.so.1",
            "libc.so.6",
        ],
        "os": "linux",
        "runtime_files": [],
        "sha256": "d95dab15896d7088ec59c76b8f9595c4e8f8d53190a7587402b114748a2ca110",
        "size": 7937467,
        "tool": "latexindent",
        "upstream_version": "4.0.3",
        "url": "https://github.com/cmhughes/latexindent.pl/releases/download/V4.0.3/latexindent-linux-arm64",
    },
    "linux_x86_64": {
        "abi_floor": {
            "kernel": "3.2.0",
            "libc": "2.34",
            "libstdcxx": None,
        },
        "archive": {
            "format": "none",
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "latexindent-linux",
        "executable_sha256": "6a9a16edcf1242430015316b78498852eba4b700b728f646fc58d654cdd1edbe",
        "interpreter": "/lib64/ld-linux-x86-64.so.2",
        "licenses": [
            {
                "name": "GPL-3.0-only",
                "source": "https://github.com/cmhughes/latexindent.pl/blob/V4.0.3/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [
            "libc.so.6",
        ],
        "os": "linux",
        "runtime_files": [],
        "sha256": "6a9a16edcf1242430015316b78498852eba4b700b728f646fc58d654cdd1edbe",
        "size": 8104042,
        "tool": "latexindent",
        "upstream_version": "4.0.3",
        "url": "https://github.com/cmhughes/latexindent.pl/releases/download/V4.0.3/latexindent-linux",
    },
    "macos_arm64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "none",
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "latexindent-macos",
        "executable_sha256": "8b4c2b3b1fe7986fd294f6caf6e3c95f8090d2ad10d4f31a3af4f7c27e3521a9",
        "interpreter": None,
        "licenses": [
            {
                "name": "GPL-3.0-only",
                "source": "https://github.com/cmhughes/latexindent.pl/blob/V4.0.3/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "8b4c2b3b1fe7986fd294f6caf6e3c95f8090d2ad10d4f31a3af4f7c27e3521a9",
        "size": 7842281,
        "tool": "latexindent",
        "upstream_version": "4.0.3",
        "url": "https://github.com/cmhughes/latexindent.pl/releases/download/V4.0.3/latexindent-macos",
    },
    "windows_x86_64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "none",
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "latexindent.exe",
        "executable_sha256": "6e6d60bb8df66b57fff0db954fcafdb911a88485c79ec19086f80622496dee9b",
        "interpreter": None,
        "licenses": [
            {
                "name": "GPL-3.0-only",
                "source": "https://github.com/cmhughes/latexindent.pl/blob/V4.0.3/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "6e6d60bb8df66b57fff0db954fcafdb911a88485c79ec19086f80622496dee9b",
        "size": 11364075,
        "tool": "latexindent",
        "upstream_version": "4.0.3",
        "url": "https://github.com/cmhughes/latexindent.pl/releases/download/V4.0.3/latexindent.exe",
    },
}
