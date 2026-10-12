"""V1 acceptance promises owned by //quality/tools/javascript/bin."""

V1_PROMISES_QUALITY_TOOLS_JAVASCRIPT_BIN = [
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.eslint",
        "native": True,
        "owner": "//quality/tools/javascript/bin:eslint",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.prettier",
        "native": True,
        "owner": "//quality/tools/javascript/bin:prettier",
        "state": "evidenced",
    },
]
