"""Public C and C++ module extensions."""

load("//cc/private/toolchain:extensions.bzl", _gcc = "gcc")
load("//cc/private/toolchain/msvc:extensions.bzl", _msvc = "msvc")

gcc = _gcc
msvc = _msvc
