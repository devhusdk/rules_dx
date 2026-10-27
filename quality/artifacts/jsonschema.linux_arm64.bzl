"""jsonschema standalone artifact metadata (linux_arm64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.7.0",
        "libc": "2.35",
        "libstdcxx": None,
    },
    "archive": {
        "format": "zip",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-arm64/bin/",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-arm64/bin/jsonschema",
                "size": 12805128,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-arm64/share/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-arm64/share/bash-completion/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-arm64/share/bash-completion/completions/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "jsonschema-17.3.1-linux-arm64/share/bash-completion/completions/jsonschema",
                "size": 8728,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-arm64/share/zsh/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-arm64/share/zsh/site-functions/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "jsonschema-17.3.1-linux-arm64/share/zsh/site-functions/_jsonschema",
                "size": 10523,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "arm64",
    "delivery_class": "standalone",
    "executable": "jsonschema-17.3.1-linux-arm64/bin/jsonschema",
    "executable_sha256": "eee02a01fa7bc8de10f408a31d818d008ade616e634896ffdc5b1d552c6eacb0",
    "interpreter": "/lib/ld-linux-aarch64.so.1",
    "licenses": [
        {
            "name": "AGPL-3.0",
            "source": "https://github.com/sourcemeta/jsonschema/blob/v17.3.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "ld-linux-aarch64.so.1",
        "libc.so.6",
        "libm.so.6",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "d42a1e6a28e3e1d9c0c3ffa10d380c90e23e3cfcd0f8278dc3bbe1ad354f4529",
    "size": 5263424,
    "tool": "jsonschema",
    "upstream_version": "17.3.1",
    "url": "https://github.com/sourcemeta/jsonschema/releases/download/v17.3.1/jsonschema-17.3.1-linux-arm64.zip",
}
