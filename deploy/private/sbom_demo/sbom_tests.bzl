"""Analysis test: sbom must be a dx_sbom target."""

load("//core:failure_test.bzl", "failure_test")
load("//deploy/private:config.bzl", "DEPLOY_TOOL")
load("//deploy/private:deployment.bzl", "dx_deployment")

def plain_file_sbom_tests(name):
    """Declares a deployment whose sbom is a plain file and the test that expects it to fail."""
    dx_deployment(
        name = name + "_target",
        sbom = "app.rs",
        config = ":app_wheel_config",
        deploy = DEPLOY_TOOL,
        build_mode = "release",
        tags = ["manual"],
    )
    failure_test(
        name = name,
        target = ":" + name + "_target",
        expected_failure_substring = "is misplaced here",
    )
