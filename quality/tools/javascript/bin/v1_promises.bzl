"""V1 acceptance promises owned by //quality/tools/javascript/bin."""

V1_PROMISES = [
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
            "//quality/tests:prettier_consumer_test",
        ],
        "id": "tool.prettier",
        "native": True,
        "owner": "//quality/tools/javascript/bin:prettier",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:stylelint_consumer_test",
        ],
        "id": "tool.stylelint",
        "native": True,
        "owner": "//quality/tools/javascript/bin:stylelint",
        "state": "evidenced",
    },
]
