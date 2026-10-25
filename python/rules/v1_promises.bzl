"""V1 acceptance promises owned by //python/rules."""

V1_PROMISES = [
    {
        "evidence": [
            "//python/tests/consumer:service_consumer_test",
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.python",
        "native": False,
        "owner": "//python/rules:defs.bzl",
        "state": "evidenced",
    },
]
