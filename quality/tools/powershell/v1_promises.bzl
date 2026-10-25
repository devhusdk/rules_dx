"""V1 acceptance promises owned by //quality/tools/powershell."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:powershell_consumer_test",
        ],
        "id": "tool.psscriptanalyzer",
        "native": True,
        "owner": "//quality/tools/powershell:psscriptanalyzer",
        "state": "evidenced",
    },
]
