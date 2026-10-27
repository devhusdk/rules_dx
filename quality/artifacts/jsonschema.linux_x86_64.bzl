"""jsonschema standalone artifact metadata (linux_x86_64) -- GENERATED, do not edit."""

# buildifier: disable=attr-licenses
ARTIFACT = {
    "abi_floor": {
        "kernel": "3.2.0",
        "libc": "2.35",
        "libstdcxx": None,
    },
    "archive": {
        "format": "zip",
        "members": [
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-x86_64/share/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-x86_64/share/zsh/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-x86_64/share/zsh/site-functions/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "jsonschema-17.3.1-linux-x86_64/share/zsh/site-functions/_jsonschema",
                "size": 10523,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-x86_64/share/bash-completion/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-x86_64/share/bash-completion/completions/",
                "size": 0,
            },
            {
                "is_executable": False,
                "mode": "0o644",
                "name": "jsonschema-17.3.1-linux-x86_64/share/bash-completion/completions/jsonschema",
                "size": 8728,
            },
            {
                "is_executable": False,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-x86_64/bin/",
                "size": 0,
            },
            {
                "is_executable": True,
                "mode": "0o755",
                "name": "jsonschema-17.3.1-linux-x86_64/bin/jsonschema",
                "size": 15080504,
            },
        ],
    },
    "checksum_source": "upstream published checksums file",
    "cpu": "x86_64",
    "delivery_class": "standalone",
    "executable": "jsonschema-17.3.1-linux-x86_64/bin/jsonschema",
    "executable_sha256": "c89b9dfc3b36dc0806a2c5a086fc44976fca843b372cf3d7a2bc2c607203c4a7",
    "interpreter": "/lib64/ld-linux-x86-64.so.2",
    "licenses": [
        {
            "name": "AGPL-3.0",
            "source": "https://github.com/sourcemeta/jsonschema/blob/v17.3.1/LICENSE",
        },
    ],
    "linkage": "dynamic",
    "needed_shared_libraries": [
        "ld-linux-x86-64.so.2",
        "libc.so.6",
        "libm.so.6",
    ],
    "os": "linux",
    "runtime_files": [],
    "schema_version": 1,
    "sha256": "a857fcfe35e3f1506a256192be9704dc8e0a8983a39519bbf4ff9e68b17d4550",
    "size": 5734396,
    "tool": "jsonschema",
    "upstream_version": "17.3.1",
    "url": "https://github.com/sourcemeta/jsonschema/releases/download/v17.3.1/jsonschema-17.3.1-linux-x86_64.zip",
}
