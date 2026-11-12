"""Shared helpers for the web component languages."""

load("@bazel_skylib//rules:copy_file.bzl", "copy_file")

def dx_web_app_logo(name = "logo"):
    """Places the shared web app logo at src/assets/logo.png in the calling package."""
    copy_file(
        name = name,
        src = Label("//tools/testing:web_app_logo.png"),
        out = "src/assets/logo.png",
        testonly = False,
    )
