"""The upstream rules_dart providers the quality aspects read, re-exported from one place."""

load("@rules_dart//dart:providers.bzl", _DartAnalyzableInfo = "DartAnalyzableInfo", _DartInfo = "DartInfo", _DartPackageInfo = "DartPackageInfo")

DartAnalyzableInfo = _DartAnalyzableInfo
DartInfo = _DartInfo
DartPackageInfo = _DartPackageInfo
