"""V1 acceptance promises owned by //deploy/release."""

V1_PROMISES = [
    {
        "evidence": [
            "//deploy/release:bcr_unit",
        ],
        "id": "artifact.bcr_source",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/release:matrix_unit",
        ],
        "id": "artifact.matrix",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/release:notice_unit",
        ],
        "id": "artifact.notice",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/release:sbom_unit",
        ],
        "id": "artifact.notice_sbom_prov",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/release:sbom_unit",
        ],
        "id": "artifact.notice_sbom_spdx",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/release:release_artifacts_verify",
        ],
        "id": "deploy.release",
        "native": False,
        "owner": "//deploy/release",
        "state": "evidenced",
    },
]
