"""Unit and analysis tests for the GitHub Release publisher."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":validate_github.bzl", "github_asset_collision_error", "github_notes_error", "github_tag_error")

def github_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "github_tag_error accepts the dry-run placeholder tag",
                github_tag_error("v0.0.0-dryrun"),
                "",
            ),
            expect_equal(
                "github_tag_error accepts dotted release tags",
                github_tag_error("v1.2.3"),
                "",
            ),
            expect_equal(
                "github_tag_error rejects an empty tag",
                github_tag_error(""),
                "github_deploy: invalid release_tag '': want a non-empty tag " +
                "(for example 'v0.0.0-dryrun')",
            ),
            expect_equal(
                "github_tag_error rejects shell-unsafe tag characters",
                github_tag_error("v1.0;curl evil"),
                "github_deploy: invalid release_tag 'v1.0;curl evil': want only [A-Za-z0-9._-]",
            ),
            expect_equal(
                "github_tag_error rejects current, which tag_from_head replaces",
                github_tag_error("current"),
                "github_deploy: invalid release_tag 'current': use tag_from_head = True",
            ),
            expect_equal(
                "github_tag_error rejects a tag gh would read as a flag",
                github_tag_error("--prerelease"),
                "github_deploy: invalid release_tag '--prerelease': want a git tag name: no leading '-' or '.', no '..', no trailing '.' or '.lock'",
            ),
            expect_equal(
                "github_tag_error rejects a leading dot",
                github_tag_error(".v1"),
                "github_deploy: invalid release_tag '.v1': want a git tag name: no leading '-' or '.', no '..', no trailing '.' or '.lock'",
            ),
            expect_equal(
                "github_tag_error rejects a double dot",
                github_tag_error("v1..2"),
                "github_deploy: invalid release_tag 'v1..2': want a git tag name: no leading '-' or '.', no '..', no trailing '.' or '.lock'",
            ),
            expect_equal(
                "github_tag_error rejects a trailing dot",
                github_tag_error("v1."),
                "github_deploy: invalid release_tag 'v1.': want a git tag name: no leading '-' or '.', no '..', no trailing '.' or '.lock'",
            ),
            expect_equal(
                "github_tag_error rejects a .lock suffix",
                github_tag_error("v1.lock"),
                "github_deploy: invalid release_tag 'v1.lock': want a git tag name: no leading '-' or '.', no '..', no trailing '.' or '.lock'",
            ),
            expect_equal(
                "github_notes_error accepts commits",
                github_notes_error("commits"),
                "",
            ),
            expect_equal(
                "github_notes_error accepts none with a notes_file",
                github_notes_error("none", "//docs:notes.md"),
                "",
            ),
            expect_equal(
                "github_notes_error rejects a label",
                github_notes_error("//docs:notes.md"),
                "github_deploy: invalid notes '//docs:notes.md': want 'commits' or 'none'; use notes_file for a file",
            ),
            expect_equal(
                "github_notes_error rejects commits with a notes_file",
                github_notes_error("commits", "//docs:notes.md"),
                "github_deploy: notes_file needs notes = 'none'",
            ),
            expect_equal(
                "github_asset_collision_error accepts unique asset names",
                github_asset_collision_error([
                    ("//linux:app", "app-linux.tar.gz"),
                    ("//mac:app", "app-mac.tar.gz"),
                ]),
                "",
            ),
            expect_equal(
                "github_asset_collision_error names both artifacts that share an asset name",
                github_asset_collision_error([
                    ("//linux:app.tar.gz", "app.tar.gz"),
                    ("//mac:app.tar.gz", "app.tar.gz"),
                ]),
                "github_deploy: artifacts //linux:app.tar.gz and " +
                "//mac:app.tar.gz share the asset name 'app.tar.gz'",
            ),
        ],
    )

def _attested_config_test_impl(ctx):
    env = analysistest.begin(ctx)
    writes = [a for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite"]
    asserts.equals(env, 1, len(writes), "want one FileWrite action")
    config = json.decode(writes[0].content)["config"] if writes else {}
    attestations = config.get("attestations", [])
    asserts.equals(env, ["crates", "maven"], [a["registry"] for a in attestations])
    asserts.equals(env, "crates_demo", attestations[0]["crate_name"] if attestations else "")
    maven_rlocs = attestations[1]["rlocs"] if len(attestations) > 1 else []
    asserts.equals(env, 4, len(maven_rlocs), "want the jar, pom, sources and javadoc")
    for rloc in maven_rlocs:
        asserts.true(env, rloc in config.get("asset_rlocs", []), "attach must upload " + rloc)
    return analysistest.end(env)

_attested_config_test = analysistest.make(_attested_config_test_impl)

def github_attestation_tests(name, size):
    """Checks that registry_deploys reach the github config and attach uploads the maven files."""
    _attested_config_test(
        name = name,
        size = size,
        target_under_test = ":github_attested_config",
    )
