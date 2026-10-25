"""V1 acceptance promises owned by //zig/rules."""

V1_PROMISES = [
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
            "//zig/rules:wrapper_contract_tests",
        ],
        "id": "family.zig",
        "native": False,
        "owner": "//zig/rules:defs.bzl",
        "state": "evidenced",
    },
]
