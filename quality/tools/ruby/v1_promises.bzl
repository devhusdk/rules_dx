"""V1 acceptance promises owned by //quality/tools/ruby."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:ruby_consumer_test",
        ],
        "id": "tool.rubocop",
        "native": True,
        "owner": "//quality/tools/ruby:rubocop",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:ruby_consumer_test",
        ],
        "id": "tool.standardrb",
        "native": True,
        "owner": "//quality/tools/ruby:standardrb",
        "state": "evidenced",
    },
]
