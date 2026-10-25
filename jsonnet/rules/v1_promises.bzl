"""V1 acceptance promises owned by //jsonnet/rules."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
            "//quality/tests:jsonnet_consumer_test",
        ],
        "id": "family.jsonnet",
        "native": False,
        "owner": "//jsonnet/rules:defs.bzl",
        "state": "evidenced",
    },
]
