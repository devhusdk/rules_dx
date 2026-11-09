"""Quality tool rows for the oci_image family."""

load("//core/quality:tool_defs.bzl", "row", "tool")

TOOLS = [
    tool(
        id = "container_structure_test",
        adapters = {"typecheck": ["oci_image"]},
        support = [
            row(
                capability = "typecheck",
                classes = ["oci_image"],
                family = "oci_image",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                transport = "single",
            ),
        ],
    ),
    tool(
        id = "trivy_image_config",
        adapters = {"lint": ["oci_image"]},
        concerns = {"lint": "security"},
        support = [
            row(
                capability = "lint",
                classes = ["oci_image"],
                family = "oci_image",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                transport = "single",
            ),
        ],
    ),
]
