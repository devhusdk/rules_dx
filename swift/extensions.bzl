"""Public Swift module extensions."""

load("//swift/private/lock:hub.bzl", _swiftpm = "swiftpm")
load("//swift/private/toolchain:extensions.bzl", _swift_linux = "swift_linux", _swift_windows = "swift_windows")

swift_linux = _swift_linux
swift_windows = _swift_windows
swiftpm = _swiftpm
