"""V1 acceptance promises owned by //terraform/rules."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
            "//quality/tests:terraform_consumer_test",
        ],
        "id": "family.terraform",
        "native": False,
        "owner": "//terraform/rules:defs.bzl",
        "state": "evidenced",
    },
]
