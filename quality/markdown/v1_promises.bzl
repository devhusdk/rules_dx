"""V1 acceptance promises owned by //quality/markdown."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.markdown_check",
        "native": True,
        "owner": "//quality/markdown:quality_markdown",
        "state": "evidenced",
    },
]
