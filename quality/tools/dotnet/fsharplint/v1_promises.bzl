"""V1 acceptance promises owned by //quality/tools/dotnet/fsharplint."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:fsharp_consumer_test",
        ],
        "id": "tool.fsharplint",
        "native": True,
        "owner": "//quality/tools/dotnet/fsharplint",
        "state": "evidenced",
    },
]
