"""Blocked placeholder of the windows_arm64 cell."""

CELL = struct(
    archives = {},
    blocker = "no Windows layout, launcher or probe evidence is pinned for windows_arm64 (#5104, #5109)",
    constraints = ["@platforms//os:windows", "@platforms//cpu:arm64"],
    directory = "",
    layout = None,
    needs = None,
    release = "",
    runtime = None,
)
