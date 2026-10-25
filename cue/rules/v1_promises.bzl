"""V1 acceptance promises owned by //cue/rules."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
            "//quality/tests:cue_consumer_test",
        ],
        "id": "family.cue",
        "native": False,
        "owner": "//cue/rules:defs.bzl",
        "state": "evidenced",
    },
]
