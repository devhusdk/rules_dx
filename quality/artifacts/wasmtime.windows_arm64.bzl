"""wasmtime standalone artifact metadata (windows_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": None,
        "libc": None,
        "libstdcxx": None,
    },
    "archive": {
        "format": "zip",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-aarch64-windows/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-aarch64-windows/LICENSE",
                "size": 12243,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-aarch64-windows/README.md",
                "size": 7674,
            },
            {
                "is_executable": True,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-aarch64-windows/wasmtime-min.exe",
                "size": 1915392,
            },
            {
                "is_executable": True,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-aarch64-windows/wasmtime.exe",
                "size": 36521984,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "wasmtime-v49.0.2-aarch64-windows/wasmtime.exe",
    "executable_sha256": "71c5d746f02bbe4994e6f85c8db1e8a695418d36816ed7d028a03f778ea8a3a0",
    "interpreter": None,
    "licenses": [
        {
            "name": "Apache-2.0 WITH LLVM-exception",
            "source": "https://github.com/bytecodealliance/wasmtime/blob/v49.0.2/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [],
    "os": "windows",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "e3a5cf6a18b9af5e922f5e3434711278311b58da010765d9bb5b7885a1bf2095",
    "size": 12193971,
    "tool": "wasmtime",
    "upstream_version": "49.0.2",
    "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-aarch64-windows.zip",
}
