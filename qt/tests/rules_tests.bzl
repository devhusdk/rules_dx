"""Unit and analysis tests for the Qt rules."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//qt/rules:defs.bzl", "QtInfo", "qt_bundle_members", "qt_framework_path", "qt_identifier", "qt_module_path", "qt_package_path", "qt_qmldir", "qt_qrc", "qt_uri_error")
load("//qt/sdk:pins.bzl", "CELLS")
load("//quality:sources.bzl", "QualitySourcesInfo")

_LINUX = select({
    "@platforms//cpu:arm64": ["@platforms//os:linux"],
    "@platforms//cpu:x86_64": ["@platforms//os:linux"],
    "//conditions:default": ["@platforms//:incompatible"],
})

_LINUX_X86_64 = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def _unit_checks():
    return [
        expect_equal("module path", qt_module_path("Org.Example.Tally"), "Org/Example/Tally"),
        expect_equal("framework path", qt_framework_path("../dx_qt/lib/QtCore.framework/Versions/A/QtCore"), "QtCore.framework/Versions/A/QtCore"),
        expect_equal("framework path skips plain libraries", qt_framework_path("_solib_k8/libQt6Core.so.6"), None),
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
            "qmldir with dependencies",
            qt_qmldir("A", "a.qmltypes", [], ["QtQml.Models", "QtQuick"]),
            "module A\ntypeinfo a.qmltypes\nprefer :/qt/qml/A/\ndepends QtQml.Models\ndepends QtQuick\n",
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
        expect_equal(
            "bundle members",
            qt_bundle_members(
                [
                    "app/tasks",
                    "_solib_k8/qt/libQt6Core.so.6",
                    "../qt/lib/libQt6Core.so.6",
                    "../qt/lib/libQt6Core.so",
                    "_solib_k8/llvm/libunwind.so",
                    "../qt/lib/libQt6Gui.so.6",
                    "../qt/lib/libicuuc.so.73",
                    "../qt/lib/libQt6Network.so.6",
                    "../qt/qml/builtins.qmltypes",
                    "../qt/qml/QML/qmldir",
                    "../qt/qml/QtQml/qmldir",
                    "../qt/qml/QtQml/libqmlplugin.so",
                    "../qt/qml/QtQml/Models/qmldir",
                    "../qt/qml/QtQuick/qmldir",
                ],
                ["../qt/licenses/qt/LGPL-3.0-only.txt"],
                None,
                CELLS["linux_x86_64"].needs,
            ),
            [
                ("lib/libQt6Core.so.6", "../qt/lib/libQt6Core.so.6"),
                ("lib/libQt6Network.so.6", "../qt/lib/libQt6Network.so.6"),
                ("lib/libicuuc.so.73", "../qt/lib/libicuuc.so.73"),
                ("lib/libunwind.so", "_solib_k8/llvm/libunwind.so"),
                ("licenses/qt/LGPL-3.0-only.txt", "../qt/licenses/qt/LGPL-3.0-only.txt"),
                ("qml/QML/qmldir", "../qt/qml/QML/qmldir"),
                ("qml/QtQml/Models/qmldir", "../qt/qml/QtQml/Models/qmldir"),
                ("qml/QtQml/libqmlplugin.so", "../qt/qml/QtQml/libqmlplugin.so"),
                ("qml/QtQml/qmldir", "../qt/qml/QtQml/qmldir"),
            ],
        ),
        expect_equal(
            "bundle root package licenses",
            qt_bundle_members(["app/tool"], ["licenses/fmt/LICENSE", "third_party/licenses/zlib/LICENSE"], None, CELLS["linux_x86_64"].needs),
            [("licenses/fmt/LICENSE", "licenses/fmt/LICENSE"), ("licenses/zlib/LICENSE", "third_party/licenses/zlib/LICENSE")],
        ),
        expect_equal(
            "bundle without QML",
            qt_bundle_members(["app/tool", "_solib_k8/qt/libQt6Core.so.6", "../qt/lib/libQt6Core.so.6", "../qt/lib/libQt6Gui.so.6"], [], None, CELLS["linux_x86_64"].needs),
            [("lib/libQt6Core.so.6", "../qt/lib/libQt6Core.so.6")],
        ),
        expect_equal(
            "bundle with Gui",
            qt_bundle_members(
                [
                    "app/viewer",
                    "_solib_k8/qt/libQt6Gui.so.6",
                    "../qt/lib/libQt6Gui.so.6",
                    "../qt/lib/libQt6Core.so.6",
                    "../qt/lib/libX11.so.6",
                    "../qt/lib/libxkbcommon.so.0",
                    "../qt/lib/libQt6XcbQpa.so.6",
                    "../qt/plugins/platforms/libqoffscreen.so",
                    "../qt/fonts/fonts.conf",
                    "../qt/fonts/DejaVuSans.ttf",
                ],
                [],
                None,
                CELLS["linux_x86_64"].needs,
            ),
            [
                ("fonts/DejaVuSans.ttf", "../qt/fonts/DejaVuSans.ttf"),
                ("fonts/fonts.conf", "../qt/fonts/fonts.conf"),
                ("lib/libQt6Core.so.6", "../qt/lib/libQt6Core.so.6"),
                ("lib/libQt6Gui.so.6", "../qt/lib/libQt6Gui.so.6"),
                ("lib/libX11.so.6", "../qt/lib/libX11.so.6"),
                ("lib/libxkbcommon.so.0", "../qt/lib/libxkbcommon.so.0"),
                ("plugins/platforms/libqoffscreen.so", "../qt/plugins/platforms/libqoffscreen.so"),
            ],
        ),
        expect_equal(
            "bundle with display plugins",
            qt_bundle_members(
                [
                    "app/viewer",
                    "_solib_k8/qt/libQt6Gui.so.6",
                    "../qt/lib/libQt6Gui.so.6",
                    "../qt/lib/libQt6Core.so.6",
                    "../qt/lib/libQt6WaylandClient.so.6",
                    "../qt/lib/libQt6XcbQpa.so.6",
                    "../qt/lib/libxcb.so.1",
                    "../qt/lib/libxcb-cursor.so.0",
                    "../qt/lib/libxcb-glx.so.0",
                    "../qt/lib/libxcb-image.so.0",
                    "../qt/lib/libxcb-render.so.0",
                    "../qt/lib/libxcb-render-util.so.0",
                    "../qt/lib/libxcb-shm.so.0",
                    "../qt/lib/libxcb-util.so.1",
                    "../qt/lib/libxcb-xkb.so.1",
                    "../qt/lib/libwayland-client.so.0",
                    "../qt/lib/libwayland-cursor.so.0",
                    "../qt/lib/libffi.so.8",
                    "../qt/plugins/platforms/libqxcb.so",
                    "../qt/plugins/xcbglintegrations/libqxcb-egl-integration.so",
                    "../qt/plugins/xcbglintegrations/libqxcb-glx-integration.so",
                    "../qt/plugins/platforms/libqwayland.so",
                    "../qt/plugins/wayland-shell-integration/libxdg-shell.so",
                    "../qt/plugins/wayland-shell-integration/libwl-shell-plugin.so",
                ],
                [],
                None,
                CELLS["linux_x86_64"].needs,
            ),
            [
                ("lib/libQt6Core.so.6", "../qt/lib/libQt6Core.so.6"),
                ("lib/libQt6Gui.so.6", "../qt/lib/libQt6Gui.so.6"),
                ("lib/libQt6WaylandClient.so.6", "../qt/lib/libQt6WaylandClient.so.6"),
                ("lib/libQt6XcbQpa.so.6", "../qt/lib/libQt6XcbQpa.so.6"),
                ("lib/libffi.so.8", "../qt/lib/libffi.so.8"),
                ("lib/libwayland-client.so.0", "../qt/lib/libwayland-client.so.0"),
                ("lib/libwayland-cursor.so.0", "../qt/lib/libwayland-cursor.so.0"),
                ("lib/libxcb-cursor.so.0", "../qt/lib/libxcb-cursor.so.0"),
                ("lib/libxcb-glx.so.0", "../qt/lib/libxcb-glx.so.0"),
                ("lib/libxcb-image.so.0", "../qt/lib/libxcb-image.so.0"),
                ("lib/libxcb-render-util.so.0", "../qt/lib/libxcb-render-util.so.0"),
                ("lib/libxcb-render.so.0", "../qt/lib/libxcb-render.so.0"),
                ("lib/libxcb-shm.so.0", "../qt/lib/libxcb-shm.so.0"),
                ("lib/libxcb-util.so.1", "../qt/lib/libxcb-util.so.1"),
                ("lib/libxcb-xkb.so.1", "../qt/lib/libxcb-xkb.so.1"),
                ("lib/libxcb.so.1", "../qt/lib/libxcb.so.1"),
                ("plugins/platforms/libqwayland.so", "../qt/plugins/platforms/libqwayland.so"),
                ("plugins/platforms/libqxcb.so", "../qt/plugins/platforms/libqxcb.so"),
                ("plugins/wayland-shell-integration/libxdg-shell.so", "../qt/plugins/wayland-shell-integration/libxdg-shell.so"),
                ("plugins/xcbglintegrations/libqxcb-egl-integration.so", "../qt/plugins/xcbglintegrations/libqxcb-egl-integration.so"),
                ("plugins/xcbglintegrations/libqxcb-glx-integration.so", "../qt/plugins/xcbglintegrations/libqxcb-glx-integration.so"),
            ],
        ),
        expect_equal(
            "a lone xcb glx library is not staged without a plugin that needs it",
            qt_bundle_members(
                [
                    "app/viewer",
                    "../qt/lib/libQt6Gui.so.6",
                    "../qt/lib/libxcb-glx.so.0",
                    "../qt/plugins/platforms/libqoffscreen.so",
                ],
                [],
                None,
                CELLS["linux_x86_64"].needs,
            ),
            [
                ("lib/libQt6Gui.so.6", "../qt/lib/libQt6Gui.so.6"),
                ("plugins/platforms/libqoffscreen.so", "../qt/plugins/platforms/libqoffscreen.so"),
            ],
        ),
        expect_equal(
            "bundle with runtime plugins",
            qt_bundle_members(
                [
                    "app/viewer",
                    "_solib_k8/qt/libQt6Core.so.6",
                    "../qt/lib/libQt6Core.so.6",
                    "../qt/lib/libQt6DBus.so.6",
                    "../qt/lib/libQt6Gui.so.6",
                    "../qt/lib/libQt6Network.so.6",
                    "../qt/plugins/imageformats/libqjpeg.so",
                    "../qt/plugins/platformthemes/libqgtk3.so",
                    "../qt/plugins/platformthemes/libqxdgdesktopportal.so",
                    "../qt/plugins/tls/libqopensslbackend.so",
                ],
                [],
                None,
                CELLS["linux_x86_64"].needs,
            ),
            [
                ("lib/libQt6Core.so.6", "../qt/lib/libQt6Core.so.6"),
                ("lib/libQt6DBus.so.6", "../qt/lib/libQt6DBus.so.6"),
                ("lib/libQt6Gui.so.6", "../qt/lib/libQt6Gui.so.6"),
                ("lib/libQt6Network.so.6", "../qt/lib/libQt6Network.so.6"),
                ("plugins/imageformats/libqjpeg.so", "../qt/plugins/imageformats/libqjpeg.so"),
                ("plugins/platformthemes/libqxdgdesktopportal.so", "../qt/plugins/platformthemes/libqxdgdesktopportal.so"),
                ("plugins/tls/libqopensslbackend.so", "../qt/plugins/tls/libqopensslbackend.so"),
            ],
        ),
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

def _rust_module_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.equals(env, {}, {key: value for key, value in target[QualitySourcesInfo].direct_sources.items() if value.to_list()})
    paths = [f.short_path for f in target[DefaultInfo].files.to_list()]
    for want in ["model_qml/Ledger/Model/qmldir", "model_qml/Ledger/Model/model.qmltypes"]:
        asserts.true(env, "examples/qt-rust/model/" + want in paths, "missing " + want + " in " + str(paths))
    asserts.equals(env, ["model_metatypes.json"], _basenames(target[QtInfo].metatypes.to_list()))
    asserts.true(env, CcInfo in target, "CcInfo is not forwarded")
    return analysistest.end(env)

_rust_module_test = analysistest.make(_rust_module_test_impl)

def _bridge_test_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [action for action in analysistest.target_actions(env) if action.mnemonic == "QtRustBridge"]
    asserts.equals(env, 1, len(actions))
    argv = actions[0].argv
    asserts.equals(env, ["qt", "examples/qt-rust/model/ledger_model.rs", "examples/qt-rust/model/ledger_model"], argv[1:4], str(argv))
    outputs = [f.basename for f in actions[0].outputs.to_list()]
    asserts.equals(env, ["ledger_model.cxx.cpp", "ledger_model.cxx.h", "ledger_model.cxxqt.cpp", "ledger_model.cxxqt.h"], sorted(outputs))
    asserts.equals(env, [f.basename for f in actions[0].outputs.to_list()], [arg.rpartition("/")[2] for arg in argv[4:]])
    return analysistest.end(env)

_bridge_test = analysistest.make(_bridge_test_impl)

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

def _uic_test_impl(ctx):
    env = analysistest.begin(ctx)
    argv = _argv(env, "QtUic")
    asserts.true(env, argv[-1].endswith("qt/tests/fixtures/dialog.ui"), str(argv))
    asserts.true(env, argv[-2].endswith("qt/tests/_qt/form/ui_dialog.h"), str(argv))
    return analysistest.end(env)

_uic_test = analysistest.make(_uic_test_impl)

def _widgets_test_impl(ctx):
    env = analysistest.begin(ctx)
    headers = [f.short_path for f in analysistest.target_under_test(env)[CcInfo].compilation_context.headers.to_list()]
    for want in ["/include/QtWidgets/QDialog", "/include/QtGui/QFont", "qt/tests/_qt/form/ui_dialog.h"]:
        asserts.true(env, len([h for h in headers if h.endswith(want)]) > 0, "missing " + want)
    return analysistest.end(env)

_widgets_test = analysistest.make(_widgets_test_impl)

def _lrelease_test_impl(ctx):
    env = analysistest.begin(ctx)
    argv = _argv(env, "QtLrelease")
    asserts.equals(env, ["-silent", "qt/tests/fixtures/de.ts", "-qm"], argv[1:4], str(argv))
    asserts.true(env, argv[-1].endswith("qt/tests/_qt/translated/i18n/de.qm"), str(argv))
    embedded = [f.basename for action in analysistest.target_actions(env) if action.mnemonic == "QtRcc" for f in action.inputs.to_list()]
    asserts.true(env, "de.qm" in embedded, str(embedded))
    return analysistest.end(env)

_lrelease_test = analysistest.make(_lrelease_test_impl)

def _runpath_test_impl(ctx):
    env = analysistest.begin(ctx)
    argv = _argv(env, "CppLink")
    asserts.equals(env, ctx.attr.bundled, "-Wl,--disable-new-dtags,-rpath,$ORIGIN/resources/lib" in argv, str(argv))
    return analysistest.end(env)

_runpath_test = analysistest.make(_runpath_test_impl, attrs = {"bundled": attr.bool(mandatory = True)})

def _frameworks_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    flags = [flag for linker_input in target[CcInfo].linking_context.linker_inputs.to_list() for flag in linker_input.user_link_flags]
    files = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list()]
    asserts.equals(env, ctx.attr.flags, flags)
    asserts.equals(env, ctx.attr.files, files)
    return analysistest.end(env)

_frameworks_test = analysistest.make(_frameworks_test_impl, attrs = {
    "files": attr.string_list(),
    "flags": attr.string_list(),
})

def _bundle_test_impl(ctx):
    env = analysistest.begin(ctx)
    prefix = "examples/qt-tasks/app/runtime/"
    members = [f.short_path[len(prefix):] for f in analysistest.target_under_test(env)[DefaultInfo].files.to_list()]
    for want in ["lib/libQt6Core.so.6", "lib/libQt6Qml.so.6", "lib/libicuuc.so.73", "lib/libstdc++.so.6", "qml/QML/qmldir", "qml/QtQml/qmldir", "qml/QtQml/libqmlplugin.so", "licenses/qt/LGPL-3.0-only.txt"]:
        asserts.true(env, want in members, "missing " + want)
    asserts.false(env, "lib/libQt6Core.so" in members, "unversioned library staged")
    asserts.equals(env, [], [m for m in members if m.startswith("qml/QtQuick/")])
    for unwanted in ["Gui", "Quick", "Widgets", "XcbQpa", "WaylandClient", "EglFSDeviceIntegration"]:
        asserts.equals(env, [], [m for m in members if m.startswith("lib/libQt6" + unwanted)], unwanted + " is staged")
    for want in ["licenses/fmt/LICENSE", "licenses/llvm/LICENSE.TXT"]:
        asserts.true(env, want in members, "missing " + want)
    return analysistest.end(env)

_bundle_test = analysistest.make(_bundle_test_impl)

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
        target_compatible_with = _LINUX,
    )
    _moc_test(
        name = name + "_moc",
        target_under_test = ":selected_qt",
        target_compatible_with = _LINUX,
    )
    _uic_test(
        name = name + "_uic",
        target_under_test = ":form_qt",
        target_compatible_with = _LINUX,
    )
    _widgets_test(
        name = name + "_widgets",
        target_under_test = ":form",
        target_compatible_with = _LINUX,
    )
    _lrelease_test(
        name = name + "_lrelease",
        target_under_test = ":translated_qt",
        target_compatible_with = _LINUX,
    )
    _runpath_test(
        name = name + "_runpath",
        bundled = True,
        target_under_test = ":plain_app_upstream",
        target_compatible_with = _LINUX_X86_64,
    )
    _runpath_test(
        name = name + "_test_runpath",
        bundled = False,
        target_under_test = ":plain_test_upstream",
        target_compatible_with = _LINUX_X86_64,
    )
    _frameworks_test(
        name = name + "_frameworks",
        files = ["qt/tests/frameworks_fixture/QtFake.framework/Versions/A/QtFake"],
        flags = ["-Wl,-rpath,@loader_path/frameworks_fixture"],
        target_under_test = ":frameworks_fixture",
    )
    _frameworks_test(
        name = name + "_linux_frameworks",
        target_under_test = ":plain_app_qt_frameworks",
        target_compatible_with = _LINUX_X86_64,
    )
    _bundle_test(
        name = name + "_bundle",
        target_under_test = "//examples/qt-tasks/app:runtime",
        target_compatible_with = _LINUX_X86_64,
    )
    _rust_module_test(
        name = name + "_rust_module",
        target_under_test = "//examples/qt-rust/model",
        target_compatible_with = _LINUX_X86_64,
    )
    _bridge_test(
        name = name + "_bridge",
        target_under_test = "//examples/qt-rust/model:model_bridge",
        target_compatible_with = _LINUX_X86_64,
    )
    failures = {
        "bad_depends": "qml_depends uri 'QtQml.2Models' segment '2Models' must start with a letter",
        "bad_uri": "uri '1Bad' segment '1Bad' must start with a letter",
        "bundle_bad_license": "qt_bundle: license 'qt/tests/probe.cc' is not below a licenses directory",
        "bundle_unknown_module": "qt_bundle: QML module 'Tasks.Missing' is not in the import tree",
        "clashing_translations": "translated_other both embed :/i18n/de.qm",
        "twin_translations": "both produce de.qm",
        "foreign": "shared.h' must be in package qt/tests; give it its own target",
        "rust_foreign": "bridge '" + str(Label("//qt/tests/other:bridge.rs")) + "' must be in package qt/tests; give it its own target",
    }
    for fixture in sorted(failures.keys()):
        _failure_test(
            name = name + "_" + fixture,
            target_under_test = ":" + fixture,
            message = failures[fixture],
            target_compatible_with = _LINUX_X86_64 if fixture == "bundle_unknown_module" else _LINUX,
        )
    native.test_suite(
        name = name,
        tests = [":" + name + "_" + suffix for suffix in ["unit", "module", "rust_module", "bridge", "compile", "moc", "uic", "widgets", "lrelease", "runpath", "test_runpath", "frameworks", "linux_frameworks", "bundle"] + sorted(failures.keys())],
    )
