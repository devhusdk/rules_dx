"""V1 acceptance promises owned by //quality/tools/python."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:html_template_consumer_test",
        ],
        "id": "tool.djlint",
        "native": True,
        "owner": "//quality/tools/python:djlint",
        "state": "evidenced",
    },
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
    {
        "evidence": [
            "//quality/adapter:quality_adapter_test",
            "//quality:real_pipeline_unit",
            "//quality/tests:yaml_consumer_test",
        ],
        "id": "tool.yamllint",
        "native": True,
        "owner": "//quality/tools/python:yamllint",
        "state": "evidenced",
    },
]
