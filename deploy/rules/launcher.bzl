"""Shared deploy-launcher helpers, re-exported from config.bzl until every kind reads a deploy config."""

load(":config.bzl", _package_rlocation = "package_rlocation", _rlocation_path = "rlocation_path")

rlocation_path = _rlocation_path
package_rlocation = _package_rlocation
