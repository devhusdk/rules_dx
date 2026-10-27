"""Qt SDK pin, request and generated BUILD coverage."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":layout_linux.bzl", LINUX_LAYOUT = "LAYOUT")
load(
    ":pins.bzl",
    "CELLS",
    "DEFAULT_MODULES",
    "EXTRACTORS",
    "ICU_VERSION",
    "LAYOUTS",
    "MODULES",
    "QT_LIBRARY_LICENSE",
    "QT_TOOL_LICENSE",
    "QT_VERSION",
    "RUNTIME_MIRROR",
    "RUNTIME_REPOSITORIES",
)
load(
    ":repos.bzl",
    "HUB_NAME",
    "canonical_id_for",
    "cell_build",
    "cell_errors",
    "cell_from_json",
    "cell_repository_name",
    "cell_spec",
    "closure_libraries",
    "consumer_cells",
    "default_request",
    "effective_cells",
    "host_key",
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

def _package(cell, name):
    for package in CELLS[cell].runtime.packages:
        if package.name == name:
            return package
    fail("no runtime package " + name)

def _closure_packages(cell, closure):
    return [package.name for package in CELLS[cell].runtime.packages if package.closure == closure]

def _pin_checks():
    return [
        expect_equal("qt version", QT_VERSION, "6.11.3"),
        expect_equal("icu version", ICU_VERSION, "73.2"),
        expect_equal("x86_64 release", CELLS["linux_x86_64"].release, "6.11.3-0-202610220312"),
        expect_equal("x86_64 runtime", [CELLS["linux_x86_64"].runtime.arch, CELLS["linux_x86_64"].runtime.release], ["x86_64", "9.6"]),
        expect_equal("x86_64 glibc floor", CELLS["linux_x86_64"].runtime.glibc, "2.34"),
        expect_equal("x86_64 libstdc++ version", CELLS["linux_x86_64"].runtime.libstdcxx, "11.5.0"),
        expect_equal("x86_64 font", CELLS["linux_x86_64"].runtime.font.file, "dejavu-sans-fonts-2.37-18.el9.noarch.rpm"),
        expect_equal("arm64 release", CELLS["linux_arm64"].release, "6.11.3-0-202610231039"),
        expect_equal("arm64 runtime", [CELLS["linux_arm64"].runtime.arch, CELLS["linux_arm64"].runtime.release], ["aarch64", "10.1"]),
        expect_equal("arm64 glibc floor", CELLS["linux_arm64"].runtime.glibc, "2.39"),
        expect_equal("arm64 libstdc++ version", CELLS["linux_arm64"].runtime.libstdcxx, "14.3.0"),
        expect_equal("arm64 font", CELLS["linux_arm64"].runtime.font.file, "dejavu-sans-fonts-2.37-25.el10.noarch.rpm"),
        expect_equal("arm64 core runtime packages", _closure_packages("linux_arm64", "core"), [
            "glib2",
            "libbrotli",
            "libgcc",
            "libstdc++",
            "libzstd",
            "pcre2",
            "zlib-ng-compat",
        ]),
        expect_equal("arm64 display runtime packages", _closure_packages("linux_arm64", "display"), [
            "libICE",
            "libSM",
            "libX11-xcb",
            "libffi",
            "libuuid",
            "libwayland-client",
            "libwayland-cursor",
            "libxkbcommon-x11",
            "xcb-util",
            "xcb-util-cursor",
            "xcb-util-image",
            "xcb-util-keysyms",
            "xcb-util-renderutil",
            "xcb-util-wm",
        ]),
        expect_false("arm64 gui closure skips krb5", "krb5-libs" in _closure_packages("linux_arm64", "gui")),
        expect_equal("blocked cells carry no runtime", [cell for cell in CELLS if CELLS[cell].blocker != "" and (CELLS[cell].runtime != None or CELLS[cell].release != "")], []),
        expect_equal("linux cells share the linux layout", [cell for cell in qualified_cells() if CELLS[cell].layout != LINUX_LAYOUT], []),
        expect_equal("blocked cells name no layout", [cell for cell in CELLS if CELLS[cell].blocker != "" and CELLS[cell].layout != None], []),
        expect_equal("linux layout names libraries by soname", LINUX_LAYOUT.library("Core"), "libQt6Core.so.6"),
        expect_equal("extractor hosts", sorted(EXTRACTORS.keys()), ["linux_arm64", "linux_x86_64"]),
        expect_equal("runtime mirror is the vault root", RUNTIME_MIRROR, "https://vault.almalinux.org"),
        expect_equal("library license", QT_LIBRARY_LICENSE, "LicenseRef-Qt-Commercial OR LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only"),
        expect_equal("tool license", QT_TOOL_LICENSE, "LicenseRef-Qt-Commercial OR GPL-3.0-only WITH Qt-GPL-exception-1.0"),
        expect_equal("pin errors", pin_errors(), []),
        expect_equal("qualified cells", qualified_cells(), ["linux_arm64", "linux_x86_64"]),
        expect_equal("cells", sorted(CELLS.keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]),
        expect_equal("blocked cells carry no artifacts", [cell for cell in CELLS if CELLS[cell].blocker != "" and len(CELLS[cell].archives) > 0], []),
        expect_equal("modules", sorted(MODULES.keys()), ["qtbase", "qtdeclarative", "qttools"]),
        expect_equal("qttools keeps lrelease only", MODULES["qttools"].files, ["bin/lrelease", "modules/Linguist.json", "sbom/qttools-6.11.3.spdx.json"]),
        expect_equal("default modules", DEFAULT_MODULES, ["qtbase"]),
        expect_equal("platform plugins", LINUX_LAYOUT.platform_plugins, ["libqminimal.so", "libqoffscreen.so"]),
        expect_equal("display plugins", LINUX_LAYOUT.display_plugins, {
            "wayland": [
                "plugins/platforms/libqwayland.so",
                "plugins/wayland-decoration-client/libbradient.so",
                "plugins/wayland-shell-integration/libxdg-shell.so",
            ],
            "xcb": [
                "plugins/platforms/libqxcb.so",
                "plugins/xcbglintegrations/libqxcb-egl-integration.so",
                "plugins/xcbglintegrations/libqxcb-glx-integration.so",
            ],
        }),
        expect_equal("runtime plugins", LINUX_LAYOUT.runtime_plugins, {
            "imageformats": [
                "plugins/imageformats/libqgif.so",
                "plugins/imageformats/libqico.so",
                "plugins/imageformats/libqjpeg.so",
            ],
            "platformthemes": ["plugins/platformthemes/libqxdgdesktopportal.so"],
            "tls": [
                "plugins/tls/libqcertonlybackend.so",
                "plugins/tls/libqopensslbackend.so",
            ],
        }),
        expect_equal("display runtime packages", _closure_packages("linux_x86_64", "display"), [
            "libX11-xcb",
            "libffi",
            "libwayland-client",
            "libwayland-cursor",
            "libxkbcommon-x11",
            "xcb-util",
            "xcb-util-cursor",
            "xcb-util-image",
            "xcb-util-keysyms",
            "xcb-util-renderutil",
            "xcb-util-wm",
        ]),
        expect_equal("display closure sonames", closure_libraries("linux_x86_64", "display"), [
            "libX11-xcb.so.1",
            "libffi.so.8",
            "libwayland-client.so.0",
            "libwayland-cursor.so.0",
            "libxcb-cursor.so.0",
            "libxcb-glx.so.0",
            "libxcb-icccm.so.4",
            "libxcb-image.so.0",
            "libxcb-keysyms.so.1",
            "libxcb-randr.so.0",
            "libxcb-render-util.so.0",
            "libxcb-render.so.0",
            "libxcb-shape.so.0",
            "libxcb-shm.so.0",
            "libxcb-sync.so.1",
            "libxcb-util.so.1",
            "libxcb-xfixes.so.0",
            "libxcb-xkb.so.1",
            "libxkbcommon-x11.so.0",
        ]),
        expect_equal("runtime repositories", RUNTIME_REPOSITORIES, ["AppStream", "BaseOS"]),
        expect_equal("icu libraries", LINUX_LAYOUT.icu_libraries, ["libicudata.so.73", "libicui18n.so.73", "libicuuc.so.73"]),
        expect_equal("core runtime packages", _closure_packages("linux_x86_64", "core"), [
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
        expect_equal("gui runtime packages", _closure_packages("linux_x86_64", "gui"), [
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
        expect_equal("gui closure sonames", closure_libraries("linux_x86_64", "gui"), [
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
        expect_equal("core closure sonames", closure_libraries("linux_x86_64", "core"), [
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

def _layout(**kwargs):
    fields = {
        "display_plugins": {},
        "icu_libraries": [],
        "launcher": "posix",
        "library": LINUX_LAYOUT.library,
        "os": "macos",
        "platform_plugins": [],
        "plugin_glob": "",
        "runtime_glob": "",
        "runtime_plugins": {},
    }
    fields.update(kwargs)
    return struct(**fields)

def _cell(layout, runtime = None):
    archive = struct(archive = "6.11.3-0-1qt.7z", sha256 = "0" * 64)
    return struct(
        archives = {key: archive for key in MODULES},
        blocker = "",
        constraints = ["@platforms//os:macos", "@platforms//cpu:arm64"],
        directory = "mac_x64/desktop",
        layout = layout,
        needs = None,
        release = "6.11.3-0-1",
        runtime = runtime,
    )

def _seam_checks():
    return [
        expect_equal("linux host", [host_key("linux", "amd64"), host_key("linux", "aarch64")], ["linux_x86_64", "linux_arm64"]),
        expect_equal("macos host", [host_key("Mac OS X", "x86_64"), host_key("mac os x", "aarch64")], ["macos_x86_64", "macos_arm64"]),
        expect_equal("windows host", [host_key("windows 11", "amd64"), host_key("Windows 11", "aarch64")], ["windows_x86_64", "windows_arm64"]),
        expect_equal("windows server host", [host_key("windows server 2022", "amd64"), host_key("windows server 2022", "arm64")], ["windows_x86_64", "windows_arm64"]),
        expect_equal("non-linux cell needs no runtime closure or icu", cell_errors("macos_arm64", _cell(_layout())), []),
        expect_equal("linux cell needs a runtime closure and icu", cell_errors("linux_x", _cell(_layout(os = "linux", icu_libraries = ["libicuuc.so.73"]))), [
            "cell 'linux_x' pins no runtime closure",
            "cell 'linux_x' pins no 'icu' archive",
        ]),
        expect_equal("qualified cell needs a layout", cell_errors("bare", _cell(None)), ["cell 'bare' names no layout"]),
        expect_equal("layout launcher is known", cell_errors("odd", _cell(_layout(launcher = "win32"))), ["cell 'odd' layout launcher 'win32' is not one of posix"]),
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
            runtime_url_for("linux_x86_64", _package("linux_x86_64", "libstdc++")),
            "https://vault.almalinux.org/9.6/BaseOS/x86_64/os/Packages/libstdc%2B%2B-11.5.0-5.el9_5.alma.1.x86_64.rpm",
        ),
        expect_equal(
            "mirrored runtime url",
            runtime_url_for("linux_x86_64", _package("linux_x86_64", "zlib"), "https://mirror.example.com/alma"),
            "https://mirror.example.com/alma/9.6/BaseOS/x86_64/os/Packages/zlib-1.2.11-40.el9.x86_64.rpm",
        ),
        expect_equal(
            "AppStream runtime url",
            runtime_url_for("linux_x86_64", _package("linux_x86_64", "libX11")),
            "https://vault.almalinux.org/9.6/AppStream/x86_64/os/Packages/libX11-1.7.0-11.el9.x86_64.rpm",
        ),
        expect_equal(
            "arm64 qtbase url",
            url_for("linux_arm64", "qtbase"),
            "https://download.qt.io/online/qtsdkrepository/linux_arm64/desktop/qt6_6113/qt6_6113/qt.qt6.6113.linux_gcc_arm64/" +
            "6.11.3-0-202610231039qtbase-Linux-Ubuntu_24_04-GCC-Linux-Ubuntu_24_04-AARCH64.7z",
        ),
        expect_equal(
            "arm64 runtime url",
            runtime_url_for("linux_arm64", _package("linux_arm64", "libstdc++")),
            "https://vault.almalinux.org/10.1/BaseOS/aarch64/os/Packages/libstdc%2B%2B-14.3.1-2.1.el10.alma.1.aarch64.rpm",
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
    hub = hub_build({cell: cell_repository_name(cell) for cell in qualified_cells()}, "//qt/rules:defs.bzl")
    arm64 = cell_build("linux_arm64", ["qtbase"], "@rules_cc//cc:defs.bzl", "//qt/sdk:unsupported.bzl", "//qt/sdk:needs.bzl")
    base = cell_build("linux_x86_64", ["qtbase"], "@rules_cc//cc:defs.bzl", "//qt/sdk:unsupported.bzl", "//qt/sdk:needs.bzl")
    full = cell_build("linux_x86_64", ["qtbase", "qtdeclarative"], "@rules_cc//cc:defs.bzl", "//qt/sdk:unsupported.bzl", "//qt/sdk:needs.bzl")
    tools = cell_build("linux_x86_64", ["qtbase", "qtdeclarative", "qttools"], "@rules_cc//cc:defs.bzl", "//qt/sdk:unsupported.bzl", "//qt/sdk:needs.bzl")
    return [
        expect_equal("hub name", HUB_NAME, "dx_qt"),
        expect_equal("cell repository", cell_repository_name("linux_x86_64"), "dx_qt_linux_x86_64"),
        expect_equal("hub targets", hub_targets(), [
            "core",
            "display_wayland",
            "display_xcb",
            "fonts",
            "gui",
            "licenses",
            "lrelease",
            "metatypes",
            "moc",
            "needs",
            "network",
            "platforms",
            "plugins_imageformats",
            "plugins_platformthemes",
            "plugins_tls",
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
        expect_contains("hub loads the Rust runtime rule", hub, 'load("//qt/rules:defs.bzl", "qt_rust_runtime")\n'),
        expect_contains("hub declares the CXX-Qt runtime", hub, 'qt_rust_runtime(\n    name = "cxx_qt",\n    core = ":core",\n'),
        expect_contains("hub routes qmllint", hub, '":linux_x86_64": "@dx_qt_linux_x86_64//:qmllint"'),
        expect_contains("hub routes needs", hub, '":linux_x86_64": "@dx_qt_linux_x86_64//:needs"'),
        expect_contains("base cell loads qt_needs", base, 'load("//qt/sdk:needs.bzl", "qt_needs")\n'),
        expect_contains("base cell publishes its needs", base, 'qt_needs(\n    name = "needs",'),
        expect_contains("base cell needs carry the library table", base, '"libxcb.so.1": ["libXau.so.6"]'),
        expect_contains("hub routes arm64 qmllint", hub, '":linux_arm64": "@dx_qt_linux_arm64//:qmllint"'),
        expect_contains("arm64 core closure links zlib-ng", arm64, '"lib/libz.so.1"'),
        expect_false("arm64 core closure skips pcre", '"lib/libpcre.so.1"' in arm64),
        expect_contains("hub guards other platforms", hub, no_match_error()),
        expect_contains("no-match names blocked cells", no_match_error(), "macos_arm64 (no runtime closure or probe evidence is pinned for macos_arm64)"),
        expect_contains("no-match names the qualified cells", no_match_error(), "qualified: linux_arm64, linux_x86_64;"),
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
        expect_contains("base cell ships the xcb plugins", base, 'name = "display_xcb",\n    srcs = ["plugins/platforms/libqxcb.so", "plugins/xcbglintegrations/libqxcb-egl-integration.so", "plugins/xcbglintegrations/libqxcb-glx-integration.so"],'),
        expect_contains("base cell ships the wayland plugins", base, 'name = "display_wayland",\n    srcs = ["plugins/platforms/libqwayland.so", "plugins/wayland-decoration-client/libbradient.so", "plugins/wayland-shell-integration/libxdg-shell.so"],'),
        expect_contains("base cell ships the tls plugins", base, 'name = "plugins_tls",\n    srcs = ["plugins/tls/libqcertonlybackend.so", "plugins/tls/libqopensslbackend.so"],'),
        expect_contains("arm64 cell ships the image format plugins", arm64, 'name = "plugins_imageformats",\n    srcs = ["plugins/imageformats/libqgif.so", "plugins/imageformats/libqico.so", "plugins/imageformats/libqjpeg.so"],'),
        expect_false("gui closure leaves display libraries unlinked", '"lib/libxcb-cursor.so.0"' in base),
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
        checks = _pin_checks() + _seam_checks() + _url_checks() + _request_checks() + _build_checks() + _hardening_checks() + _consumer_checks(),
    )

_CUSTOM = [str(Label(constraint)) for constraint in ["@platforms//os:linux", "@platforms//cpu:x86_64", "//qt/sdk:custom"]]

def _spec(cell = "linux_x86_64"):
    return json.decode(json.encode(cell_spec(CELLS[cell])))

def _tag(spec, name = "custom", constraints = _CUSTOM, layout = "linux"):
    return struct(
        constraints = constraints,
        layout = layout,
        name = name,
        spec = spec if type(spec) == "string" else json.encode(spec),
    )

def _errors(spec, name = "custom", constraints = _CUSTOM):
    cells, errors = consumer_cells([_tag(spec, name, constraints)])
    return errors + pin_errors(effective_cells(cells))

def _hardening_checks():
    archive = _spec()
    archive["archives"]["qtbase"]["archive"] = archive["release"] + "../qtbase.7z"
    rpm = _spec()
    rpm["runtime"]["packages"][0]["file"] = "../" + rpm["runtime"]["packages"][0]["file"]
    name = _spec()
    name["runtime"]["packages"][0]["name"] = "../glib2"
    directory = _spec()
    directory["directory"] = "linux_x64/../../etc"
    rooted = _spec()
    rooted["directory"] = "/linux_x64"
    query = _spec()
    query["directory"] = "linux_x64?x=1"
    host = _spec()
    host["directory"] = "https://evil.example.com/qt"
    release = _spec()
    release["runtime"]["release"] = "9.6/../10"
    arch = _spec()
    arch["runtime"]["arch"] = "x86_64/x"
    library = _spec()
    library["runtime"]["packages"][0]["libraries"] = ["../../etc/passwd"]
    font = _spec()
    font["runtime"]["font"]["fonts"] = ["/etc/passwd"]
    license_dir = _spec()
    license_dir["runtime"]["packages"][0]["license_dir"] = "usr/share/licenses/.."
    text = _spec()
    text["runtime"]["license_texts"]["../escape"] = text["runtime"]["license_texts"]["pcre2/LICENCE"]
    first = _spec()["runtime"]["packages"][0]["name"]
    return [
        expect_contains("archive file is a plain name", _errors(archive), "cell 'custom' archive 'qtbase' file '" + archive["archives"]["qtbase"]["archive"] + "' is not a plain file name"),
        expect_contains("rpm file is a plain name", _errors(rpm), "runtime package '" + first + "' file '" + rpm["runtime"]["packages"][0]["file"] + "' is not a plain file name"),
        expect_contains("package name is plain", _errors(name), "runtime package '../glib2' name is not a plain package name"),
        expect_contains("directory has no parent segment", _errors(directory), "cell 'custom' directory 'linux_x64/../../etc' is not a relative url path"),
        expect_contains("directory is relative", _errors(rooted), "cell 'custom' directory '/linux_x64' is not a relative url path"),
        expect_contains("directory has no query", _errors(query), "cell 'custom' directory 'linux_x64?x=1' is not a relative url path"),
        expect_contains("directory names no host", _errors(host), "cell 'custom' directory 'https://evil.example.com/qt' is not a relative url path"),
        expect_contains("runtime release is a token", _errors(release), "cell 'custom' runtime release '9.6/../10' is not made of letters, digits, '.', '_' and '-'"),
        expect_contains("runtime arch is a token", _errors(arch), "cell 'custom' runtime arch 'x86_64/x' is not made of letters, digits, '.', '_' and '-'"),
        expect_contains("libraries are relative", _errors(library), "runtime package '" + first + "' path '../../etc/passwd' is not a relative path"),
        expect_contains("fonts are relative", _errors(font), "runtime package 'dejavu-sans-fonts' path '/etc/passwd' is not a relative path"),
        expect_contains("license directories are relative", _errors(license_dir), "runtime package '" + first + "' path 'usr/share/licenses/..' is not a relative path"),
        expect_contains("license texts are relative", _errors(text), "cell 'custom' license text '../escape' is not a relative path"),
    ]

def _consumer_checks():
    round_trip = []
    for cell in qualified_cells():
        entry, errors = cell_from_json(cell, _spec(cell), CELLS[cell].constraints, LAYOUTS["linux"])
        round_trip.append(expect_equal(cell + " spec decodes cleanly", errors, []))
        round_trip.append(expect_true(cell + " spec round trips", entry == CELLS[cell]))
    sha = _spec()
    sha["archives"]["qtbase"]["sha256"] = "abc"
    seven = _spec()
    seven["archives"]["qtbase"]["archive"] = seven["archives"]["qtbase"]["archive"].removesuffix(".7z") + ".zip"
    release = _spec()
    release["release"] = "6.8.3-0-1"
    missing = _spec()
    missing["archives"].pop("qttools")
    unsorted = _spec()
    unsorted["runtime"]["packages"] = list(reversed(unsorted["runtime"]["packages"]))
    http = _spec()
    http["runtime"]["license_texts"]["pcre2/LICENCE"]["url"] = "http://example.com/LICENCE"
    repository = _spec()
    repository["runtime"]["packages"][0]["repository"] = "Extras"
    first = repository["runtime"]["packages"][0]["name"]
    unknown = _spec()
    unknown["extra"] = 1
    typed = _spec()
    typed["directory"] = 3
    fontless = _spec()
    fontless["runtime"].pop("font")
    url = _spec()
    url["runtime"]["packages"][0]["url"] = "https://example.com/x.rpm"
    needless = _spec()
    needless.pop("needs")
    bare = effective_cells(consumer_cells([_tag(needless, "custom_x86_64")])[0])
    table = _spec()
    table["needs"]["library"] = ["libxcb.so.1"]
    listed = _spec()
    listed["needs"]["plugin"] = {"a.so": "libxcb.so.1"}
    partial = _spec()
    partial["needs"].pop("plugin")
    pinned = [str(Label("@platforms//os:linux")), str(Label("@platforms//cpu:x86_64"))]
    macos = effective_cells(consumer_cells([_tag(_spec(), "macos_arm64", ["@platforms//os:macos", "@platforms//cpu:arm64"])])[0])
    custom = effective_cells(consumer_cells([_tag(_spec(), "custom_x86_64")])[0])
    active = qualified_cells(custom)
    hub = hub_build({cell: cell_repository_name(cell) for cell in active}, "//qt/rules:defs.bzl", {cell: custom[cell].constraints for cell in active}, no_match_error(custom))
    return round_trip + [
        expect_equal("a clean consumer cell passes", _errors(_spec()), []),
        expect_equal("consumer sha256 is checked", _errors(sha), ["cell 'custom' archive 'qtbase' sha256 is not 64 hex characters"]),
        expect_equal("consumer archives are 7z", _errors(seven), ["cell 'custom' archive 'qtbase' is not a .7z archive"]),
        expect_contains("consumer release is checked", _errors(release), "cell 'custom' release '6.8.3-0-1' is not a 6.11.3 release"),
        expect_equal("consumer cells pin every module", _errors(missing), ["cell 'custom' pins no 'qttools' archive"]),
        expect_equal("consumer runtime packages are sorted", _errors(unsorted), ["cell 'custom' runtime packages are not sorted by name"]),
        expect_equal("consumer license texts use https", _errors(http), ["license text 'custom: pcre2/LICENCE' does not use https"]),
        expect_equal("consumer repositories are known", _errors(repository), ["runtime package '" + first + "' repository 'Extras' is not one of AppStream, BaseOS"]),
        expect_equal("consumer spec keys are known", _errors(unknown), ["cell 'custom' spec has unknown key 'extra'"]),
        expect_equal("consumer spec fields are typed", _errors(typed), ["cell 'custom' spec 'directory' is not a string"]),
        expect_equal("consumer runtime needs a font", _errors(fontless), ["cell 'custom' spec runtime is missing 'font'"]),
        expect_equal("consumer packages take no url", _errors(url), ["cell 'custom' spec runtime.packages[0] has unknown key 'url'"]),
        expect_equal("consumer needs are optional", _errors(needless), []),
        expect_equal("consumer needs tables are typed", _errors(table), ["cell 'custom' spec needs 'library' is not an object of string lists"]),
        expect_equal("consumer needs entries are string lists", _errors(listed), ["cell 'custom' spec needs 'plugin' is not an object of string lists"]),
        expect_equal("consumer needs carry every table", _errors(partial), ["cell 'custom' spec needs is missing 'plugin'"]),
        expect_equal("consumer cell names avoid needs", consumer_cells([_tag(_spec(), "needs")])[1], ["cell 'needs' name is taken by a @dx_qt target"]),
        expect_contains("consumer cell publishes its needs", cell_build("custom_x86_64", ["qtbase"], "@rules_cc//cc:defs.bzl", "//qt/sdk:unsupported.bzl", "//qt/sdk:needs.bzl", custom), 'qt_needs(\n    name = "needs",'),
        expect_contains("a needless consumer cell refuses bundling", cell_build("custom_x86_64", ["qtbase"], "@rules_cc//cc:defs.bzl", "//qt/sdk:unsupported.bzl", "//qt/sdk:needs.bzl", bare), "qt_unsupported(\n    name = \"needs\",\n    reason = \"cell 'custom_x86_64' spec carries no needs tables; qt_bundle cannot package it\","),
        expect_equal("consumer spec is JSON", _errors("{"), ["cell 'custom' spec is not a JSON object"]),
        expect_equal("consumer cell names are plain", consumer_cells([_tag(_spec(), "Bad-Name")])[1], ["cell 'Bad-Name' name is not made of lowercase letters, digits and '_'"]),
        expect_equal("consumer cell names avoid hub targets", consumer_cells([_tag(_spec(), "core")])[1], ["cell 'core' name is taken by a @dx_qt target"]),
        expect_equal("consumer layouts are known", consumer_cells([_tag(_spec(), layout = "windows")])[1], ["cell 'custom' layout 'windows' is not one of linux"]),
        expect_equal("consumer cells are declared once", consumer_cells([_tag(_spec()), _tag(_spec())])[1], ["cell 'custom' is declared twice"]),
        expect_equal("consumer constraints set the cell apart", consumer_cells([_tag(_spec(), constraints = pinned)])[1], ["cell 'custom' constraints match cell 'linux_x86_64'; add a constraint that sets it apart"]),
        expect_equal("a consumer cell replaces its pinned namesake", consumer_cells([_tag(_spec(), "linux_x86_64", ["@platforms//os:linux", "@platforms//cpu:x86_64"])])[1], []),
        expect_equal("an override qualifies a blocked cell", qualified_cells(macos), ["linux_arm64", "linux_x86_64", "macos_arm64"]),
        expect_false("an override leaves the blocked list", "macos_arm64 (" in no_match_error(macos)),
        expect_equal("a new consumer cell joins the qualified cells", active, ["custom_x86_64", "linux_arm64", "linux_x86_64"]),
        expect_contains("hub routes the consumer cell", hub, '":custom_x86_64": "@dx_qt_custom_x86_64//:core"'),
        expect_contains("hub matches the consumer constraint", hub, '        "%s",\n' % str(Label("//qt/sdk:custom"))),
        expect_contains("consumer cell builds core", cell_build("custom_x86_64", ["qtbase"], "@rules_cc//cc:defs.bzl", "//qt/sdk:unsupported.bzl", "//qt/sdk:needs.bzl", custom), '    srcs = ["lib/libQt6Core.so.6"],'),
    ]
