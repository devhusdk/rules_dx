"""Public C and C++ module extensions."""

load("//cc/private/toolchains:extensions.bzl", _gcc = "gcc")
load("//cc/private/toolchains/msvc:extensions.bzl", _msvc = "msvc")

gcc = _gcc
msvc = _msvc
