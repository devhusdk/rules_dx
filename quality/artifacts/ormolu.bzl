"""ormolu standalone artifact metadata -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACTS = {
    "linux_x86_64": {
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
                    "mode": "0o555",
                    "name": "ormolu",
                    "size": 44852176,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "ormolu",
        "executable_sha256": "40eed9dcc3413c136725d8c2e349b9c86ee7ce97d2d2347309af47c92e7bf6c3",
        "interpreter": None,
        "licenses": [
            {
                "name": "BSD-3-Clause",
                "source": "https://github.com/tweag/ormolu/blob/0.9.0.0/LICENSE.md",
            },
        ],
        "linkage": "static",
        "needed_shared_libraries": [],
        "os": "linux",
        "runtime_files": [],
        "schema_version": 1,
        "sha256": "367eb4969986504cf171f71f91e24ea5d7d9d97c41701e8e6eef9b642fd0d201",
        "size": 8744258,
        "tool": "ormolu",
        "upstream_version": "0.9.0.0",
        "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-x86_64-linux.zip",
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
                    "name": "ormolu.exe",
                    "size": 110974464,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "ormolu.exe",
        "executable_sha256": "2cfcb3bc8a24684bb094d8f346681d818468f9e633ca51f988cd7f847ed903b4",
        "interpreter": None,
        "licenses": [
            {
                "name": "BSD-3-Clause",
                "source": "https://github.com/tweag/ormolu/blob/0.9.0.0/LICENSE.md",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "schema_version": 1,
        "sha256": "afcde0c0a9bace9c8ae9bd98d027620b2901947670dfc54f63fa44c221d5b19d",
        "size": 18555622,
        "tool": "ormolu",
        "upstream_version": "0.9.0.0",
        "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-x86_64-windows.zip",
    },
}
