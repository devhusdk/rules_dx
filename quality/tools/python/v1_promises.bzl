"""V1 acceptance promises owned by //quality/tools/python."""

V1_PROMISES_QUALITY_TOOLS_PYTHON = [
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.flake8",
        "native": True,
        "owner": "//quality/tools/python:flake8",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.pydoclint",
        "native": True,
        "owner": "//quality/tools/python:pydoclint",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
        ],
        "id": "tool.pylint",
        "native": True,
        "owner": "//quality/tools/python:pylint",
        "state": "evidenced",
    },
]
