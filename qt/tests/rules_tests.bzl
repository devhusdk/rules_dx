"""Unit and analysis tests for the Qt rules."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//qt/rules:defs.bzl", "QtInfo", "qt_identifier", "qt_module_path", "qt_package_path", "qt_qmldir", "qt_qrc", "qt_uri_error")
load("//qt/sdk:pins.bzl", "UNSUPPORTED_TARGETS")
load("//quality:sources.bzl", "QualitySourcesInfo")

_LINUX_X86_64 = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def _unit_checks():
    return [
        expect_equal("module path", qt_module_path("Org.Example.Tally"), "Org/Example/Tally"),
        expect_equal(
            "qmldir with types",
            qt_qmldir("Tally", "tally.qmltypes", [("Main", "1.0", "Main.qml"), ("Badge", "1.0", "parts/Badge.qml")]),
            "module Tally\ntypeinfo tally.qmltypes\nprefer :/qt/qml/Tally/\nMain 1.0 Main.qml\nBadge 1.0 parts/Badge.qml\n",
        ),
        expect_equal(
            "qmldir without C++ types",
            qt_qmldir("A.B", "", []),
            "module A.B\nprefer :/qt/qml/A/B/\n",
        ),
        expect_equal(
            "qrc escapes markup",
            qt_qrc("/p&q", [("a<b>.txt", "../x/a<b>.txt")]),
            "<RCC>\n  <qresource prefix=\"/p&amp;q\">\n    <file alias=\"a&lt;b&gt;.txt\">../x/a&lt;b&gt;.txt</file>\n  </qresource>\n</RCC>\n",
        ),
        expect_equal("identifier for main", qt_identifier(Label("//examples/qt-app:tally")), "qt_pexamples_sqt_dapp_ptally"),
        expect_equal("identifier splits package and name", qt_identifier(Label("//a/b:c")) != qt_identifier(Label("//a:b_c")), True),
        expect_equal("identifier splits dash and underscore", qt_identifier(Label("//qt-app:x")) != qt_identifier(Label("//qt_app:x")), True),
        expect_equal("uri accepted", qt_uri_error("Org.Example_2.Tally"), ""),
        expect_equal("uri digit rejected", qt_uri_error("1Bad"), "uri '1Bad' segment '1Bad' must start with a letter"),
        expect_equal("uri dash rejected", qt_uri_error("Bad-Name"), "uri 'Bad-Name' segment 'Bad-Name' holds '-'"),
        expect_equal("uri empty rejected", qt_uri_error(""), "uri must not be empty"),
        expect_equal("main package path", qt_package_path("app/ui/Main.qml", "app"), "ui/Main.qml"),
        expect_equal("external package path", qt_package_path("../+ext+x/app/Main.qml", "app"), "Main.qml"),
        expect_equal("root package path", qt_package_path("Main.qml", ""), "Main.qml"),
        expect_equal("outside package", qt_package_path("other/Main.qml", "app"), None),
    ]

def _basenames(files):
    return sorted([f.basename for f in files])

def _module_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    sources = target[QualitySourcesInfo].direct_sources
    owned = {key: _basenames(sources[key].to_list()) for key in sources}
    asserts.equals(env, {"c": ["counter.h"], "cpp": ["counter.cc"], "qml": ["Badge.qml", "Main.qml"]}, owned)
    paths = [f.short_path for f in target[DefaultInfo].files.to_list()]
    for want in ["tally_qml/Tally/qmldir", "tally_qml/Tally/tally.qmltypes", "tally_qml/Tally/Main.qml", "tally_qml/Tally/Badge.qml"]:
        asserts.true(env, "examples/qt-app/tally/" + want in paths, "missing " + want + " in " + str(paths))
    asserts.equals(env, ["tally_metatypes.json"], _basenames(target[QtInfo].metatypes.to_list()))
    asserts.true(env, CcInfo in target, "CcInfo is not forwarded")
    return analysistest.end(env)

_module_test = analysistest.make(_module_test_impl)

def _argv(env, mnemonic):
    actions = [action for action in analysistest.target_actions(env) if action.mnemonic == mnemonic]
    if len(actions) == 0:
        return []
    return actions[0].argv

def _compile_test_impl(ctx):
    env = analysistest.begin(ctx)
    argv = _argv(env, "CppCompile")
    asserts.true(env, "-DDX_QT_SELECTED=1" in argv, "selected copt missing: " + str(argv))
    asserts.true(env, "-Iqt/tests" in argv, "package include missing: " + str(argv))
    return analysistest.end(env)

_compile_test = analysistest.make(_compile_test_impl)

def _moc_test_impl(ctx):
    env = analysistest.begin(ctx)
    argv = _argv(env, "QtMoc")
    asserts.true(env, "--output-json" in argv, str(argv))
    asserts.true(env, "-fqt/tests/fixtures/ticker.h" in argv, str(argv))
    asserts.true(env, len([arg for arg in argv if arg.startswith("-I") and arg.endswith("/include/QtCore")]) > 0, str(argv))
    asserts.true(env, argv[-1].endswith("qt/tests/fixtures/ticker.h"), str(argv))
    outputs = [f.basename for action in analysistest.target_actions(env) if action.mnemonic == "QtMoc" for f in action.outputs.to_list()]
    asserts.equals(env, ["moc_fixtures_ticker.cpp", "moc_fixtures_ticker.cpp.json"], sorted(outputs))
    return analysistest.end(env)

_moc_test = analysistest.make(_moc_test_impl)

def _failure_test_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.message)
    return analysistest.end(env)

_failure_test = analysistest.make(
    _failure_test_impl,
    expect_failure = True,
    attrs = {"message": attr.string(mandatory = True)},
)

def qt_rules_tests(name):
    """Declares the Qt rules unit and analysis tests."""
    starlark_test(
        name = name + "_unit",
        mode = "unit",
        checks = _unit_checks(),
    )
    _module_test(
        name = name + "_module",
        target_under_test = "//examples/qt-app/tally",
        target_compatible_with = _LINUX_X86_64,
    )
    _compile_test(
        name = name + "_compile",
        target_under_test = ":selected_upstream",
        target_compatible_with = _LINUX_X86_64,
    )
    _moc_test(
        name = name + "_moc",
        target_under_test = ":selected_qt",
        target_compatible_with = _LINUX_X86_64,
    )
    failures = {
        "bad_uri": "uri '1Bad' segment '1Bad' must start with a letter",
        "foreign": "shared.h' must be in package qt/tests; give it its own target",
        "form": "Qt target 'widgets' is unsupported: " + UNSUPPORTED_TARGETS["widgets"],
    }
    for fixture in sorted(failures.keys()):
        _failure_test(
            name = name + "_" + fixture,
            target_under_test = ":" + fixture,
            message = failures[fixture],
            target_compatible_with = _LINUX_X86_64,
        )
    native.test_suite(
        name = name,
        tests = [":" + name + "_" + suffix for suffix in ["unit", "module", "compile", "moc"] + sorted(failures.keys())],
    )
