"""Public Swift module extensions."""

load("//swift/private/lock:hub.bzl", _swiftpm = "swiftpm")
load("//swift/private/toolchain:extensions.bzl", _swift = "swift")

swift = _swift
swiftpm = _swiftpm
