"""Android SDK toolchain target for exec hosts without a pinned Android SDK."""

def _unavailable_android_sdk_impl(_ctx):
    return [platform_common.ToolchainInfo(android_sdk_info = None)]

unavailable_android_sdk = rule(
    implementation = _unavailable_android_sdk_impl,
    doc = "Resolves rules_android's SDK toolchain type to a ToolchainInfo without android_sdk_info.",
)
