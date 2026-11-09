"""Unit and analysis tests for the pub.dev publisher."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":api_compat.bzl", "api_compat_error")
load(":validate_pub.bzl", "PUB_DEFAULT_REGISTRY_URL", "pub_registry_url_error", "pub_name_error", "pub_version_error")

def pub_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "pub_name_error accepts the demo package name",
                pub_name_error("pub_demo"),
                "",
            ),
            expect_equal(
                "pub_name_error rejects an empty name",
                pub_name_error(""),
                "pub_deploy: invalid package name '': want a non-empty " +
                "name (for example 'pub_demo')",
            ),
            expect_equal(
                "pub_name_error rejects dashes and capitals",
                pub_name_error("Pub-Demo"),
                "pub_deploy: invalid package name 'Pub-Demo': want only [a-z0-9_]",
            ),
            expect_equal(
                "pub_version_error accepts prerelease versions",
                pub_version_error("1.2.3-beta.1"),
                "",
            ),
            expect_equal(
                "pub_version_error rejects version characters",
                pub_version_error("1.0;curl evil"),
                "pub_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+]",
            ),
            expect_equal(
                "pub_registry_url_error accepts the default server",
                pub_registry_url_error(PUB_DEFAULT_REGISTRY_URL),
                "",
            ),
            expect_equal(
                "pub_registry_url_error rejects plaintext URLs",
                pub_registry_url_error("http://pub.example.com"),
                "pub_deploy: invalid registry_url 'http://pub.example.com': " +
                "want an https URL so publishes never go over plaintext",
            ),
            expect_equal(
                "pub_deploy accepts the default api_compat and api_baseline",
                api_compat_error("pub_deploy", "fail", "registry"),
                "",
            ),
            expect_equal(
                "pub_deploy rejects an unknown api_compat",
                api_compat_error("pub_deploy", "loud", "registry"),
                "pub_deploy: invalid api_compat 'loud': want 'off', 'warn' or 'fail'",
            ),
            expect_equal(
                "pub_deploy rejects an unknown api_baseline",
                api_compat_error("pub_deploy", "fail", "tag"),
                "pub_deploy: invalid api_baseline 'tag': want 'registry' or 'git'",
            ),
        ],
    )

def _config_content_test_impl(ctx):
    env = analysistest.begin(ctx)
    writes = [a for a in analysistest.target_actions(env) if a.mnemonic == "FileWrite"]
    asserts.equals(env, 1, len(writes), "want one FileWrite action")
    content = writes[0].content if writes else ""
    asserts.true(env, "\"api_compat\":\"fail\"" in content, "config must default api_compat to fail: " + content)
    asserts.true(env, "\"api_baseline\":\"registry\"" in content, "config must default api_baseline to registry: " + content)
    asserts.true(env, "dart_apitool.dill" in content, "config must carry the dart_apitool kernel: " + content)
    asserts.true(env, "flutter_dart_rloc" in content, "config must carry the pinned flutter dart: " + content)
    asserts.true(env, "flutter.version.json" in content, "config must carry the flutter root: " + content)
    return analysistest.end(env)

_config_content_test = analysistest.make(_config_content_test_impl)

def pub_config_content_tests(name):
    """Checks that the pub config defaults api_compat to fail against the registry."""
    _config_content_test(
        name = name,
        size = "small",
        target_under_test = ":pub_demo_config",
    )
