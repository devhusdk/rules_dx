"""Validators for the image deploy rule."""

IMAGE_BASES = {
    "cc": "@distroless_cc",
    "dotnet": "@dotnet_runtime",
    "java": "@distroless_java",
    "node": "@distroless_nodejs",
    "python": "@distroless_python",
    "static": "@distroless_static",
}

IMAGE_RUNTIMES = {
    "dotnet": {
        "linux/amd64": "@dotnet_x86_64-unknown-linux-gnu//:runtime",
        "linux/arm64": "@dotnet_arm64-unknown-linux-gnu//:runtime",
    },
    "java": {
        "linux/amd64": "@remotejdk21_linux//:jdk",
        "linux/arm64": "@remotejdk21_linux_aarch64//:jdk",
    },
    "node": {
        "linux/amd64": "@nodejs_linux_amd64//:node_bin",
        "linux/arm64": "@nodejs_linux_arm64//:node_bin",
    },
    "python": {
        "linux/amd64": "@python_3_12_x86_64_unknown_linux_gnu//:files",
        "linux/arm64": "@python_3_12_aarch64_unknown_linux_gnu//:files",
    },
}

IMAGE_PLATFORM_LABELS = {
    "linux/amd64": "@rules_dx//deploy/rules/platforms:linux_amd64",
    "linux/arm64": "@rules_dx//deploy/rules/platforms:linux_arm64",
}

IMAGE_LABEL_PLATFORMS = {str(Label(label)): platform for platform, label in IMAGE_PLATFORM_LABELS.items()}

def image_options_error(binary, base, platforms, main = None, main_class = None, runtime = False):
    """Validates the oci_image_from_binary arguments."""
    if type(binary) != "string" or binary == "":
        return "oci_image_from_binary: invalid binary '" + str(binary) + "': want a binary target label"
    if base not in IMAGE_BASES:
        return ("oci_image_from_binary: invalid base '" + str(base) +
                "': want one of " + ", ".join(sorted(IMAGE_BASES)))
    if type(platforms) != "list" or len(platforms) == 0:
        return "oci_image_from_binary: want at least one platform, one of " + ", ".join(IMAGE_PLATFORM_LABELS.values())
    for platform in platforms:
        if platform not in IMAGE_LABEL_PLATFORMS:
            return ("oci_image_from_binary: invalid platform '" + str(platform) +
                    "': want one of " + ", ".join(IMAGE_PLATFORM_LABELS.values()))
    if len(depset(platforms).to_list()) != len(platforms):
        return "oci_image_from_binary: want each platform once, got " + ", ".join(platforms)
    if runtime and base not in IMAGE_RUNTIMES:
        return "oci_image_from_binary: runtime only applies to the python, java, node and dotnet bases"
    if base in ("python", "node", "dotnet") and not main:
        return "oci_image_from_binary: base '" + base + "' needs main, the workspace path of the entry script"
    if base not in ("python", "java", "node", "dotnet") and (main or main_class):
        return "oci_image_from_binary: main and main_class only apply to the python, java, node and dotnet bases"
    return ""
