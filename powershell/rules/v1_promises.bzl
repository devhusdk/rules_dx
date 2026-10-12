"""V1 acceptance promises owned by //powershell/rules."""

V1_PROMISES_POWERSHELL_RULES = [
    {
        "evidence": [
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.powershell",
        "native": False,
        "owner": "//powershell/rules:defs.bzl",
        "state": "evidenced",
    },
]
