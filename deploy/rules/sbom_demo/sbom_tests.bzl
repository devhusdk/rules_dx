"""Analysis test: sbom must be a dx_sbom target."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//deploy/rules:config.bzl", "DEPLOY_TOOL")
load("//deploy/rules:defs.bzl", "dx_deployment")

def _plain_file_rejected_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "is misplaced here")
    return analysistest.end(env)

plain_file_sbom_test = analysistest.make(
    _plain_file_rejected_impl,
    expect_failure = True,
)

def plain_file_sbom_tests(name):
    """Declares a deployment whose sbom is a plain file and the test that expects it to fail."""
    dx_deployment(
        name = name + "_target",
        sbom = "app.rs",
        config = ":app_wheel_config",
        deploy = DEPLOY_TOOL,
        profile = "release",
        tags = ["manual"],
    )
    plain_file_sbom_test(
        name = name,
        size = "small",
        target_under_test = ":" + name + "_target",
    )
