"""Unit and analysis tests for the deploy boundary."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":defs.bzl", "VALID_DEPLOY_BUILD_MODES")

def deploy_observations(label, app = "", artifacts = [], build_mode = "release"):
    """Renders the expected analysis observations for one dx_deployment subject."""
    fields = {
        "app": app,
        "artifacts": ",".join(artifacts),
        "build_mode": build_mode,
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

def deploy_analysis_test(name, subject, app = "", artifacts = [], build_mode = "release"):
    """Checks the DxDeployInfo one dx_deployment subject in this package exposes."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":" + subject],
        expected_observations = deploy_observations(
            "//" + native.package_name() + ":" + subject,
            app = app,
            artifacts = artifacts,
            build_mode = build_mode,
        ),
    )

def deploy_unit_tests(name):
    """Checks the build mode vocabulary and the observation renderer."""
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "build mode vocabulary pins debug, dev, release",
                VALID_DEPLOY_BUILD_MODES,
                ["debug", "dev", "release"],
            ),
            expect_equal(
                "deploy_observations renders a default subject",
                deploy_observations("//deploy/rules:deploy_default"),
                """subject //deploy/rules:deploy_default
file deploy_default
field app=
field artifacts=
field build_mode=release
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
                    build_mode = "debug",
                ),
                """subject //pkg:demo
file demo
field app=//pkg:app
field artifacts=//pkg:z,//pkg:a
field build_mode=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//pkg:demo
aspect_field transitive_count=0""",
            ),
        ],
    )

_RELEASE_DEMO_CONFIG = "_main/deploy/rules/release_demo_config.deploy.json"

def _config_env_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, RunEnvironmentInfo in target, "a configured deployment must set its run environment")
    if RunEnvironmentInfo in target:
        asserts.equals(
            env,
            {"DX_DEPLOY_CONFIG": _RELEASE_DEMO_CONFIG},
            target[RunEnvironmentInfo].environment,
        )
    runfiles = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list()]
    asserts.true(
        env,
        "deploy/rules/release_demo_config.deploy.json" in runfiles,
        "the config must ship in the deployment runfiles",
    )
    return analysistest.end(env)

_config_env_test = analysistest.make(_config_env_test_impl)

def _no_config_env_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.false(env, RunEnvironmentInfo in target, "a deployment without config sets no run environment")
    return analysistest.end(env)

_no_config_env_test = analysistest.make(_no_config_env_test_impl)

def deploy_config_tests(name):
    """Checks that dx_deployment exposes its config through DX_DEPLOY_CONFIG."""
    _config_env_test(
        name = name + "_env",
        size = "small",
        target_under_test = ":release_demo",
    )
    _no_config_env_test(
        name = name + "_none",
        size = "small",
        target_under_test = ":deploy_default",
    )
    native.test_suite(
        name = name,
        tests = [name + "_env", name + "_none"],
    )

def _config_runfiles_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    want = "_main/deploy/rules/" + ctx.attr.config_name + ".deploy.json"
    asserts.true(env, RunEnvironmentInfo in target, "a configured deployment must set its run environment")
    if RunEnvironmentInfo in target:
        asserts.equals(
            env,
            {"DX_DEPLOY_CONFIG": want},
            target[RunEnvironmentInfo].environment,
        )
    runfiles = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list()]
    for pin in ctx.attr.pins:
        asserts.true(
            env,
            len([p for p in runfiles if p.endswith("/" + pin)]) > 0,
            "the pinned artifact " + pin + " must ship in the runfiles: " + ", ".join(runfiles),
        )
    return analysistest.end(env)

_config_runfiles_test = analysistest.make(
    _config_runfiles_test_impl,
    attrs = {
        "config_name": attr.string(mandatory = True),
        "pins": attr.string_list(mandatory = True),
    },
)

def config_runfiles_tests(name, demos):
    """Checks that each demo deployment names its config and ships its pinned artifacts.

    Args:
      name: test_suite name.
      demos: dict of deployment name to the basenames of the artifacts it pins.
    """
    tests = []
    for demo, pins in demos.items():
        test = name + "_" + demo.replace(".", "_")
        _config_runfiles_test(
            name = test,
            size = "small",
            target_under_test = ":" + demo,
            config_name = demo + "_config",
            pins = pins,
        )
        tests.append(test)
    native.test_suite(
        name = name,
        tests = tests,
    )
