"""Checks for iOS application, test and XCFramework declarations."""

load("//apple/private/bundle:validate.bzl", "bundle_id_errors", "bundle_version_errors")
load("//apple/private/provisioning:validate.bzl", "provisioning_version_below")
load(":validate.bzl", "IOS_PLATFORMS", "ios_arch_errors", "ios_deployment_errors", "ios_environment_errors", "ios_xcode_errors")

def ios_test_coverage_kwargs(env):
    """Returns the test attributes that export coverage through the pinned LLVM."""
    return {
        "collect_code_coverage": False,
        "env": dict(env) | {"COLLECT_PROFDATA": "1"},
    }

def ios_app_resource_errors(resources):
    """Returns one error string per empty app resource name."""
    errors = []
    for resource in resources:
        if resource == "":
            errors.append("apple_app: resource names must not be empty")
    return errors

def ios_entitlement_errors(entitlements):
    """Returns one error string when the entitlements path leaves its contract."""
    if entitlements == "":
        return []
    if entitlements.endswith(".entitlements"):
        return []
    return ["apple_app: entitlements '" + entitlements + "' must be an .entitlements file"]

def ios_signing_errors(environment, provisioning_profile):
    """Returns one error string per rejected app signing selection."""
    if environment == "device":
        if provisioning_profile == "":
            return ["apple_app: device builds need provisioning_profile, a .mobileprovision file"]
        if not provisioning_profile.endswith(".mobileprovision"):
            return ["apple_app: provisioning_profile '" + provisioning_profile + "' must be a .mobileprovision file"]
        return []
    if provisioning_profile != "":
        return ["apple_app: simulator builds sign ad hoc; leave provisioning_profile empty"]
    return []

def ios_entry_errors(srcs, entry):
    """Returns one error string unless exactly one of srcs or entry names the app entry point."""
    if len(srcs) == 0 and entry == "":
        return ["apple_app: srcs needs the native entry point that calls UIApplicationMain, or entry names a Swift @main module"]
    if len(srcs) > 0 and entry != "":
        return ["apple_app: set srcs or entry for the app entry point, not both"]
    return []

def ios_test_source_errors(srcs, deps):
    """Returns one error string when a test has neither native sources nor test modules."""
    if len(srcs) == 0 and len(deps) == 0:
        return ["apple_app_test: srcs or deps needs the XCTest cases"]
    return []

def ios_default_app(environment):
    """Returns the default simulator application composition for one environment."""
    return struct(
        arch = "arm64",
        bundle_id = "",
        deployment = "",
        entitlements = "",
        entry = "",
        environment = environment,
        frameworks = [],
        library = "",
        provisioning_profile = "",
        resources = [],
        srcs = [],
        version = "",
        xcode_version = "",
    )

def ios_app_errors(config):
    """Returns one error string per rejected field of one app composition."""
    errors = list(ios_environment_errors(config.environment))
    errors.extend(ios_arch_errors(config.environment, config.arch))
    if config.deployment == "":
        errors.append("apple_app: deployment names the minimum iOS version")
    errors.extend(ios_deployment_errors(config.environment, config.deployment))
    errors.extend(ios_xcode_errors(config.xcode_version))
    errors.extend(bundle_id_errors("apple_app", config.bundle_id))
    errors.extend(bundle_version_errors("apple_app", config.version))
    errors.extend(ios_app_resource_errors(config.resources))
    errors.extend(ios_entitlement_errors(config.entitlements))
    errors.extend(ios_signing_errors(config.environment, config.provisioning_profile))
    errors.extend(ios_entry_errors(config.srcs, config.entry))
    if config.library == "" and len(config.frameworks) == 0 and config.entry == "":
        errors.append("apple_app: app composes one qualified Rust library cell in library or XCFramework imports in frameworks, or a Swift @main module in entry")
    return errors

