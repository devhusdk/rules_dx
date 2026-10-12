"""V1 acceptance promises owned by //deploy/rules."""

V1_PROMISES_DEPLOY_RULES = [
    {
        "evidence": [
            "//deploy/rules:archive_unit",
        ],
        "id": "artifact.standalone_archive",
        "native": True,
        "owner": "//deploy/rules:archive.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:archive_unit",
            "//deploy/rules:archive_analysis",
        ],
        "id": "deploy.rule_archive",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:crates_unit",
            "//deploy/rules:crates_analysis",
        ],
        "id": "deploy.rule_crates",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:deploy_unit",
            "//deploy/rules:deploy_analysis",
        ],
        "id": "deploy.rule_deploy",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:deploy_app_analysis",
        ],
        "id": "deploy.rule_deploy_app",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:github_unit",
            "//deploy/rules:github_analysis",
        ],
        "id": "deploy.rule_github",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:maven_unit",
            "//deploy/rules:maven_analysis",
        ],
        "id": "deploy.rule_maven",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:npm_unit",
            "//deploy/rules:npm_analysis",
        ],
        "id": "deploy.rule_npm",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:nuget_unit",
            "//deploy/rules:nuget_analysis",
        ],
        "id": "deploy.rule_nuget",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:oci_unit",
            "//deploy/rules:oci_analysis",
        ],
        "id": "deploy.rule_oci",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:octopus_unit",
            "//deploy/rules:octopus_analysis",
        ],
        "id": "deploy.rule_octopus",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:promotion_unit",
            "//deploy/rules:promotion_analysis",
        ],
        "id": "deploy.rule_promotion",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
    {
        "evidence": [
            "//deploy/rules:pypi_unit",
            "//deploy/rules:pypi_analysis",
        ],
        "id": "deploy.rule_pypi",
        "native": False,
        "owner": "//deploy/rules:defs.bzl",
        "state": "evidenced",
    },
]
