"""Unit and analysis tests for the Qt rules."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//qt/rules:defs.bzl", "QT_BUNDLE_COMPATIBLE_WITH", "QT_BUNDLE_MACOS_RUNPATH", "QT_COMPATIBLE_WITH", "QT_CONF", "qt_bundle_members", "qt_consumer_identity", "qt_framework_path", "qt_identifier", "qt_library_stem", "qt_module_path", "qt_package_path", "qt_qmldir", "qt_qrc", "qt_runtime_members")
load("//qt/rules:providers.bzl", "QtInfo")
load("//qt/rules:validate.bzl", "qt_uri_error")
load("//qt/sdk:pins.bzl", "CELLS")
load("//quality:sources.bzl", "QualitySourcesInfo")

_LINUX = select({
    "@platforms//cpu:arm64": ["@platforms//os:linux"],
    "@platforms//cpu:x86_64": ["@platforms//os:linux"],
    "//conditions:default": ["@platforms//:incompatible"],
})

_MACOS_ARM64 = ["@platforms//os:macos", "@platforms//cpu:arm64"]

_LINUX_X86_64 = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def _unit_checks():
    return [
        expect_equal("consumer identity linux_arm64", qt_consumer_identity("linux_arm64"), struct(arch = "aarch64", os = "linux", min_runtime = "glibc-2.39")),
        expect_equal("consumer identity linux_x86_64", qt_consumer_identity("linux_x86_64"), struct(arch = "x86_64", os = "linux", min_runtime = "glibc-2.34")),
        expect_equal("consumer identity macos_arm64", qt_consumer_identity("macos_arm64"), struct(arch = "aarch64", os = "macos", min_runtime = "macos-13.0")),
        expect_equal("library stem linux", qt_library_stem("libQt6Gui.so.6"), "Qt6Gui"),
        expect_equal("library stem linux plugin", qt_library_stem("libqoffscreen.so"), "qoffscreen"),
        expect_equal("library stem macos plugin", qt_library_stem("libqoffscreen.dylib"), "qoffscreen"),
        expect_equal("library stem framework", qt_library_stem("QtGui"), "QtGui"),
        expect_equal("library stem windows", qt_library_stem("Qt6Gui.dll"), "Qt6Gui"),
        expect_equal("library stem windows plugin", qt_library_stem("qoffscreen.dll"), "qoffscreen"),
        expect_equal("library stem skips other files", qt_library_stem("qt.conf"), None),
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
        expect_equal("identifier for main", qt_identifier(Label("//qt/tests/fixtures/qt-app:tally")), "qt_pqt_stests_sfixtures_sqt_dapp_ptally"),
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
                ["QML", "QtQml"],
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
            "macos bundle members",
            qt_bundle_members(
                [
                    "app/tasks",
                    "app/qt_frameworks/QtGui.framework/Versions/A/QtGui",
                    "../qt/lib/QtCore.framework/Versions/A/QtCore",
                    "../qt/lib/QtDBus.framework/Versions/A/QtDBus",
                    "../qt/lib/QtGui.framework/Versions/A/QtGui",
                    "../qt/lib/QtNetwork.framework/Versions/A/QtNetwork",
                    "../qt/lib/QtSql.framework/Versions/A/QtSql",
                    "../qt/plugins/tls/libqcertonlybackend.dylib",
                ],
                [],
                [],
                CELLS["macos_arm64"].needs,
            ),
            [
                ("lib/QtCore.framework/Versions/A/QtCore", "../qt/lib/QtCore.framework/Versions/A/QtCore"),
                ("lib/QtDBus.framework/Versions/A/QtDBus", "../qt/lib/QtDBus.framework/Versions/A/QtDBus"),
                ("lib/QtGui.framework/Versions/A/QtGui", "../qt/lib/QtGui.framework/Versions/A/QtGui"),
                ("lib/QtNetwork.framework/Versions/A/QtNetwork", "../qt/lib/QtNetwork.framework/Versions/A/QtNetwork"),
                ("plugins/tls/libqcertonlybackend.dylib", "../qt/plugins/tls/libqcertonlybackend.dylib"),
            ],
        ),
        expect_equal(
            "bundle root package licenses",
            qt_bundle_members(["app/tool"], ["licenses/fmt/LICENSE", "third_party/licenses/zlib/LICENSE"], [], CELLS["linux_x86_64"].needs),
            [("licenses/fmt/LICENSE", "licenses/fmt/LICENSE"), ("licenses/zlib/LICENSE", "third_party/licenses/zlib/LICENSE")],
        ),
        expect_equal(
            "bundle stages macos frameworks",
            qt_bundle_members(
                [
                    "app/tool",
                    "app/frames/QtGui.framework/Versions/A/QtGui",
                    "../qt/lib/QtGui.framework/Versions/A/QtGui",
                    "../qt/lib/QtCore.framework/Versions/A/QtCore",
                    "../qt/lib/QtNetwork.framework/Versions/A/QtNetwork",
                    "../qt/plugins/platforms/libqoffscreen.dylib",
                ],
                [],
                [],
                struct(
                    library = {"QtGui.framework": ["QtCore.framework"]},
                    plugin = {},
                    runtime_plugin = {"platforms/libqoffscreen.dylib": ["QtGui.framework"]},
                ),
            ),
            [
                ("lib/QtCore.framework/Versions/A/QtCore", "../qt/lib/QtCore.framework/Versions/A/QtCore"),
                ("lib/QtGui.framework/Versions/A/QtGui", "../qt/lib/QtGui.framework/Versions/A/QtGui"),
                ("plugins/platforms/libqoffscreen.dylib", "../qt/plugins/platforms/libqoffscreen.dylib"),
            ],
        ),
        expect_equal(
            "bundle skips framework headers and stages the cocoa plugin",
            qt_bundle_members(
                [
                    "app/tool",
                    "../qt/lib/QtCore.framework/Headers/qglobal.h",
                    "../qt/lib/QtCore.framework/Versions/A/Headers/qglobal.h",
                    "../qt/lib/QtCore.framework/Versions/A/QtCore",
                    "../qt/lib/QtGui.framework/Versions/A/Headers/qwindow.h",
                    "../qt/lib/QtGui.framework/Versions/A/QtGui",
                    "../qt/plugins/platforms/libqcocoa.dylib",
                ],
                [],
                [],
                CELLS["macos_arm64"].needs,
            ),
            [
                ("lib/QtCore.framework/Versions/A/QtCore", "../qt/lib/QtCore.framework/Versions/A/QtCore"),
                ("lib/QtGui.framework/Versions/A/QtGui", "../qt/lib/QtGui.framework/Versions/A/QtGui"),
                ("plugins/platforms/libqcocoa.dylib", "../qt/plugins/platforms/libqcocoa.dylib"),
            ],
        ),
        expect_equal(
            "bundle stages linked macos dylibs",
            qt_bundle_members(
                [
                    "app/tool",
                    "app/_solib_darwin_arm64/libapp_Smodel_Slibmodel_Uupstream.dylib",
                    "app/_solib_darwin_arm64/libapp_Sui_Slibui_Uupstream.dylib",
                    "../qt/lib/QtCore.framework/Versions/A/QtCore",
                ],
                [],
                [],
                CELLS["macos_arm64"].needs,
            ),
            [
                ("lib/libapp_Smodel_Slibmodel_Uupstream.dylib", "app/_solib_darwin_arm64/libapp_Smodel_Slibmodel_Uupstream.dylib"),
                ("lib/libapp_Sui_Slibui_Uupstream.dylib", "app/_solib_darwin_arm64/libapp_Sui_Slibui_Uupstream.dylib"),
            ],
        ),
        # The linked rule (a DLL not under ../) is confirmed on a Windows host by #6520.
        expect_equal(
            "bundle stages windows dlls",
            qt_bundle_members(
                [
                    "app/tool.exe",
                    "app/Qt6Gui.dll",
                    "../qt/bin/Qt6Gui.dll",
                    "../qt/bin/Qt6Core.dll",
                    "../qt/bin/Qt6Network.dll",
                    "../qt/lib/Qt6Gui.lib",
                    "../qt/plugins/platforms/qoffscreen.dll",
                ],
                [],
                [],
                struct(
                    library = {"Qt6Gui.dll": ["Qt6Core.dll", "KERNEL32.dll"]},
                    plugin = {},
                    runtime_plugin = {"platforms/qoffscreen.dll": ["Qt6Gui.dll"]},
                ),
            ),
            [
                ("bin/Qt6Core.dll", "../qt/bin/Qt6Core.dll"),
                ("bin/Qt6Gui.dll", "../qt/bin/Qt6Gui.dll"),
                ("plugins/platforms/qoffscreen.dll", "../qt/plugins/platforms/qoffscreen.dll"),
            ],
        ),
        expect_equal(
            "runtime members of a windows bundle",
            qt_runtime_members(["bin/Qt6Core.dll", "plugins/platforms/qoffscreen.dll", "lib/libQt6Core.so.6", "resources/bin/x.dll"]),
            ["bin/Qt6Core.dll"],
        ),
        expect_equal("qt.conf points at resources", QT_CONF, "[Paths]\nPrefix = resources\n"),
        expect_equal(
            "bundle without linked windows dlls",
            qt_bundle_members(["app/tool.exe", "../qt/bin/Qt6Core.dll", "../qt/bin/Qt6Gui.dll"], [], [], struct(library = {}, plugin = {}, runtime_plugin = {})),
            [],
        ),
        expect_equal(
            "bundle without QML",
            qt_bundle_members(["app/tool", "_solib_k8/qt/libQt6Core.so.6", "../qt/lib/libQt6Core.so.6", "../qt/lib/libQt6Gui.so.6"], [], [], CELLS["linux_x86_64"].needs),
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
                [],
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
                [],
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
                [],
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
                [],
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
        asserts.true(env, "qt/tests/fixtures/qt-app/tally/" + want in paths, "missing " + want + " in " + str(paths))
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
        asserts.true(env, "qt/tests/fixtures/qt-rust/model/" + want in paths, "missing " + want + " in " + str(paths))
    asserts.equals(env, ["model_metatypes.json"], _basenames(target[QtInfo].metatypes.to_list()))
    asserts.true(env, CcInfo in target, "CcInfo is not forwarded")
    for linker_input in target[CcInfo].linking_context.linker_inputs.to_list():
        owner = str(linker_input.owner)
        for library in linker_input.libraries:
            if "model_upstream" in owner:
                asserts.equals(env, None, library.dynamic_library, "model_upstream links as a dylib: " + owner)
        for flag in linker_input.user_link_flags:
            asserts.false(env, "-undefined" in flag, "dynamic_lookup flag on " + owner)
    return analysistest.end(env)

_rust_module_test = analysistest.make(_rust_module_test_impl)

def _library_link_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    found = False
    for linker_input in target[CcInfo].linking_context.linker_inputs.to_list():
        owner = str(linker_input.owner)
        if "gui_upstream" in owner:
            found = True
            for library in linker_input.libraries:
                if ctx.attr.static:
                    asserts.equals(env, None, library.dynamic_library, "gui_upstream links as a dylib: " + owner)
                else:
                    asserts.true(env, library.dynamic_library != None, "gui_upstream has no dylib: " + owner)
        for flag in linker_input.user_link_flags:
            asserts.false(env, "-undefined" in flag, "dynamic_lookup flag on " + owner)
    asserts.true(env, found, "gui_upstream is not in the linking context")
    if ctx.attr.static:
        for action in analysistest.target_actions(env):
            asserts.false(env, action.mnemonic == "CppLink", "unexpected CppLink action")
    return analysistest.end(env)

_library_link_test = analysistest.make(_library_link_test_impl, attrs = {"static": attr.bool()})

def _bridge_test_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [action for action in analysistest.target_actions(env) if action.mnemonic == "QtRustBridge"]
    asserts.equals(env, 1, len(actions))
    argv = actions[0].argv
    asserts.equals(env, ["qt", "qt/tests/fixtures/qt-rust/model/ledger_model.rs", "qt/tests/fixtures/qt-rust/model/ledger_model"], argv[1:4], str(argv))
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
    asserts.equals(env, ctx.attr.bundled, ctx.attr.flag in argv, str(argv))
    return analysistest.end(env)

_runpath_test = analysistest.make(_runpath_test_impl, attrs = {
    "bundled": attr.bool(mandatory = True),
    "flag": attr.string(mandatory = True),
})

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
    target = analysistest.target_under_test(env)
    prefix = "qt/tests/fixtures/qt-tasks/app/" + target.label.name + "/"
    members = [f.short_path[len(prefix):] for f in target[DefaultInfo].files.to_list()]
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

def _macos_bundle_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    prefix = "qt/tests/fixtures/qt-tasks/app/" + target.label.name + "/"
    members = [f.short_path[len(prefix):] for f in target[DefaultInfo].files.to_list()]
    for want in ["lib/QtCore.framework/Versions/A/QtCore", "lib/QtQml.framework/Versions/A/QtQml", "qml/QtQml/libqmlplugin.dylib"]:
        asserts.true(env, want in members, "missing " + want)
    asserts.equals(env, [], [m for m in members if "/Headers/" in m or "/Headers" == m[-8:]], "headers staged")
    for unwanted in ["Gui", "Quick", "Widgets"]:
        asserts.equals(env, [], [m for m in members if m.startswith("lib/Qt" + unwanted + ".framework")], unwanted + " is staged")
    return analysistest.end(env)

_macos_bundle_test = analysistest.make(_macos_bundle_test_impl)

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
        target_under_test = "//qt/tests/fixtures/qt-app/tally",
        target_compatible_with = QT_COMPATIBLE_WITH,
    )
    _compile_test(
        name = name + "_compile",
        target_under_test = ":selected_upstream",
        target_compatible_with = QT_COMPATIBLE_WITH,
    )
    _moc_test(
        name = name + "_moc",
        target_under_test = ":selected_qt",
        target_compatible_with = QT_COMPATIBLE_WITH,
    )
    _uic_test(
        name = name + "_uic",
        target_under_test = ":form_qt",
        target_compatible_with = QT_COMPATIBLE_WITH,
    )
    _widgets_test(
        name = name + "_widgets",
        target_under_test = ":form",
        target_compatible_with = QT_COMPATIBLE_WITH,
    )
    _lrelease_test(
        name = name + "_lrelease",
        target_under_test = ":translated_qt",
        target_compatible_with = QT_COMPATIBLE_WITH,
    )
    _runpath_test(
        name = name + "_runpath",
        bundled = True,
        flag = "-Wl,--disable-new-dtags,-rpath,$ORIGIN/resources/lib",
        target_under_test = ":plain_app_upstream",
        target_compatible_with = _LINUX,
    )
    _runpath_test(
        name = name + "_macos_runpath",
        bundled = True,
        flag = QT_BUNDLE_MACOS_RUNPATH,
        target_under_test = ":plain_app_upstream",
        target_compatible_with = _MACOS_ARM64,
    )
    _runpath_test(
        name = name + "_test_runpath",
        bundled = False,
        flag = "-Wl,--disable-new-dtags,-rpath,$ORIGIN/resources/lib",
        target_under_test = ":plain_test_upstream",
        target_compatible_with = _LINUX,
    )
    _runpath_test(
        name = name + "_macos_test_runpath",
        bundled = False,
        flag = QT_BUNDLE_MACOS_RUNPATH,
        target_under_test = ":plain_test_upstream",
        target_compatible_with = _MACOS_ARM64,
    )
    _library_link_test(
        name = name + "_macos_library_link",
        static = True,
        target_under_test = ":gui_upstream",
        target_compatible_with = _MACOS_ARM64,
    )
    _library_link_test(
        name = name + "_linux_library_link",
        static = False,
        target_under_test = ":gui_upstream",
        target_compatible_with = _LINUX,
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
        target_under_test = "//qt/tests/fixtures/qt-tasks/app:runtime",
        target_compatible_with = _LINUX,
    )
    _macos_bundle_test(
        name = name + "_macos_bundle",
        target_under_test = "//qt/tests/fixtures/qt-tasks/app:runtime",
        target_compatible_with = _MACOS_ARM64,
    )
    _rust_module_test(
        name = name + "_rust_module",
        target_under_test = "//qt/tests/fixtures/qt-rust/model",
        target_compatible_with = QT_COMPATIBLE_WITH,
    )
    _bridge_test(
        name = name + "_bridge",
        target_under_test = "//qt/tests/fixtures/qt-rust/model:model_bridge",
        target_compatible_with = QT_COMPATIBLE_WITH,
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
            target_compatible_with = QT_BUNDLE_COMPATIBLE_WITH if fixture.startswith("bundle_") else QT_COMPATIBLE_WITH,
        )
    native.test_suite(
        name = name,
        tests = [":" + name + "_" + suffix for suffix in ["unit", "module", "rust_module", "bridge", "compile", "moc", "uic", "widgets", "lrelease", "runpath", "test_runpath", "frameworks", "linux_frameworks", "bundle", "macos_runpath", "macos_test_runpath", "macos_bundle", "macos_library_link", "linux_library_link"] + sorted(failures.keys())],
    )
