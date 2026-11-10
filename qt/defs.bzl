"""Public Qt API."""

load(
    "//qt/private:defs.bzl",
    _QT_BUNDLE_COMPATIBLE_WITH = "QT_BUNDLE_COMPATIBLE_WITH",
    _QT_BUNDLE_MACOS_RUNPATH = "QT_BUNDLE_MACOS_RUNPATH",
    _QT_BUNDLE_RUNPATH = "QT_BUNDLE_RUNPATH",
    _QT_COMPATIBLE_WITH = "QT_COMPATIBLE_WITH",
    _QT_CONF = "QT_CONF",
    _QT_CONSUMER_MIN_RUNTIME = "QT_CONSUMER_MIN_RUNTIME",
    _QT_CONSUMER_RUNTIME = "QT_CONSUMER_RUNTIME",
    _QT_REPOSITORY = "QT_REPOSITORY",
    _QT_SOURCE_SPECS = "QT_SOURCE_SPECS",
    _qt_bundle = "qt_bundle",
    _qt_bundle_members = "qt_bundle_members",
    _qt_cc_binary = "qt_cc_binary",
    _qt_cc_library = "qt_cc_library",
    _qt_cc_test = "qt_cc_test",
    _qt_consumer_identity = "qt_consumer_identity",
    _qt_framework_path = "qt_framework_path",
    _qt_frameworks = "qt_frameworks",
    _qt_identifier = "qt_identifier",
    _qt_library_stem = "qt_library_stem",
    _qt_module_path = "qt_module_path",
    _qt_os_supplied_libraries = "qt_os_supplied_libraries",
    _qt_package_path = "qt_package_path",
    _qt_qml_module = "qt_qml_module",
    _qt_qmldir = "qt_qmldir",
    _qt_qrc = "qt_qrc",
    _qt_runtime_members = "qt_runtime_members",
    _qt_rust_cc_library = "qt_rust_cc_library",
    _qt_rust_runtime = "qt_rust_runtime",
    _qt_version_parts = "qt_version_parts",
)

load("//qt/private:rust.bzl", _qt_rust_library = "qt_rust_library")

QT_BUNDLE_COMPATIBLE_WITH = _QT_BUNDLE_COMPATIBLE_WITH
QT_BUNDLE_MACOS_RUNPATH = _QT_BUNDLE_MACOS_RUNPATH
QT_BUNDLE_RUNPATH = _QT_BUNDLE_RUNPATH
QT_COMPATIBLE_WITH = _QT_COMPATIBLE_WITH
QT_CONF = _QT_CONF
QT_CONSUMER_MIN_RUNTIME = _QT_CONSUMER_MIN_RUNTIME
QT_CONSUMER_RUNTIME = _QT_CONSUMER_RUNTIME
QT_REPOSITORY = _QT_REPOSITORY
QT_SOURCE_SPECS = _QT_SOURCE_SPECS
qt_bundle = _qt_bundle
qt_bundle_members = _qt_bundle_members
qt_cc_binary = _qt_cc_binary
qt_cc_library = _qt_cc_library
qt_cc_test = _qt_cc_test
qt_consumer_identity = _qt_consumer_identity
qt_framework_path = _qt_framework_path
qt_frameworks = _qt_frameworks
qt_identifier = _qt_identifier
qt_library_stem = _qt_library_stem
qt_module_path = _qt_module_path
qt_os_supplied_libraries = _qt_os_supplied_libraries
qt_package_path = _qt_package_path
qt_qml_module = _qt_qml_module
qt_qmldir = _qt_qmldir
qt_qrc = _qt_qrc
qt_runtime_members = _qt_runtime_members
qt_rust_cc_library = _qt_rust_cc_library
qt_rust_library = _qt_rust_library
qt_rust_runtime = _qt_rust_runtime
qt_version_parts = _qt_version_parts
