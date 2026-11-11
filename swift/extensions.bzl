"""Public Swift module extensions."""

load("//swift/private/lock:hub.bzl", _swiftpm = "swiftpm")
load("//swift/private/toolchain:extensions.bzl", _swift_windows = "swift_windows")

swift_windows = _swift_windows
swiftpm = _swiftpm
