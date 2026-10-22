"""Pinned Qt SDK, runtime closure and extractor identities."""

QT_VERSION = "6.11.3"

QT_RELEASE = "6.11.3-0-202609240253"

QT_MIRROR = "https://download.qt.io/online/qtsdkrepository"

QT_COMPILER = "GCC 11.5.0"

ICU_VERSION = "73.2"

ICU_LIBRARIES = ["libicudata.so.73", "libicui18n.so.73", "libicuuc.so.73"]

GLIBC_FLOOR = "2.34"

LIBSTDCXX_VERSION = "11.5.0"

RUNTIME_DISTRIBUTION = "AlmaLinux 9.6 BaseOS"

RUNTIME_MIRROR = "https://vault.almalinux.org/9.6/BaseOS/x86_64/os/Packages"

QT_LIBRARY_LICENSE = "LicenseRef-Qt-Commercial OR LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only"

QT_TOOL_LICENSE = "LicenseRef-Qt-Commercial OR GPL-3.0-only WITH Qt-GPL-exception-1.0"

ICU_LICENSE = "Unicode-DFS-2016"

MODULES = {
    "qtbase": struct(
        requires = [],
        targets = ["core", "metatypes", "moc", "network", "rcc", "test", "uic"],
    ),
    "qtdeclarative": struct(
        requires = ["qtbase"],
        targets = ["qml", "qml_imports", "qmlformat", "qmllint", "qmltyperegistrar", "qtqml_qmldir"],
    ),
}

DEFAULT_MODULES = ["qtbase"]

UNSUPPORTED_TARGETS = {
    "gui": "Qt Gui needs libEGL, libGL, fontconfig, freetype, X11, xkbcommon and D-Bus, which the pinned runtime closure does not carry",
    "quick": "Qt Quick needs Qt Gui and an OpenGL or software scene graph, which the pinned runtime closure does not carry",
    "widgets": "Qt Widgets needs Qt Gui and a platform plugin, which the pinned runtime closure does not carry",
}

CELLS = {
    "linux_arm64": struct(
        archives = {},
        blocker = "no runtime closure or probe evidence is pinned for linux_arm64",
        constraints = ["@platforms//os:linux", "@platforms//cpu:arm64"],
        directory = "",
    ),
    "linux_x86_64": struct(
        archives = {
            "icu": struct(
                archive = QT_RELEASE + "icu-linux-Rhel8.6-x86_64.7z",
                sha256 = "9021d0ef1063d0c6940e98c4793467aa11660b9bbd5df13b0ba060b212e8762e",
            ),
            "qtbase": struct(
                archive = QT_RELEASE + "qtbase-Linux-RHEL_9_6-GCC-Linux-RHEL_9_6-X86_64.7z",
                sha256 = "fadc26e718ad109bb0f9b9a301885b5888426703ccaf8fa83d23fc90881fcf97",
            ),
            "qtdeclarative": struct(
                archive = QT_RELEASE + "qtdeclarative-Linux-RHEL_9_6-GCC-Linux-RHEL_9_6-X86_64.7z",
                sha256 = "1f216e0ef75423fcb345ca3c14579b56269ef001cada91553c016f86fcde5046",
            ),
        },
        blocker = "",
        constraints = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        directory = "linux_x64/desktop/qt6_6113/qt6_6113/qt.qt6.6113.linux_gcc_64",
    ),
    "macos_arm64": struct(
        archives = {},
        blocker = "no runtime closure or probe evidence is pinned for macos_arm64",
        constraints = ["@platforms//os:macos", "@platforms//cpu:arm64"],
        directory = "",
    ),
    "windows_x86_64": struct(
        archives = {},
        blocker = "no runtime closure or probe evidence is pinned for windows_x86_64",
        constraints = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
        directory = "",
    ),
}

EXTRACTOR = struct(
    archive = "7z2603-linux-x64.tar.xz",
    binary = "7zzs",
    sha256 = "dc99eff5008f1ab79bd7084c68513701547a808a89502bf4133683535ab3c695",
    url = "https://github.com/ip7z/7zip/releases/download/26.03/7z2603-linux-x64.tar.xz",
)

