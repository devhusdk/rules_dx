"""MSIX pin, license and identity-rule coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":defs.bzl", "alias_errors", "application_errors", "compatible_with", "data_path", "identity_errors", "named_version_errors", "publisher_cn", "publisher_errors", "version_errors")
load(":pins.bzl", "BUILDTOOLS_BIN", "BUILDTOOLS_SHA256", "BUILDTOOLS_URL", "BUILDTOOLS_VERSION", "LICENSE", "SUPPORTED_CELLS", "TOOL_ARCHES")
load(":repos.bzl", "UNACCEPTED", "hub_build", "license_errors")

def _data_path_checks():
    root = struct(workspace_name = "", package = "")
    pkg = struct(workspace_name = "", package = "pkg")
    return [
        expect_equal("root data keeps its subdirectory", data_path(struct(short_path = "assets/logo.png", basename = "logo.png"), root), "assets/logo.png"),
        expect_equal("root data a", data_path(struct(short_path = "a/x.txt", basename = "x.txt"), root), "a/x.txt"),
        expect_equal("root data b", data_path(struct(short_path = "b/x.txt", basename = "x.txt"), root), "b/x.txt"),
        expect_equal("subpackage data", data_path(struct(short_path = "pkg/assets/x", basename = "x"), pkg), "assets/x"),
        expect_equal("other package data", data_path(struct(short_path = "other/x", basename = "x"), pkg), "x"),
        expect_equal("external data in a root target", data_path(struct(short_path = "../repo/x", basename = "x"), root), "x"),
        expect_equal("external package data", data_path(struct(short_path = "../repo/pkg/a/x", basename = "x"), struct(workspace_name = "repo", package = "pkg")), "a/x"),
        expect_equal("external root data", data_path(struct(short_path = "../repo/a/x", basename = "x"), struct(workspace_name = "repo", package = "")), "a/x"),
    ]

def _checks():
    accepted = hub_build(True)
    unaccepted = hub_build(False)
    return _data_path_checks() + [
        expect_equal("build tools version", BUILDTOOLS_VERSION, "10.0.28000.2705"),
        expect_equal("build tools bin dir", BUILDTOOLS_BIN, "10.0.28000.0"),
        expect_contains("url carries the version", BUILDTOOLS_URL, "/" + BUILDTOOLS_VERSION + "/"),
        expect_true("url is nuget.org", BUILDTOOLS_URL.startswith("https://api.nuget.org/")),
        expect_equal("digest length", len(BUILDTOOLS_SHA256), 64),
        expect_equal("license name", LICENSE, "windows-sdk-buildtools"),
        expect_equal("tool arches", TOOL_ARCHES, ["arm64", "x64"]),
        expect_equal("supported cells", SUPPORTED_CELLS, ["windows_arm64", "windows_x86_64"]),
        expect_contains("accepted hub has the x64 makeappx", accepted, '"tools/bin/10.0.28000.0/x64/makeappx.exe"'),
        expect_contains("accepted hub has the arm64 signtool", accepted, '"tools/bin/10.0.28000.0/arm64/signtool.exe"'),
        expect_true("accepted hub has no error", UNACCEPTED not in accepted and "error =" not in accepted),
        expect_contains("unaccepted hub names the license", unaccepted, LICENSE),
        expect_true("unaccepted hub downloads nothing", "tools/bin" not in unaccepted),
        expect_equal("windows only by default", compatible_with([]), ["@platforms//os:windows"]),
        expect_equal("windows is not repeated", compatible_with(["@platforms//os:windows", "//x:y"]), ["@platforms//os:windows", "//x:y"]),
        expect_equal("good min version", named_version_errors("min_version", "10.0.17763.0"), []),
        expect_true("short min version", len(named_version_errors("min_version", "10.0")) == 1),
        expect_contains("bad max version names the attribute", named_version_errors("max_version_tested", "10.0.x.0")[0], "max_version_tested '10.0.x.0'"),
        expect_true("markup in a version", len(named_version_errors("min_version", '1.0.0.0"/><x a="')) == 1),
        expect_equal("known license", license_errors([LICENSE]), []),
        expect_true("unknown license", len(license_errors(["other"])) == 1),
        expect_equal("good version", version_errors("1.2.3.0"), []),
        expect_equal("top version", version_errors("65535.0.0.65535"), []),
        expect_true("three-part version", len(version_errors("1.2.3")) == 1),
        expect_true("large version part", len(version_errors("1.2.3.65536")) == 1),
        expect_true("text version part", len(version_errors("1.2.x.0")) == 1),
        expect_true("empty version part", len(version_errors("1..3.0")) == 1),
        expect_equal("good identity", identity_errors("Contoso.App-2"), []),
        expect_true("short identity", len(identity_errors("ab")) == 1),
        expect_true("identity with space", len(identity_errors("Contoso App")) == 1),
        expect_equal("good application id", application_errors("App.Main2"), []),
        expect_true("digit-led application id", len(application_errors("2App")) == 1),
        expect_true("empty application id part", len(application_errors("App.")) == 1),
        expect_equal("good publisher", publisher_errors("CN=Contoso, O=Contoso"), []),
        expect_true("publisher without CN", len(publisher_errors("O=Contoso")) == 1),
        expect_equal("escaped comma in CN", publisher_cn("CN=Acme\\, Inc, O=Acme"), "Acme, Inc"),
        expect_equal("quoted CN", publisher_cn('CN="Acme, Inc", O=x'), "Acme, Inc"),
        expect_equal("plain CN", publisher_cn("CN=Dx Cell, O=Dx"), "Dx Cell"),
        expect_equal("CN only", publisher_cn("CN=Solo"), "Solo"),
        expect_equal("multi-valued CN", publisher_cn("CN=A+O=B"), "A"),
        expect_equal("escaped backslash in CN", publisher_cn("CN=Back\\\\slash"), "Back\\slash"),
        expect_equal("no alias", alias_errors(""), []),
        expect_equal("good alias", alias_errors("contoso-app.exe"), []),
        expect_true("alias without exe", len(alias_errors("contoso")) == 1),
        expect_true("alias with a path", len(alias_errors("bin\\contoso.exe")) == 1),
    ]

def msix_metadata_tests(name):
    """Declares the MSIX metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
