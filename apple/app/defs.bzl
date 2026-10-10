"""Composed iOS app bundle and framework selections."""

load(
    "//apple/ios:defs.bzl",
    _ios_arch_errors = "arch_errors",
    _ios_deployment_errors = "deployment_errors",
    _ios_environment_errors = "environment_errors",
    _ios_environments = "environments",
    _ios_framework_errors = "framework_errors",
    _ios_sdk_errors = "sdk_errors",
    _ios_triple_errors = "triple_errors",
    _ios_xcode_errors = "xcode_errors",
)

BUILD_OP = "build"

LAUNCH_OP = "launch"

SIGN_OP = "sign"

EXPORT_OP = "export"

PUBLISH_OP = "publish"

_DEVICE_ENV = "device"

_EFFECT_OPS = [EXPORT_OP, PUBLISH_OP, SIGN_OP]

_DIGITS = "0123456789"

def operations():
    """Returns the supported app operation names."""
    return sorted([BUILD_OP, EXPORT_OP, LAUNCH_OP, PUBLISH_OP, SIGN_OP])

def effects():
    """Returns the app operations that need explicitly allowed effects."""
    return sorted(_EFFECT_OPS)

def app_file_name(name):
    """Returns the bundle directory name for one app name."""
    return name + ".app"

def framework_file_name(name):
    """Returns the framework directory name for one framework name."""
    return name + ".framework"

def xcframework_file_name(name):
    """Returns the xcframework directory name for one framework name."""
    return name + ".xcframework"

def bundle_id_errors(bundle_id):
    """Returns one error string when the bundle identifier leaves reverse-DNS."""
    if bundle_id == "":
        return ["apple_app: select one bundle identifier: want reverse-DNS 'com.example.app'"]
    parts = bundle_id.split(".")
    if len(parts) < 2:
        return ["apple_app: bundle identifier '" + bundle_id + "' is not reverse-DNS"]
    for part in parts:
        if part == "" or " " in part:
            return ["apple_app: bundle identifier '" + bundle_id + "' is not reverse-DNS"]
    return []

def _parts(version):
    numbers = []
    for part in version.split("."):
        if part == "":
            return []
        for char in part.elems():
            if char not in _DIGITS:
                return []
        numbers.append(int(part))
    return numbers

def app_version_errors(version):
    """Returns one error string when the app version is not dotted."""
    if version == "":
        return []
    if len(_parts(version)) == 0:
        return ["apple_app: version '" + version + "' is not a dotted version"]
    return []

def resource_errors(resources):
    """Returns one error string per empty or escaping bundle resource."""
    errors = []
    for resource in resources:
        if resource == "":
            errors.append("apple_app: resource names must not be empty")
        elif resource.startswith("/"):
            errors.append("apple_app: resource '" + resource + "' escapes the bundle")
        elif ".." in resource.split("/"):
            errors.append("apple_app: resource '" + resource + "' escapes the bundle")
    return errors

def entitlement_errors(entitlements):
    """Returns one error string per empty or duplicated entitlement name."""
    errors = []
    seen = []
    for entitlement in entitlements:
        if entitlement == "":
            errors.append("apple_app: entitlement names must not be empty")
        elif entitlement in seen:
            errors.append("apple_app: entitlement '" + entitlement + "' is listed twice")
        else:
            seen.append(entitlement)
    return errors

def slice_errors(served, slices):
    """Returns one error string per malformed or unserved framework slice."""
    errors = []
    for slice in slices:
        cell = slice.split("/")
        if len(cell) != 2 or cell[0] == "" or cell[1] == "":
            errors.append("apple_app: slice '" + slice + "' is not 'environment/arch'")
            continue
        environment = cell[0]
        if environment not in _ios_environments():
            errors.append("apple_app: unknown environment '" + environment + "' in slice '" + slice + "': want " + ", ".join(_ios_environments()))
            continue
        errors.extend(_ios_arch_errors(environment, cell[1]))
        if environment not in served:
            errors.append("apple_app: slice '" + slice + "' is outside the " + ", ".join(served) + " framework")
    return errors

def operation_errors(operation, provisioning_profile, entitlements, allow_effects):
    """Returns one error string per rejected app operation selection."""
    if operation == "":
        return ["apple_app: select one operation: want " + ", ".join(operations())]
    if operation not in operations():
        return ["apple_app: unknown operation '" + operation + "': want " + ", ".join(operations())]
    if operation in _EFFECT_OPS and not allow_effects:
        return ["apple_app: " + operation + " is an effect: allow effects explicitly; check never signs, exports, or publishes"]
    if operation in _EFFECT_OPS and provisioning_profile == "":
        return ["apple_app: " + operation + " needs a provisioning profile before signing"]
    if operation in _EFFECT_OPS and len(entitlements) == 0:
        return ["apple_app: " + operation + " needs entitlements before signing"]
    return []

def pending_errors(operation, environment):
    """Returns the pending-execution diagnostic for one app operation."""
    if operation == BUILD_OP:
        return []
    if operation == LAUNCH_OP and environment == _DEVICE_ENV:
        return ["apple_app: device execution stays pending: run on a qualified macOS executor with Xcode; the simulator does not stand in for the device"]
    if operation == LAUNCH_OP:
        return ["apple_app: simulator execution stays pending: run on a qualified macOS executor with Xcode; no simulator or device launch is qualified from this host"]
    if operation in _EFFECT_OPS:
        return ["apple_app: " + operation + " stays pending: run on a qualified macOS executor with Xcode; signing and export need supplied credentials"]
    return []

