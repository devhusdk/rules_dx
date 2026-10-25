"""V1 acceptance promises owned by //quality/tools/dotnet."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:csharp_consumer_test",
        ],
        "id": "tool.csharpier",
        "native": True,
        "owner": "//quality/tools/dotnet:csharpier",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:fsharp_consumer_test",
        ],
        "id": "tool.fantomas",
        "native": True,
        "owner": "//quality/tools/dotnet:fantomas",
        "state": "evidenced",
    },
]
