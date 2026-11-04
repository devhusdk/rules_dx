"""tinymist standalone artifact metadata -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACTS = {
    "linux_arm64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "tar.gz",
            "members": [
                {
                    "is_executable": False,
                    "mode": "0o40755",
                    "name": "tinymist-aarch64-unknown-linux-musl",
                    "size": 0,
                },
                {
                    "is_executable": True,
                    "mode": "0o100755",
                    "name": "tinymist-aarch64-unknown-linux-musl/tinymist",
                    "size": 71445944,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "tinymist-aarch64-unknown-linux-musl/tinymist",
        "executable_sha256": "7a9e1dfb39fe4e644576ad5e2be8c093e8054346e49ff1e4d2abd888d7445ec6",
        "interpreter": None,
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/Myriad-Dreamin/tinymist/blob/v0.15.12/LICENSE",
            },
        ],
        "linkage": "static",
        "needed_shared_libraries": [],
        "os": "linux",
        "runtime_files": [],
        "sha256": "9dfdd715cfa3e6636ba8e59df445bdbc70a45cdb6710d0ff3d61c4a948912db6",
        "size": 32219558,
        "tool": "tinymist",
        "upstream_version": "0.15.12",
        "url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-aarch64-unknown-linux-musl.tar.gz",
    },
    "linux_x86_64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "tar.gz",
            "members": [
                {
                    "is_executable": False,
                    "mode": "0o40755",
                    "name": "tinymist-x86_64-unknown-linux-musl",
                    "size": 0,
                },
                {
                    "is_executable": True,
                    "mode": "0o100755",
                    "name": "tinymist-x86_64-unknown-linux-musl/tinymist",
                    "size": 77539944,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "tinymist-x86_64-unknown-linux-musl/tinymist",
        "executable_sha256": "67f093cc1a6e1846065350656667ff9bc8e3da59cf376ddc0525899d81a5489f",
        "interpreter": None,
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/Myriad-Dreamin/tinymist/blob/v0.15.12/LICENSE",
            },
        ],
        "linkage": "static-pie",
        "needed_shared_libraries": [],
        "os": "linux",
        "runtime_files": [],
        "sha256": "bb9c73151dd16864f5bb2c426aa990de424985969bea54b5c4a7d5373cedcab2",
        "size": 33551527,
        "tool": "tinymist",
        "upstream_version": "0.15.12",
        "url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-x86_64-unknown-linux-musl.tar.gz",
    },
    "macos_arm64": {
        "abi_floor": {
            "kernel": None,
            "libc": None,
            "libstdcxx": None,
        },
        "archive": {
            "format": "tar.gz",
            "members": [
                {
                    "is_executable": False,
                    "mode": "0o40755",
                    "name": "tinymist-aarch64-apple-darwin",
                    "size": 0,
                },
                {
                    "is_executable": True,
                    "mode": "0o100755",
                    "name": "tinymist-aarch64-apple-darwin/tinymist",
                    "size": 64973648,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "tinymist-aarch64-apple-darwin/tinymist",
        "executable_sha256": "92f1d92e7822be822a054aa29a97b4099439ee907c6676034a5002d572ae2f67",
        "interpreter": None,
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/Myriad-Dreamin/tinymist/blob/v0.15.12/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "macos",
        "runtime_files": [],
        "sha256": "5f77362580b0bfc8a31b47718dfe3f426ce854d59f04030ade398f1926167b06",
        "size": 30842684,
        "tool": "tinymist",
        "upstream_version": "0.15.12",
        "url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-aarch64-apple-darwin.tar.gz",
    },
    "windows_arm64": {
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
                    "mode": "0o644",
                    "name": "tinymist.exe",
                    "size": 58044416,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "arm64",
        "delivery_class": "standalone",
        "executable": "tinymist.exe",
        "executable_sha256": "226d9f069041117fd5cbd83babc12d60943246c9c0b0d06e935a52b18a9eacdb",
        "interpreter": None,
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/Myriad-Dreamin/tinymist/blob/v0.15.12/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "c97b019b99be096000352975fff5ac5e7d6c58c31b63af3712091768f9dec455",
        "size": 28755976,
        "tool": "tinymist",
        "upstream_version": "0.15.12",
        "url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-aarch64-pc-windows-msvc.zip",
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
                    "mode": "0o644",
                    "name": "tinymist.exe",
                    "size": 64103424,
                },
            ],
        },
        "checksum_source": "upstream published checksums file",
        "cpu": "x86_64",
        "delivery_class": "standalone",
        "executable": "tinymist.exe",
        "executable_sha256": "c7a2ad09d2f25f18e09f203deda20de1e086b3134fe70a5d163b720e97524af4",
        "interpreter": None,
        "licenses": [
            {
                "name": "Apache-2.0",
                "source": "https://github.com/Myriad-Dreamin/tinymist/blob/v0.15.12/LICENSE",
            },
        ],
        "linkage": "dynamic",
        "needed_shared_libraries": [],
        "os": "windows",
        "runtime_files": [],
        "sha256": "7dfa68a764363bf825871c7d0ea302b6590a33b5b3ae43585e34d04296e987b1",
        "size": 30079432,
        "tool": "tinymist",
        "upstream_version": "0.15.12",
        "url": "https://github.com/Myriad-Dreamin/tinymist/releases/download/v0.15.12/tinymist-x86_64-pc-windows-msvc.zip",
    },
}