RUNTIME_PACKAGES = [
    struct(
        file = "glib2-2.68.4-16.el9_6.3.x86_64.rpm",
        libraries = ["usr/lib64/libglib-2.0.so.0", "usr/lib64/libgthread-2.0.so.0"],
        license = "LGPL-2.1-or-later",
        license_dir = "usr/share/licenses/glib2",
        license_from = "glib2",
        name = "glib2",
        sha256 = "527db28ac9c2163061d84af68f0310bcbc8350e938d9bc16e834b4a2febb9436",
    ),
    struct(
        file = "keyutils-libs-1.6.3-1.el9.x86_64.rpm",
        libraries = ["usr/lib64/libkeyutils.so.1"],
        license = "GPL-2.0-or-later AND LGPL-2.1-or-later",
        license_dir = "usr/share/licenses/keyutils-libs",
        license_from = "keyutils-libs",
        name = "keyutils-libs",
        sha256 = "b0f15b1712150c0634ebc4aba29671539dd9c94f2350190a83e34e678168d0ac",
    ),
    struct(
        file = "krb5-libs-1.21.1-8.el9_6.x86_64.rpm",
        libraries = [
            "usr/lib64/libgssapi_krb5.so.2",
            "usr/lib64/libk5crypto.so.3",
            "usr/lib64/libkrb5.so.3",
            "usr/lib64/libkrb5support.so.0",
        ],
        license = "MIT",
        license_dir = "usr/share/licenses/krb5-libs",
        license_from = "krb5-libs",
        name = "krb5-libs",
        sha256 = "d1237fb8b81e9630869b03952281045b21805bb87f89481c4eadae0a9564dec2",
    ),
    struct(
        file = "libbrotli-1.0.9-7.el9_5.x86_64.rpm",
        libraries = ["usr/lib64/libbrotlicommon.so.1", "usr/lib64/libbrotlidec.so.1"],
        license = "MIT",
        license_dir = "usr/share/licenses/libbrotli",
        license_from = "libbrotli",
        name = "libbrotli",
        sha256 = "77906ae44bcd42a9f6646d814874768e8befaabf413133d44f71a95ac6c6d68c",
    ),
    struct(
        file = "libcom_err-1.46.5-7.el9.x86_64.rpm",
        libraries = ["usr/lib64/libcom_err.so.2"],
        license = "MIT",
        license_dir = "usr/share/licenses/libcom_err",
        license_from = "libcom_err",
        name = "libcom_err",
        sha256 = "937d1d0911c263f39f0c0d8e3f0346ae06abe3bddd527294e492f77acc5841de",
    ),
    struct(
        file = "libgcc-11.5.0-5.el9_5.alma.1.x86_64.rpm",
        libraries = ["lib64/libgcc_s.so.1"],
        license = "GPL-3.0-or-later WITH GCC-exception-3.1",
        license_dir = "usr/share/licenses/libgcc",
        license_from = "libgcc",
        name = "libgcc",
        sha256 = "7bd3ec130e1712442121b61b27164386006b9bfa6f0fa188316ecb35575a768f",
    ),
    struct(
        file = "libselinux-3.6-3.el9.x86_64.rpm",
        libraries = ["usr/lib64/libselinux.so.1"],
        license = "LicenseRef-Fedora-Public-Domain",
        license_dir = "usr/share/licenses/libselinux",
        license_from = "libselinux",
        name = "libselinux",
        sha256 = "636371a0ead5a8c5e5dfcad34c35d95d1ad2c328a654fa2c2cbd12b7bd6948c3",
    ),
    struct(
        file = "libstdc++-11.5.0-5.el9_5.alma.1.x86_64.rpm",
        libraries = ["usr/lib64/libstdc++.so.6"],
        license = "GPL-3.0-or-later WITH GCC-exception-3.1",
        license_dir = "usr/share/licenses/libgcc",
        license_from = "libgcc",
        name = "libstdc++",
        sha256 = "7efb1ded0ec2763589dd3bc6a9d52fa9f898cc1f3db6e8761e1d223ce4506424",
    ),
    struct(
        file = "libzstd-1.5.5-1.el9.x86_64.rpm",
        libraries = ["usr/lib64/libzstd.so.1"],
        license = "BSD-3-Clause AND GPL-2.0-only",
        license_dir = "usr/share/licenses/libzstd",
        license_from = "libzstd",
        name = "libzstd",
        sha256 = "33006cb9736d17f74c7a30cf9375438a9228023639239bf69c84b2530ad97462",
    ),
    struct(
        file = "openssl-libs-3.2.2-6.el9_5.1.x86_64.rpm",
        libraries = ["usr/lib64/libcrypto.so.3"],
        license = "Apache-2.0",
        license_dir = "usr/share/licenses/openssl-libs",
        license_from = "openssl-libs",
        name = "openssl-libs",
        sha256 = "cf0e0916f0c38b3e75111cbf1ebeddd97f828e02f8c50c378c4463c22b6c0093",
    ),
    struct(
        file = "pcre-8.44-4.el9.x86_64.rpm",
        libraries = ["usr/lib64/libpcre.so.1"],
        license = "BSD-3-Clause",
        license_dir = "usr/share/licenses/pcre",
        license_from = "pcre",
        name = "pcre",
        sha256 = "6cf80404b6bce41768313d9cc99a2bbc15dff0d682680f52481588030b8e2d64",
    ),
    struct(
        file = "pcre2-10.40-6.el9.x86_64.rpm",
        libraries = ["usr/lib64/libpcre2-8.so.0"],
        license = "BSD-3-Clause",
        license_dir = "",
        license_from = "",
        name = "pcre2",
        sha256 = "0cde5e36e904affa6feb6ce92f5c166774ddaaa6c7ffb4fbd1ff26881cde8b5a",
    ),
    struct(
        file = "zlib-1.2.11-40.el9.x86_64.rpm",
        libraries = ["usr/lib64/libz.so.1"],
        license = "Zlib AND BSL-1.0",
        license_dir = "usr/share/licenses/zlib",
        license_from = "zlib",
        name = "zlib",
        sha256 = "90d7d5d145c6d397645ffefdff40e3d2783e3bf3bfa7b6c1f089c2cad81a3191",
    ),
]

