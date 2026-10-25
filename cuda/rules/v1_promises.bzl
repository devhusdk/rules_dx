"""V1 acceptance promises owned by //cuda/rules."""

V1_PROMISES = [
    {
        "evidence": [
            "//cuda/rules:wrapper_contract_tests",
            "//cuda/testing:gpu_facts_test",
            "//cuda/tests:saxpy_device_code_test",
            "//cuda/tests:saxpy_gpu_test",
            "//cuda/tests:saxpy_gpu_unavailable_test",
            "//quality:wrapper_owners_unit",
        ],
        "id": "family.cuda",
        "native": False,
        "owner": "//cuda/rules:defs.bzl",
        "state": "evidenced",
    },
]
