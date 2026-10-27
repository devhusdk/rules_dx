"""wasmtime standalone artifact metadata (macos_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": None,
        "libc": None,
        "libstdcxx": None,
    },
    "archive": {
        "format": "tar.xz",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-x86_64-macos",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-x86_64-macos/LICENSE",
                "size": 12243,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-x86_64-macos/wasmtime-min",
                "size": 3068424,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-x86_64-macos/wasmtime",
                "size": 63974616,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-x86_64-macos/README.md",
                "size": 7674,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "wasmtime-v49.0.2-x86_64-macos/wasmtime",
    "executable_sha256": "3e4920a081ee945c7633ac82ad4cf8456551b1913586da77eea6c0520a32eadc",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0 WITH LLVM-exception",
            "source": "https://github.com/bytecodealliance/wasmtime/blob/v49.0.2/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "macos",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "b24513efff6a5dbeb196e2a01097ba632a8b0ea1563ba297ada26ceb3aa067d9",
    "size": 10848216,
    "tool": "wasmtime",
    "upstream_version": "49.0.2",
    "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-x86_64-macos.tar.xz",
}
