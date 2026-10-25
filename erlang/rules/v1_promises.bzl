"""V1 acceptance promises owned by //erlang/rules."""

V1_PROMISES = [
    {
        "evidence": [
            "//erlang/tests/starter:starter_test",
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.erlang",
        "native": False,
        "owner": "//erlang/rules:defs.bzl",
        "state": "evidenced",
    },
]
