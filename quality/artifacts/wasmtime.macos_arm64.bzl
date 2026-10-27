"""wasmtime standalone artifact metadata (macos_arm64) -- GENERATED, do not edit."""

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
                "name": "wasmtime-v49.0.2-aarch64-macos",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-aarch64-macos/LICENSE",
                "size": 12243,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-aarch64-macos/wasmtime-min",
                "size": 2869504,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-aarch64-macos/wasmtime",
                "size": 59999056,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-aarch64-macos/README.md",
                "size": 7674,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "wasmtime-v49.0.2-aarch64-macos/wasmtime",
    "executable_sha256": "4408e44af098f7581366b0396088924d7e181720b9309574b4c30bac1c86b6dc",
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
    "sha256": "ea5a79261caa39819b863fd850879428cf90548353e1e5cf9bcbcc4e7addf2c1",
    "size": 9565456,
    "tool": "wasmtime",
    "upstream_version": "49.0.2",
    "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-aarch64-macos.tar.xz",
}
