"""V1 acceptance promises owned by //quality/tools/jvm."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.checkstyle",
        "native": True,
        "owner": "//quality/tools/jvm:checkstyle",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.google_java_format",
        "native": True,
        "owner": "//quality/tools/jvm:google_java_format",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.ktfmt",
        "native": True,
        "owner": "//quality/tools/jvm:ktfmt",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.ktlint",
        "native": True,
        "owner": "//quality/tools/jvm:ktlint",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.pmd",
        "native": True,
        "owner": "//quality/tools/jvm:pmd",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:scalafix_consumer_test",
            "//tools/testing/hermetic:scala_scalafix",
        ],
        "id": "tool.scalafix",
        "native": True,
        "owner": "//quality/tools/jvm:scalafix",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.spotbugs",
        "native": True,
        "owner": "//quality/tools/jvm:spotbugs",
        "state": "evidenced",
    },
]
