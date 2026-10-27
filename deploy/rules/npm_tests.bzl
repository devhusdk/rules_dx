"""Unit and analysis tests for the npm pack feed publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":npm.bzl", "npm_access_error", "npm_effective_provenance", "npm_filenames", "npm_package_error", "npm_provenance_error", "npm_registry_error", "npm_tag_error")

def npm_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "npm_access_error accepts public unscoped",
                npm_access_error("npm-demo", "public"),
                "",
            ),
            expect_equal(
                "npm_access_error accepts restricted scoped",
                npm_access_error("@scope/npm-demo", "restricted"),
                "",
            ),
            expect_equal(
                "npm_access_error rejects restricted unscoped",
                npm_access_error("npm-demo", "restricted"),
                "npm_deploy: access 'restricted' needs a scoped package, got unscoped 'npm-demo'",
            ),
            expect_equal(
                "npm_access_error rejects an unknown access",
                npm_access_error("npm-demo", "private"),
                "npm_deploy: invalid access 'private': want 'public' or 'restricted'",
            ),
            expect_equal(
                "npm_access_error rejects a malformed scope",
                npm_access_error("@scope", "public"),
                "npm_deploy: invalid package '@scope': a scoped name is @scope/name",
            ),
            expect_equal(
                "npm_provenance_error accepts required for public npmjs",
                npm_provenance_error("public", "https://registry.npmjs.org", "required"),
                "",
            ),
            expect_equal(
                "npm_provenance_error accepts none for restricted",
                npm_provenance_error("restricted", "https://npm.example.com", "none"),
                "",
            ),
            expect_equal(
                "npm_provenance_error rejects required for restricted",
                npm_provenance_error("restricted", "https://registry.npmjs.org", "required"),
                "npm_deploy: provenance 'required' is not supported with access 'restricted'; use access 'public' or provenance 'none'",
            ),
            expect_equal(
                "npm_provenance_error rejects required for another registry",
                npm_provenance_error("public", "https://npm.example.com", "required"),
                "npm_deploy: provenance 'required' is not supported by registry 'https://npm.example.com'; use https://registry.npmjs.org or provenance 'none'",
            ),
            expect_equal(
                "npm_provenance_error rejects an unknown policy",
                npm_provenance_error("public", "https://registry.npmjs.org", "auto"),
                "npm_deploy: invalid provenance 'auto': want 'required' or 'none'",
            ),
            expect_equal(
                "npm_effective_provenance defaults public to required",
                npm_effective_provenance("public", ""),
                "required",
            ),
            expect_equal(
                "npm_effective_provenance defaults restricted to none",
                npm_effective_provenance("restricted", ""),
                "none",
            ),
            expect_equal(
                "npm_effective_provenance keeps an explicit policy",
                npm_effective_provenance("public", "none"),
                "none",
            ),
            expect_equal(
                "npm_filenames pins the tarball and feed names",
                npm_filenames("npm_demo"),
                ("npm_demo.tgz", "npm_demo.feed.json"),
            ),
            expect_equal(
                "npm_filenames derives names per instance",
                npm_filenames("release"),
                ("release.tgz", "release.feed.json"),
            ),
            expect_equal(
                "npm_package_error accepts a plain package name",
                npm_package_error("npm-demo"),
                "",
            ),
            expect_equal(
                "npm_package_error accepts a scoped package name",
                npm_package_error("@scope/npm-demo"),
                "",
            ),
            expect_equal(
                "npm_package_error rejects an empty package name",
                npm_package_error(""),
                "npm_deploy: invalid package '': want a non-empty " +
                "package name (for example 'npm-demo')",
            ),
            expect_equal(
                "npm_package_error rejects shell-unsafe package characters",
                npm_package_error("bad;pkg"),
                "npm_deploy: invalid package 'bad;pkg': want only " +
                "[A-Za-z0-9@/_.-] so the package name embeds safely " +
                "in the deploy program",
            ),
            expect_equal(
                "npm_tag_error accepts the latest placeholder tag",
                npm_tag_error("latest"),
                "",
            ),
            expect_equal(
                "npm_tag_error accepts dotted prerelease tags",
                npm_tag_error("next"),
                "",
            ),
            expect_equal(
                "npm_tag_error rejects an empty tag",
                npm_tag_error(""),
                "npm_deploy: invalid tag '': want a non-empty tag " +
                "(for example 'latest')",
            ),
            expect_equal(
                "npm_tag_error rejects shell-unsafe tag characters",
                npm_tag_error("v1.0;evil"),
                "npm_deploy: invalid tag 'v1.0;evil': want only " +
                "[A-Za-z0-9._-] so the tag embeds safely in the deploy program",
            ),
            expect_equal(
                "npm_registry_error accepts the default registry",
                npm_registry_error("https://registry.npmjs.org"),
                "",
            ),
            expect_equal(
                "npm_registry_error rejects a non-https registry",
                npm_registry_error("http://registry.npmjs.org"),
                "npm_deploy: invalid registry 'http://registry.npmjs.org': " +
                "want an https:// URL (for example 'https://registry.npmjs.org')",
            ),
            expect_equal(
                "npm_registry_error rejects an empty registry",
                npm_registry_error(""),
                "npm_deploy: invalid registry '': want an https:// URL " +
                "(for example 'https://registry.npmjs.org')",
            ),
        ],
    )
