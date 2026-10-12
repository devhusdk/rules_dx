"""V1 acceptance promises owned by //rust/toolchains."""

V1_PROMISES_RUST_TOOLCHAINS = [
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.clippy",
        "native": True,
        "owner": "//rust/toolchains:bindings.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.rustc",
        "native": True,
        "owner": "//rust/toolchains:bindings.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.rustfmt",
        "native": True,
        "owner": "//rust/toolchains:bindings.bzl",
        "state": "evidenced",
    },
]
