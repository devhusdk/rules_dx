"""wasmtime standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.2.0",
        "libc": "2.28",
        "libstdcxx": None,
    },
    "archive": {
        "format": "tar.xz",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-x86_64-linux",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-x86_64-linux/LICENSE",
                "size": 12243,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-x86_64-linux/wasmtime-min",
                "size": 3419488,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "wasmtime-v49.0.2-x86_64-linux/wasmtime",
                "size": 71537472,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-x86_64-linux/README.md",
                "size": 7674,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "wasmtime-v49.0.2-x86_64-linux/wasmtime",
    "executable_sha256": "d0a014e0d5b0cf48dd3549c38e2e6ecd78ff09fb3e3f10d751b87b72bdfd8635",
    "interpreter": "/lib64/ld-linux-x86-64.so.2",
    "licenses": [
        {
            "name": "Apache-2.0 WITH LLVM-exception",
            "source": "https://github.com/bytecodealliance/wasmtime/blob/v49.0.2/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "ld-linux-x86-64.so.2",
        "libc.so.6",
        "libdl.so.2",
        "libgcc_s.so.1",
        "libm.so.6",
        "libpthread.so.0",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "a4d6e9e3a5a60f527cf7793d674c48930c80c2e8977995b8a275cad3254b9322",
    "size": 11635696,
    "tool": "wasmtime",
    "upstream_version": "49.0.2",
    "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-x86_64-linux.tar.xz",
}
