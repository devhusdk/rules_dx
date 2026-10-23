"""Qt SDK pin, request and generated BUILD coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(
    ":pins.bzl",
    "CELLS",
    "DEFAULT_MODULES",
    "GLIBC_FLOOR",
    "ICU_LIBRARIES",
    "ICU_VERSION",
    "LIBSTDCXX_VERSION",
    "MODULES",
    "PLATFORM_PLUGINS",
    "QT_LIBRARY_LICENSE",
    "QT_TOOL_LICENSE",
    "QT_VERSION",
    "RUNTIME_PACKAGES",
    "RUNTIME_REPOSITORIES",
)
load(
    ":repos.bzl",
    "HUB_NAME",
    "canonical_id_for",
    "cell_build",
    "cell_repository_name",
    "closure_libraries",
    "default_request",
    "hub_build",
    "hub_targets",
    "no_match_error",
    "pin_errors",
    "qualified_cells",
    "request_errors",
    "resolve_modules",
    "runtime_url_for",
    "url_for",
)

def _request(**kwargs):
    base = default_request()
    return struct(**{key: kwargs.get(key, getattr(base, key)) for key in [
        "mirror",
        "modules",
        "runtime_mirror",
        "version",
    ]})

def _package(name):
    for package in RUNTIME_PACKAGES:
        if package.name == name:
            return package
    fail("no runtime package " + name)

def _pin_checks():
    return [
        expect_equal("qt version", QT_VERSION, "6.11.3"),
        expect_equal("icu version", ICU_VERSION, "73.2"),
        expect_equal("glibc floor", GLIBC_FLOOR, "2.34"),
        expect_equal("libstdc++ version", LIBSTDCXX_VERSION, "11.5.0"),
        expect_equal("library license", QT_LIBRARY_LICENSE, "LicenseRef-Qt-Commercial OR LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only"),
        expect_equal("tool license", QT_TOOL_LICENSE, "LicenseRef-Qt-Commercial OR GPL-3.0-only WITH Qt-GPL-exception-1.0"),
        expect_equal("pin errors", pin_errors(), []),
        expect_equal("qualified cells", qualified_cells(), ["linux_x86_64"]),
        expect_equal("cells", sorted(CELLS.keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]),
        expect_equal("blocked cells carry no artifacts", [cell for cell in CELLS if CELLS[cell].blocker != "" and len(CELLS[cell].archives) > 0], []),
        expect_equal("modules", sorted(MODULES.keys()), ["qtbase", "qtdeclarative", "qttools"]),
        expect_equal("qttools keeps lrelease only", MODULES["qttools"].files, ["bin/lrelease", "modules/Linguist.json", "sbom/qttools-6.11.3.spdx.json"]),
        expect_equal("default modules", DEFAULT_MODULES, ["qtbase"]),
        expect_equal("platform plugins", PLATFORM_PLUGINS, ["libqminimal.so", "libqoffscreen.so"]),
        expect_equal("runtime repositories", RUNTIME_REPOSITORIES, ["AppStream", "BaseOS"]),
        expect_equal("icu libraries", ICU_LIBRARIES, ["libicudata.so.73", "libicui18n.so.73", "libicuuc.so.73"]),
        expect_equal("core runtime packages", [package.name for package in RUNTIME_PACKAGES if package.closure == "core"], [
            "glib2",
            "keyutils-libs",
            "krb5-libs",
            "libbrotli",
            "libcom_err",
            "libgcc",
            "libselinux",
            "libstdc++",
            "libzstd",
            "openssl-libs",
            "pcre",
            "pcre2",
            "zlib",
        ]),
        expect_equal("gui runtime packages", [package.name for package in RUNTIME_PACKAGES if package.closure == "gui"], [
            "bzip2-libs",
            "dbus-libs",
            "fontconfig",
            "freetype",
            "graphite2",
            "harfbuzz",
            "libX11",
            "libXau",
            "libXext",
            "libcap",
            "libgcrypt",
            "libglvnd",
            "libglvnd-egl",
            "libglvnd-glx",
            "libgpg-error",
            "libpng",
            "libxcb",
            "libxkbcommon",
            "libxml2",
            "lz4-libs",
            "systemd-libs",
            "xz-libs",
        ]),
        expect_equal("gui closure sonames", closure_libraries("gui"), [
            "libEGL.so.1",
            "libGL.so.1",
            "libGLX.so.0",
            "libGLdispatch.so.0",
            "libX11.so.6",
            "libXau.so.6",
            "libXext.so.6",
            "libbz2.so.1",
            "libcap.so.2",
            "libdbus-1.so.3",
            "libfontconfig.so.1",
            "libfreetype.so.6",
            "libgcrypt.so.20",
            "libgpg-error.so.0",
            "libgraphite2.so.3",
            "libharfbuzz.so.0",
            "liblz4.so.1",
            "liblzma.so.5",
            "libpng16.so.16",
            "libsystemd.so.0",
            "libxcb.so.1",
            "libxkbcommon.so.0",
            "libxml2.so.2",
        ]),
        expect_equal("core closure sonames", closure_libraries("core"), [
            "libbrotlicommon.so.1",
            "libbrotlidec.so.1",
            "libcom_err.so.2",
            "libcrypto.so.3",
            "libgcc_s.so.1",
            "libglib-2.0.so.0",
            "libgssapi_krb5.so.2",
            "libgthread-2.0.so.0",
            "libicudata.so.73",
            "libicui18n.so.73",
            "libicuuc.so.73",
            "libk5crypto.so.3",
            "libkeyutils.so.1",
            "libkrb5.so.3",
            "libkrb5support.so.0",
            "libpcre.so.1",
            "libpcre2-8.so.0",
            "libselinux.so.1",
            "libstdc++.so.6",
            "libz.so.1",
            "libzstd.so.1",
        ]),
    ]

def _url_checks():
    return [
        expect_equal(
            "qtbase url",
            url_for("linux_x86_64", "qtbase"),
            "https://download.qt.io/online/qtsdkrepository/linux_x64/desktop/qt6_6113/qt6_6113/qt.qt6.6113.linux_gcc_64/" +
            "6.11.3-0-202610220312qtbase-Linux-RHEL_9_6-GCC-Linux-RHEL_9_6-X86_64.7z",
        ),
        expect_equal(
            "mirrored icu url",
            url_for("linux_x86_64", "icu", "https://mirror.example.com/qt"),
            "https://mirror.example.com/qt/linux_x64/desktop/qt6_6113/qt6_6113/qt.qt6.6113.linux_gcc_64/" +
            "6.11.3-0-202610220312icu-linux-Rhel8.6-x86_64.7z",
        ),
        expect_equal(
            "runtime url escapes plus",
            runtime_url_for(_package("libstdc++")),
            "https://vault.almalinux.org/9.6/BaseOS/x86_64/os/Packages/libstdc%2B%2B-11.5.0-5.el9_5.alma.1.x86_64.rpm",
        ),
        expect_equal(
            "mirrored runtime url",
            runtime_url_for(_package("zlib"), "https://mirror.example.com/alma"),
            "https://mirror.example.com/alma/BaseOS/x86_64/os/Packages/zlib-1.2.11-40.el9.x86_64.rpm",
        ),
        expect_equal(
            "AppStream runtime url",
            runtime_url_for(_package("libX11")),
            "https://vault.almalinux.org/9.6/AppStream/x86_64/os/Packages/libX11-1.7.0-11.el9.x86_64.rpm",
        ),
        expect_equal("cache identity", canonical_id_for("https://x/y.7z"), "dx-qt-archive:https://x/y.7z"),
    ]

def _request_checks():
    return [
        expect_equal("default request is clean", request_errors(default_request()), []),
        expect_equal("other version fails", request_errors(_request(version = "6.8.3")), [
            "qt_sdk: version '6.8.3' is not pinned; want 6.11.3",
        ]),
        expect_equal("unknown module fails", request_errors(_request(modules = ["qtmultimedia"])), [
            "qt_sdk: unknown module 'qtmultimedia'; want one of qtbase, qtdeclarative, qttools",
        ]),
        expect_equal("empty modules fail", request_errors(_request(modules = [])), [
            "qt_sdk: request at least one module",
        ]),
        expect_equal("plain http mirrors fail", request_errors(_request(mirror = "http://a", runtime_mirror = "http://b")), [
            "qt_sdk: mirror 'http://a' must use https",
            "qt_sdk: runtime_mirror 'http://b' must use https",
        ]),
        expect_equal("https mirrors pass", request_errors(_request(mirror = "https://a", runtime_mirror = "https://b")), []),
        expect_equal("qtbase alone", resolve_modules(["qtbase"]), ["qtbase"]),
        expect_equal("qtdeclarative pulls qtbase", resolve_modules(["qtdeclarative"]), ["qtbase", "qtdeclarative"]),
        expect_equal("qttools pulls qtdeclarative", resolve_modules(["qttools"]), ["qtbase", "qtdeclarative", "qttools"]),
        expect_equal("duplicates collapse", resolve_modules(["qtdeclarative", "qtbase", "qtdeclarative"]), ["qtbase", "qtdeclarative"]),
    ]

def _build_checks():
    hub = hub_build({"linux_x86_64": cell_repository_name("linux_x86_64")})
    base = cell_build(["qtbase"], "@rules_cc//cc:defs.bzl", "//qt/sdk:unsupported.bzl")
    full = cell_build(["qtbase", "qtdeclarative"], "@rules_cc//cc:defs.bzl", "//qt/sdk:unsupported.bzl")
    tools = cell_build(["qtbase", "qtdeclarative", "qttools"], "@rules_cc//cc:defs.bzl", "//qt/sdk:unsupported.bzl")
    return [
        expect_equal("hub name", HUB_NAME, "dx_qt"),
        expect_equal("cell repository", cell_repository_name("linux_x86_64"), "dx_qt_linux_x86_64"),
        expect_equal("hub targets", hub_targets(), [
            "core",
            "fonts",
            "gui",
            "licenses",
            "lrelease",
            "metatypes",
            "moc",
            "network",
            "platforms",
            "qml",
            "qml_imports",
            "qmlformat",
            "qmllint",
            "qmltyperegistrar",
            "qtqml_qmldir",
            "quick",
            "quick_test",
            "rcc",
            "runtime",
            "sbom",
            "test",
            "uic",
            "widgets",
        ]),
        expect_contains("hub routes qmllint", hub, '":linux_x86_64": "@dx_qt_linux_x86_64//:qmllint"'),
        expect_contains("hub guards other platforms", hub, no_match_error()),
        expect_contains("no-match names blocked cells", no_match_error(), "macos_arm64 (no runtime closure or probe evidence is pinned for macos_arm64)"),
        expect_contains("no-match names the qualified cell", no_match_error(), "qualified: linux_x86_64;"),
        expect_contains("base cell links core", base, '    srcs = ["lib/libQt6Core.so.6"],'),
        expect_contains("base cell refuses qml", base, "    name = \"qml\",\n    reason = \"enable module 'qtdeclarative' in qt_sdk.configure(modules = ...)\","),
        expect_false("base cell builds no qmllint", 'srcs = ["launcher/launcher.c", "launcher/qmllint.c"]' in base),
        expect_contains("full cell launches qmllint", full, 'srcs = ["launcher/launcher.c", "launcher/qmllint.c"]'),
        expect_contains("qmllint carries its plugins", full, 'data = ["bin/qmllint", ":plugins", ":qml_imports", ":runtime"],'),
        expect_contains("base cell launches moc from libexec", base, 'data = ["libexec/moc", ":runtime"],'),
        expect_contains("base cell launches rcc", base, 'srcs = ["launcher/launcher.c", "launcher/rcc.c"]'),
        expect_contains("base cell launches uic", base, 'srcs = ["launcher/launcher.c", "launcher/uic.c"]'),
        expect_contains("base cell links QtTest", base, '    srcs = ["lib/libQt6Test.so.6"],'),
        expect_contains("base cell ships metatypes", base, 'srcs = glob(["metatypes/*.json"]),'),
        expect_false("base cell builds no qmltyperegistrar", "launcher/qmltyperegistrar.c" in base),
        expect_contains("full cell launches qmltyperegistrar", full, 'data = ["libexec/qmltyperegistrar", ":runtime"],'),
        expect_contains("full cell ships qml headers", full, '"include/QtQml/**", "include/QtQmlIntegration/**"'),
        expect_contains("full cell refuses lrelease", full, "    name = \"lrelease\",\n    reason = \"enable module 'qttools' in qt_sdk.configure(modules = ...)\","),
        expect_contains("tools cell launches lrelease", tools, 'data = ["bin/lrelease", ":runtime"],'),
        expect_contains("base cell links widgets", base, '    srcs = ["lib/libQt6Widgets.so.6"],'),
        expect_contains("base cell defines the module macro", base, '    defines = ["QT_WIDGETS_LIB"],'),
        expect_contains("gui links its closure", base, '    deps = [":dbus", ":core", ":gui_closure"],'),
        expect_contains("gui closure links fontconfig", base, '        "lib/libfontconfig.so.1",'),
        expect_contains("base cell ships the pinned fonts", base, '"fonts/fonts.conf"'),
        expect_contains("base cell ships headless platforms", base, '["plugins/platforms/libqminimal.so", "plugins/platforms/libqoffscreen.so"]'),
        expect_false("base cell builds no quick", '"lib/libQt6Quick.so.6"' in base),
        expect_contains("base cell refuses quick", base, "    name = \"quick\",\n    reason = \"enable module 'qtdeclarative' in qt_sdk.configure(modules = ...)\","),
        expect_contains("full cell links quick", full, '    srcs = ["lib/libQt6Quick.so.6"],'),
        expect_contains("full cell links quick_test", full, '    srcs = ["lib/libQt6QuickTest.so.6"],'),
        expect_false("full cell builds gui", 'qt_unsupported(\n    name = "gui",' in full),
        expect_false("closure leaves libstdc++ to the toolchain", '"lib/libstdc++.so.6"' in full),
        expect_true("closure links libgcc_s", '"lib/libgcc_s.so.1"' in full),
    ]

def qt_metadata_tests(name):
    """Declares the Qt SDK metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _pin_checks() + _url_checks() + _request_checks() + _build_checks(),
    )
