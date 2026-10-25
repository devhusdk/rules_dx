"""V1 acceptance promises owned by //cc/rules."""

V1_PROMISES = [
    {
        "evidence": [
            "//cc/tests/consumer:app_consumer_test",
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.cc",
        "native": False,
        "owner": "//cc/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.cuda",
        "native": False,
        "owner": "//cc/rules:defs.bzl",
        "state": "evidenced",
    },
]