def app_errors(config):
    """Returns one error string per rejected field of one app selection."""
    errors = list(_ios_environment_errors(config.environment))
    errors.extend(_ios_arch_errors(config.environment, config.arch))
    errors.extend(_ios_triple_errors(config.environment, config.arch, config.triple))
    errors.extend(_ios_sdk_errors(config.environment, config.sdk))
    errors.extend(_ios_deployment_errors(config.environment, config.deployment))
    errors.extend(_ios_xcode_errors(config.xcode_version))
    errors.extend(bundle_id_errors(config.bundle_id))
    errors.extend(app_version_errors(config.version))
    errors.extend(_ios_framework_errors(config.frameworks))
    errors.extend(resource_errors(config.resources))
    errors.extend(entitlement_errors(config.entitlements))
    errors.extend(operation_errors(config.operation, config.provisioning_profile, config.entitlements, config.allow_effects))
    return errors

def xcframework_errors(name, served, slices):
    """Returns one error string per rejected framework selection field."""
    errors = []
    if name == "":
        errors.append("apple_app: select one framework name")
    if len(slices) == 0:
        errors.append("apple_app: declare one slice: want 'environment/arch'")
    for environment in served:
        if environment not in _ios_environments():
            errors.append("apple_app: unknown environment '" + environment + "': want " + ", ".join(_ios_environments()))
    errors.extend(slice_errors(served, slices))
    return errors

def default_app(environment):
    """Returns the default app selection for one environment."""
    return struct(
        allow_effects = False,
        arch = "",
        bundle_id = "",
        deployment = "",
        entitlements = [],
        environment = environment,
        frameworks = [],
        operation = BUILD_OP,
        provisioning_profile = "",
        resources = [],
        sdk = "",
        triple = "",
        version = "",
        xcode_version = "",
    )

def default_framework():
    """Returns the default framework selection."""
    return struct(
        name = "",
        served = [],
        slices = [],
    )

def _selection_fields(ctx):
    return struct(
        allow_effects = ctx.attr.allow_effects,
        arch = ctx.attr.arch,
        bundle_id = ctx.attr.bundle_id,
        deployment = ctx.attr.deployment,
        entitlements = list(ctx.attr.entitlements),
        environment = ctx.attr.environment,
        frameworks = list(ctx.attr.frameworks),
        operation = ctx.attr.operation,
        provisioning_profile = ctx.attr.provisioning_profile,
        resources = list(ctx.attr.resources),
        sdk = ctx.attr.sdk,
        triple = ctx.attr.triple,
        version = ctx.attr.version,
        xcode_version = ctx.attr.xcode_version,
    )

def _app_bundle_selection_impl(ctx):
    failures = app_errors(_selection_fields(ctx))
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_app_bundle_selection = rule(
    implementation = _app_bundle_selection_impl,
    attrs = {
        "allow_effects": attr.bool(mandatory = True),
        "arch": attr.string(mandatory = True),
        "bundle_id": attr.string(mandatory = True),
        "deployment": attr.string(mandatory = True),
        "entitlements": attr.string_list(mandatory = True),
        "environment": attr.string(mandatory = True),
        "frameworks": attr.string_list(mandatory = True),
        "operation": attr.string(mandatory = True),
        "provisioning_profile": attr.string(mandatory = True),
        "resources": attr.string_list(mandatory = True),
        "sdk": attr.string(mandatory = True),
        "triple": attr.string(mandatory = True),
        "version": attr.string(mandatory = True),
        "xcode_version": attr.string(mandatory = True),
    },
)

def apple_app_bundle(
        name,
        environment = "",
        arch = "",
        triple = "",
        sdk = "",
        deployment = "",
        xcode_version = "",
        bundle_id = "",
        version = "",
        frameworks = [],
        resources = [],
        entitlements = [],
        operation = "build",
        provisioning_profile = "",
        allow_effects = False):
    """Validates one composed app bundle selection; nothing builds or launches."""
    _app_bundle_selection(
        name = name + "_selection",
        allow_effects = allow_effects,
        arch = arch,
        bundle_id = bundle_id,
        deployment = deployment,
        entitlements = entitlements,
        environment = environment,
        frameworks = frameworks,
        operation = operation,
        provisioning_profile = provisioning_profile,
        resources = resources,
        sdk = sdk,
        triple = triple,
        version = version,
        xcode_version = xcode_version,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )

def _xcframework_selection_impl(ctx):
    failures = xcframework_errors(
        ctx.attr.framework_name,
        list(ctx.attr.served),
        list(ctx.attr.slices),
    )
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_xcframework_selection = rule(
    implementation = _xcframework_selection_impl,
    attrs = {
        "framework_name": attr.string(mandatory = True),
        "served": attr.string_list(mandatory = True),
        "slices": attr.string_list(mandatory = True),
    },
)

def apple_xcframework(name, served = [], slices = []):
    """Validates one framework slice selection; nothing builds or archives."""
    _xcframework_selection(
        name = name + "_selection",
        framework_name = name,
        served = served,
        slices = slices,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
