"""Public Qt API."""

load(
    "//qt/private:defs.bzl",
    _QT_CONSUMER_MIN_RUNTIME = "QT_CONSUMER_MIN_RUNTIME",
    _QT_CONSUMER_RUNTIME = "QT_CONSUMER_RUNTIME",
    _qt_bundle = "qt_bundle",
    _qt_cc_binary = "qt_cc_binary",
    _qt_cc_library = "qt_cc_library",
    _qt_cc_test = "qt_cc_test",
    _qt_frameworks = "qt_frameworks",
    _qt_qml_module = "qt_qml_module",
    _qt_rust_cc_library = "qt_rust_cc_library",
)

load("//qt/private:rust.bzl", _qt_rust_library = "qt_rust_library")

QT_CONSUMER_MIN_RUNTIME = _QT_CONSUMER_MIN_RUNTIME
QT_CONSUMER_RUNTIME = _QT_CONSUMER_RUNTIME
qt_bundle = _qt_bundle
qt_cc_binary = _qt_cc_binary
qt_cc_library = _qt_cc_library
qt_cc_test = _qt_cc_test
qt_frameworks = _qt_frameworks
qt_qml_module = _qt_qml_module
qt_rust_cc_library = _qt_rust_cc_library
qt_rust_library = _qt_rust_library
