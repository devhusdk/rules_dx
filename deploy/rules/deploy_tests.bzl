"""Unit and analysis tests for the deploy boundary."""

load("//libs/starlark:defs.bzl", "DxConfigInfo", "expect_equal", "starlark_test")
load("//libs/starlark:failure_test.bzl", "failure_test")
load(":defs.bzl", "VALID_DEPLOY_PROFILES", "deploy_empty_artifact_error", "deploy_profile_error")

def deploy_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "profile vocabulary pins debug, dev, release",
                VALID_DEPLOY_PROFILES,
                ["debug", "dev", "release"],
            ),
            expect_equal(
                "deploy_profile_error accepts every valid profile",
                [
                    deploy_profile_error("debug"),
                    deploy_profile_error("dev"),
                    deploy_profile_error("release"),
                ],
                ["", "", ""],
            ),
            expect_equal(
                "deploy_profile_error accepts None as command default",
                deploy_profile_error(None),
                "",
            ),
            expect_equal(
                "deploy_profile_error rejects an unknown profile",
                deploy_profile_error("staging"),
                "dx_deployment: invalid profile 'staging': want one of debug, dev, release",
            ),
            expect_equal(
                "deploy_profile_error rejects an empty profile",
                deploy_profile_error(""),
                "dx_deployment: invalid profile '': want one of debug, dev, release",
            ),
            expect_equal(
                "deploy_empty_artifact_error accepts an artifact with files",
                deploy_empty_artifact_error("//deploy/rules:web_dir", 2),
                "",
            ),
            expect_equal(
                "deploy_empty_artifact_error rejects an artifact with no files",
                deploy_empty_artifact_error("//deploy/rules:empty_artifact", 0),
                "dx_deployment: artifact //deploy/rules:empty_artifact provides no files: want declared output files or directories",
            ),
        ],
    )

def _deploy_config_probe_impl(ctx):
    """Exposes which deploy side the probe analyzed under."""
    launcher = ctx.target_platform_has_constraint(ctx.attr._launcher_side[platform_common.ConstraintValueInfo])
    artifact = ctx.target_platform_has_constraint(ctx.attr._artifact_side[platform_common.ConstraintValueInfo])
    if launcher and not artifact:
        side = "launcher"
    elif artifact and not launcher:
        side = "artifact"
    elif launcher and artifact:
        side = "both"
    else:
        side = "neither"
    script = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(script, "#!/bin/sh\nexit 0\n", is_executable = True)
    note = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(note, "side=" + side + "\n")
    return [
        DefaultInfo(
            executable = script,
            files = depset([script, note]),
        ),
        DxConfigInfo(fields = {"side": side}),
    ]

deploy_config_probe = rule(
    implementation = _deploy_config_probe_impl,
    executable = True,
    attrs = {
        "_artifact_side": attr.label(default = ":artifact_side"),
        "_launcher_side": attr.label(default = ":launcher_side"),
    },
)

EXPECTED_DEPLOY_DEFAULT_OBSERVATIONS = """subject //deploy/rules:deploy_default
file deploy_default
field app=
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:deploy_default
aspect_field transitive_count=0
config_field artifact_config=target
config_field launcher_config=exec"""

EXPECTED_DEPLOY_DEBUG_OBSERVATIONS = """subject //deploy/rules:deploy_debug
file deploy_debug
field app=
field artifacts=
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:deploy_debug
aspect_field transitive_count=0
config_field artifact_config=target
config_field launcher_config=exec"""

EXPECTED_DEPLOY_WITH_APP_OBSERVATIONS = """subject //deploy/rules:deploy_with_app
file deploy_with_app
field app=//deploy/rules:deploy_program
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:deploy_with_app
aspect_field transitive_count=0
config_field artifact_config=target
config_field launcher_config=exec"""

EXPECTED_DEPLOY_WITH_ARTIFACT_OBSERVATIONS = """subject //deploy/rules:deploy_with_artifact
file deploy_with_artifact
file index.html
file site.css
field app=
field artifacts=//deploy/rules:web_dir
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:deploy_with_artifact
aspect_field transitive_count=0
config_field artifact_config=target
config_field launcher_config=exec"""

EXPECTED_DEPLOY_SPLIT_CONFIG_OBSERVATIONS = """subject //deploy/rules:deploy_split_config
file deploy_split_config.sh
file split_artifact.sh
file split_artifact.txt
field app=
field artifacts=//deploy/rules:split_artifact
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:deploy_split_config
aspect_field transitive_count=0
config_field artifact_config=target
config_field artifact_side=artifact
config_field launcher_config=exec
config_field launcher_side=launcher"""

def deploy_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":deploy_default"],
        expected_observations = EXPECTED_DEPLOY_DEFAULT_OBSERVATIONS,
    )

def deploy_debug_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":deploy_debug"],
        expected_observations = EXPECTED_DEPLOY_DEBUG_OBSERVATIONS,
    )

def deploy_app_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":deploy_with_app"],
        expected_observations = EXPECTED_DEPLOY_WITH_APP_OBSERVATIONS,
    )

def deploy_artifact_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":deploy_with_artifact"],
        expected_observations = EXPECTED_DEPLOY_WITH_ARTIFACT_OBSERVATIONS,
    )

def deploy_split_config_analysis_tests(name):
    """Proves the launcher and artifact configuration split under --platforms=//deploy/rules:artifact_platform."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":deploy_split_config"],
        expected_observations = EXPECTED_DEPLOY_SPLIT_CONFIG_OBSERVATIONS,
        tags = ["manual"],
    )

def deploy_empty_artifact_failure_tests(name):
    """Asserts an artifact with no files fails analysis with the artifact diagnostic."""
    failure_test(
        name = name,
        target = ":deploy_empty_artifact",
        expected_failure_substring = "dx_deployment: artifact //deploy/rules:empty_artifact provides no files",
    )
