"""Unit and analysis tests for the deploy boundary."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":defs.bzl", "VALID_DEPLOY_PROFILES")

def deploy_observations(label, app = "", artifacts = [], profile = "release"):
    """Renders the expected analysis observations for one dx_deployment subject."""
    fields = {
        "app": app,
        "artifacts": ",".join(artifacts),
        "profile": profile,
    }
    lines = ["subject " + label, "file " + label.split(":")[-1]]
    for key in sorted(fields.keys()):
        lines.append("field " + key + "=" + fields[key])
    lines.append("aspect_field aspect_seen=True")
    lines.append("aspect_field field_count=" + str(len(fields)))
    lines.append("aspect_field has_subject=True")
    lines.append("aspect_field subject_label=" + label)
    lines.append("aspect_field transitive_count=0")
    return "\n".join(lines)

def deploy_analysis_test(name, subject, app = "", artifacts = [], profile = "release"):
    """Checks the DxDeployInfo one dx_deployment subject in this package exposes."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":" + subject],
        expected_observations = deploy_observations(
            "//" + native.package_name() + ":" + subject,
            app = app,
            artifacts = artifacts,
            profile = profile,
        ),
    )

def deploy_unit_tests(name):
    """Checks the deploy profile vocabulary and the observation renderer."""
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
                "deploy_observations renders a default subject",
                deploy_observations("//deploy/rules:deploy_default"),
                """subject //deploy/rules:deploy_default
file deploy_default
field app=
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:deploy_default
aspect_field transitive_count=0""",
            ),
            expect_equal(
                "deploy_observations keeps declared artifact order",
                deploy_observations(
                    "//pkg:demo",
                    app = "//pkg:app",
                    artifacts = ["//pkg:z", "//pkg:a"],
                    profile = "debug",
                ),
                """subject //pkg:demo
file demo
field app=//pkg:app
field artifacts=//pkg:z,//pkg:a
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//pkg:demo
aspect_field transitive_count=0""",
            ),
        ],
    )