LICENSE_TEXTS = {
    "icu/LICENSE": struct(
        sha256 = "f3005e195ff74d8812cc1f182a1c446fab678d70a10e3dada497585befee5416",
        url = "https://raw.githubusercontent.com/unicode-org/icu/release-73-2/icu4c/LICENSE",
    ),
    "pcre2/LICENCE": struct(
        sha256 = "87d884eceb7fc54611470ce9f74280d28612b0c877adfc767e9676892a638987",
        url = "https://raw.githubusercontent.com/PCRE2Project/pcre2/pcre2-10.40/LICENCE",
    ),
    "qt/GPL-2.0-only.txt": struct(
        sha256 = "8177f97513213526df2cf6184d8ff986c675afb514d4e68a404010521b880643",
        url = "https://raw.githubusercontent.com/qt/qtbase/5a1194b2d368e2d72c230205c464b04a2f23eccd/LICENSES/GPL-2.0-only.txt",
    ),
    "qt/GPL-3.0-only.txt": struct(
        sha256 = "8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903",
        url = "https://raw.githubusercontent.com/qt/qtbase/5a1194b2d368e2d72c230205c464b04a2f23eccd/LICENSES/GPL-3.0-only.txt",
    ),
    "qt/LGPL-3.0-only.txt": struct(
        sha256 = "da7eabb7bafdf7d3ae5e9f223aa5bdc1eece45ac569dc21b3b037520b4464768",
        url = "https://raw.githubusercontent.com/qt/qtbase/5a1194b2d368e2d72c230205c464b04a2f23eccd/LICENSES/LGPL-3.0-only.txt",
    ),
    "qt/LicenseRef-Qt-Commercial.txt": struct(
        sha256 = "9b1f50aae6267f9d5e0ceb6775ee86450262c25ec7c0573e151fe5d3f18a4700",
        url = "https://raw.githubusercontent.com/qt/qtbase/5a1194b2d368e2d72c230205c464b04a2f23eccd/LICENSES/LicenseRef-Qt-Commercial.txt",
    ),
    "qt/Qt-GPL-exception-1.0.txt": struct(
        sha256 = "40678d338ce53cd93f8b22b281a2ecbcaa3ee65ce60b25ffb0c462b0530846b2",
        url = "https://raw.githubusercontent.com/qt/qtbase/5a1194b2d368e2d72c230205c464b04a2f23eccd/LICENSES/Qt-GPL-exception-1.0.txt",
    ),
}
