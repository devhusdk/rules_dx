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
            {
                "name": "LGPL-3.0-or-later OR GPL-2.0-or-later",
                "source": "https://gmplib.org/",
            },
            {
                "name": "MIT",
                "source": "https://github.com/libffi/libffi/blob/master/LICENSE",
            },
            {
                "name": "LGPL-2.1-or-later",
                "source": "https://www.gnu.org/software/libiconv/",
            },
        ],
        "linkage": "static",
        "needed_shared_libraries": [],
        "os": "linux",
        "runtime_files": [],
        "sha256": "367eb4969986504cf171f71f91e24ea5d7d9d97c41701e8e6eef9b642fd0d201",
        "size": 8744258,
        "tool": "ormolu",
        "upstream_version": "0.9.0.0",
        "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-x86_64-linux.zip",
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
                    "mode": "0o555",
                    "name": "libcharset.1.dylib",
                    "size": 52304,
                },
                {
                    "is_executable": True,
                    "mode": "0o555",
                    "name": "libffi.7.dylib",
                    "size": 121040,
                },
                {
                    "is_executable": True,
                    "mode": "0o555",
                    "name": "libgmp.10.dylib",
                    "size": 516992,
                },
                {
                    "is_executable": True,
                    "mode": "0o555",
                    "name": "libiconv.2.dylib",
                    "size": 93488,
                },
                {
                    "is_executable": True,
                    "mode": "0o555",
                    "name": "ormolu",
                    "size": 117100608,
                },
            ],
        },
        "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "ormolu",
        "executable_sha256": "0d116c2e48b59b47eeae3d201837c42f85aa195246c05acf77870cc2f373ac24",
        "interpreter": None,
        "licenses": [
            {
                "name": "BSD-3-Clause",
                "source": "https://github.com/tweag/ormolu/blob/0.9.0.0/LICENSE.md",
            },
            {
                "name": "LGPL-3.0-or-later OR GPL-2.0-or-later",
                "source": "https://gmplib.org/",
            },
            {
                "name": "MIT",
                "source": "https://github.com/libffi/libffi/blob/master/LICENSE",
            },
            {
                "name": "LGPL-2.1-or-later",
                "source": "https://www.gnu.org/software/libiconv/",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [
            {
                "format": "zip",
                "member": "libcharset.1.dylib",
                "member_sha256": "36ce0f27ddec35eecd4290ae4e1b8eb3715ecd1e669e83efdb95958ecdcdd71d",
                "path": "libcharset.1.dylib",
                "sha256": "39efe03a0f41385bd5a1a7f874acf195fb5f6d09673be96a24f708508bb86a20",
                "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-aarch64-darwin.zip",
            },
            {
                "format": "zip",
                "member": "libffi.7.dylib",
                "member_sha256": "6c8694c78534f9550e80ffbcf80b69b0abb458192a274dbd2ab5267c19402cf7",
                "path": "libffi.7.dylib",
                "sha256": "39efe03a0f41385bd5a1a7f874acf195fb5f6d09673be96a24f708508bb86a20",
                "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-aarch64-darwin.zip",
            },
            {
                "format": "zip",
                "member": "libgmp.10.dylib",
                "member_sha256": "98844b1d66a9f8d50f3f5feb876f28e9a96e7fe3aea61d0f967bba6dc4fa9579",
                "path": "libgmp.10.dylib",
                "sha256": "39efe03a0f41385bd5a1a7f874acf195fb5f6d09673be96a24f708508bb86a20",
                "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-aarch64-darwin.zip",
            },
            {
                "format": "zip",
                "member": "libiconv.2.dylib",
                "member_sha256": "5cb6c00c480b7d8dd95326a3597fb2da9739faa66fa9119bb495a92d107dac2a",
                "path": "libiconv.2.dylib",
                "sha256": "39efe03a0f41385bd5a1a7f874acf195fb5f6d09673be96a24f708508bb86a20",
                "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-aarch64-darwin.zip",
            },
        ],
        "sha256": "39efe03a0f41385bd5a1a7f874acf195fb5f6d09673be96a24f708508bb86a20",
        "size": 23481402,
        "tool": "ormolu",
        "upstream_version": "0.9.0.0",
        "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-aarch64-darwin.zip",
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
            {
                "name": "LGPL-3.0-or-later OR GPL-2.0-or-later",
                "source": "https://gmplib.org/",
            },
            {
                "name": "MIT",
                "source": "https://github.com/libffi/libffi/blob/master/LICENSE",
            },
            {
                "name": "LGPL-2.1-or-later",
                "source": "https://www.gnu.org/software/libiconv/",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "afcde0c0a9bace9c8ae9bd98d027620b2901947670dfc54f63fa44c221d5b19d",
        "size": 18555622,
        "tool": "ormolu",
        "upstream_version": "0.9.0.0",
        "url": "https://github.com/tweag/ormolu/releases/download/0.9.0.0/ormolu-x86_64-windows.zip",
    },
}
