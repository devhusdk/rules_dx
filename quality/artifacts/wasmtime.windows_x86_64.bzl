"""wasmtime standalone artifact metadata (windows_x86_64) -- GENERATED, do not edit."""

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
                "name": "wasmtime-v49.0.2-x86_64-windows/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-x86_64-windows/LICENSE",
                "size": 12243,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-x86_64-windows/README.md",
                "size": 7674,
            },
            {
                "is_executable": True,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-x86_64-windows/wasmtime-min.exe",
                "size": 2087424,
            },
            {
                "is_executable": True,
                "mode": "0o644",
                "name": "wasmtime-v49.0.2-x86_64-windows/wasmtime.exe",
                "size": 41020928,
            },
        ],
    },
    "checksum_source": "maintainer-established byte identity (upstream publishes no asset digests)",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "wasmtime-v49.0.2-x86_64-windows/wasmtime.exe",
    "executable_sha256": "51165ae46e2e4ca71b52efff33432d9e58db41bf236cb7c768adb03f653a7152",
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
    "sha256": "c95a3925acde6398808baabe5ad13d0b1df02b004bc256c00cf8e2a36daccea8",
    "size": 13330289,
    "tool": "wasmtime",
    "upstream_version": "49.0.2",
    "url": "https://github.com/bytecodealliance/wasmtime/releases/download/v49.0.2/wasmtime-v49.0.2-x86_64-windows.zip",
}
