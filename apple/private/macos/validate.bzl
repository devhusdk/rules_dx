"""Checks for macOS application declarations."""

load("//apple/private/bundle:validate.bzl", "bundle_id_errors", "bundle_version_errors")
load("//apple/private/provisioning:defs.bzl", "XCODE_LINEAGE", "ceiling_for", "deployment_floor")
load("//apple/private/provisioning:validate.bzl", "provisioning_version_below")

MACOS_ARCHES = ["arm64"]

def macos_default_app():
    """Returns the default macOS application composition."""
    return struct(
        arch = "arm64",
        bundle_id = "",
        deployment = "",
        entitlements = "",
        frameworks = [],
        library = "",
        plugins = [],
        provisioning_profile = "",
        resources = [],
        srcs = [],
        version = "",
        xcode_version = "",
    )

def macos_plugin_errors(plugins):
    """Returns one error string per empty or repeated plugin."""
    errors = []
    seen = []
    for plugin in plugins:
        if plugin == "":
            errors.append("apple_macos_app: plugin names must not be empty")
        elif plugin in seen:
            errors.append("apple_macos_app: plugin '" + plugin + "' is listed twice")
        seen.append(plugin)
    return errors

def _dotted(version):
    parts = version.split(".")
    for part in parts:
        if part == "" or not part.isdigit():
            return False
    return True

def _deployment_errors(deployment):
    if deployment == "":
        return ["apple_macos_app: deployment names the minimum macOS version"]
    if not _dotted(deployment):
        return ["apple_macos_app: deployment '" + deployment + "' is not a dotted version"]
    errors = []
    floor = deployment_floor("macosx")
    if provisioning_version_below(deployment, floor):
        errors.append("apple_macos_app: deployment '" + deployment + "' is below " + floor)
    ceiling = ceiling_for("macosx")
    if provisioning_version_below(ceiling, deployment):
        errors.append("apple_macos_app: deployment '" + deployment + "' is above the Xcode 16 'macosx' ceiling " + ceiling)
    return errors

def macos_app_errors(config):
    """Returns one error string per rejected field of one macOS app composition."""
    errors = []
    if config.arch not in MACOS_ARCHES:
        errors.append("apple_macos_app: arch '" + config.arch + "' is outside macos: want " + ", ".join(MACOS_ARCHES))
    errors.extend(_deployment_errors(config.deployment))
    if config.xcode_version != "":
        if not _dotted(config.xcode_version):
            errors.append("apple_macos_app: xcode_version '" + config.xcode_version + "' is not a dotted version")
        elif not config.xcode_version.startswith(XCODE_LINEAGE):
            errors.append("apple_macos_app: xcode '" + config.xcode_version + "' is outside " + XCODE_LINEAGE.rstrip("."))
    errors.extend(bundle_id_errors("apple_macos_app", config.bundle_id))
    errors.extend(bundle_version_errors("apple_macos_app", config.version))
    errors.extend(macos_plugin_errors(config.plugins))
    if config.provisioning_profile != "" or config.entitlements != "":
        errors.append("apple_macos_app: building signs ad hoc only; signing is a separate operation")
    if len(config.srcs) == 0 and config.library == "":
        errors.append("apple_macos_app: srcs or library is required; pass the native entry point or a Rust static library")
    return errors