def ios_test_errors(device_type, os_version, deployment):
    """Returns one error string per rejected simulator test selection."""
    errors = []
    if device_type == "":
        errors.append("apple_app_test: device_type names the simulator device, like 'iPhone 16'")
    if os_version == "":
        errors.append("apple_app_test: os_version names the installed simulator runtime, like '18.5'")
    else:
        for part in os_version.split("."):
            if part == "" or not part.isdigit():
                errors.append("apple_app_test: os_version '" + os_version + "' is not a dotted version")
                break
    if deployment == "":
        errors.append("apple_app_test: deployment names the minimum iOS version of the host app")
    else:
        errors.extend(ios_deployment_errors("simulator", deployment))
        if len(errors) == 0 and provisioning_version_below(os_version, deployment):
            errors.append("apple_app_test: simulator runtime iOS " + os_version + " is below the app deployment " + deployment)
    return errors

def ios_slice_errors(slices):
    """Returns one error string per rejected framework platform selection."""
    if len(slices) == 0:
        return ["apple_framework: platforms needs at least one iOS platform label"]
    errors = []
    seen = {}
    has_device = False
    has_simulator = False
    want = ", ".join([str(IOS_PLATFORMS[key]) for key in sorted(IOS_PLATFORMS.keys())])
    for slice in slices:
        if slice not in IOS_PLATFORMS:
            errors.append("apple_framework: platform '" + slice + "' is not an iOS platform: want " + want)
            continue
        label = str(IOS_PLATFORMS[slice])
        if slice in seen:
            errors.append("apple_framework: duplicate platform '" + label + "'")
            continue
        seen[slice] = True
        if slice.startswith("device/"):
            has_device = True
        else:
            has_simulator = True
    if not has_device:
        errors.append("apple_framework: framework needs the device platform " + str(IOS_PLATFORMS["device/arm64"]))
    if not has_simulator:
        errors.append("apple_framework: framework needs at least one simulator platform")
    return errors

def ios_xcframework_resource_errors(resources):
    """Returns one error string per empty or absolute framework resource path."""
    errors = []
    for resource in resources:
        if resource == "":
            errors.append("apple_framework: resource names must not be empty")
        elif resource.startswith("/"):
            errors.append("apple_framework: resource '" + resource + "' must be a relative workspace path")
    return errors

def ios_xcframework_header_errors(hdrs):
    """Returns one error string when no public header is given."""
    if len(hdrs) == 0:
        return ["apple_framework: hdrs needs at least one public header for the framework module map"]
    return []

def ios_xcframework_header_name_errors(names, module):
    """Returns one error string per non-header or umbrella-clashing public header file."""
    errors = []
    umbrella = module + ".h"
    for name in names:
        if not name.endswith(".h"):
            errors.append("apple_framework: public header '" + name + "' must be a .h file")
        elif name.lower() == umbrella.lower():
            errors.append("apple_framework: public header '" + name + "' clashes with the umbrella header " + umbrella)
    return errors

def ios_default_xcframework():
    """Returns the default embeddable framework composition."""
    return struct(
        bundle_id = "",
        bundle_name = "",
        deployment = "",
        hdrs = [],
        library = "",
        resources = [],
        slices = [],
        version = "",
        xcode_version = "",
    )

def ios_xcframework_errors(config):
    """Returns one error string per rejected field of one framework composition."""
    errors = list(bundle_id_errors("apple_framework", config.bundle_id))
    errors.extend(bundle_version_errors("apple_framework", config.version))
    errors.extend(ios_slice_errors(config.slices))
    errors.extend(ios_xcframework_resource_errors(config.resources))
    errors.extend(ios_xcframework_header_errors(config.hdrs))
    if config.deployment == "":
        errors.append("apple_framework: deployment names the minimum iOS version")
    errors.extend(ios_deployment_errors("device", config.deployment))
    errors.extend(ios_xcode_errors(config.xcode_version))
    if config.library == "":
        errors.append("apple_framework: framework composes one Rust static library in library")
    return errors
