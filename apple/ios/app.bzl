"""Simulator application and embeddable framework composition selections."""

load(
    ":defs.bzl",
    "arch_errors",
    "arches",
    "deployment_errors",
    "environment_errors",
    "environments",
    "xcode_errors",
)

_ID_CHARS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_"

_DIGITS = "0123456789"

def bundle_id_errors(kind, bundle_id):
    """Returns one error string when the bundle identifier leaves reverse-DNS."""
    if bundle_id == "":
        return [kind + ": bundle_id must not be empty"]
    parts = bundle_id.split(".")
    if len(parts) < 2:
        return [kind + ": bundle_id '" + bundle_id + "' is not reverse-DNS: want like 'com.example.app'"]
    for part in parts:
        if part == "":
            return [kind + ": bundle_id '" + bundle_id + "' is not reverse-DNS: want like 'com.example.app'"]
        for char in part.elems():
            if char not in _ID_CHARS:
                return [kind + ": bundle_id '" + bundle_id + "' uses '" + char + "': want letters, digits, '-', '_', '.'"]
    return []

def bundle_version_errors(kind, version):
    """Returns one error string when the bundle version leaves dotted digits."""
    if version == "":
        return [kind + ": bundle version must not be empty"]
    for part in version.split("."):
        if part == "":
            return [kind + ": bundle version '" + version + "' is not a dotted version"]
        for char in part.elems():
            if char not in _DIGITS:
                return [kind + ": bundle version '" + version + "' is not a dotted version"]
    return []

def app_resource_errors(resources):
    """Returns one error string per empty app resource name."""
    errors = []
    for resource in resources:
        if resource == "":
            errors.append("apple_app: resource names must not be empty")
    return errors

def entitlement_errors(entitlements):
    """Returns one error string when the entitlements path leaves its contract."""
    if entitlements == "":
        return []
    if entitlements.endswith(".entitlements"):
        return []
    return ["apple_app: entitlements '" + entitlements + "' must be an .entitlements file"]

def signing_errors(environment, provisioning_profile, signing_identity):
    """Returns one error string per rejected app signing selection."""
    if environment == "device":
        errors = []
        if provisioning_profile == "":
            errors.append("apple_app: device distribution needs provisioning_profile; credentials travel by file only")
        if signing_identity == "":
            errors.append("apple_app: device distribution needs signing_identity")
        return errors
    if provisioning_profile != "" or signing_identity != "":
        return ["apple_app: simulator runs unsigned; leave provisioning_profile and signing_identity empty"]
    return []

def launch_errors(environment):
    """Returns the pending-launch diagnostic for one app environment."""
    return ["apple_app: " + environment + " launch stays pending: run on a qualified macOS executor with Xcode; no simulator or device launch is qualified from this host"]

def default_app(environment):
    """Returns the default simulator application composition for one environment."""
    return struct(
        arch = "arm64",
        bundle_id = "",
        deployment = "",
        entitlements = "",
        environment = environment,
        export_ipa = False,
        library = "",
        provisioning_profile = "",
        resources = [],
        signing_identity = "",
        version = "",
        xcode_version = "",
    )

def app_errors(config):
    """Returns one error string per rejected field of one app composition."""
    errors = list(environment_errors(config.environment))
    errors.extend(arch_errors(config.environment, config.arch))
    errors.extend(deployment_errors(config.environment, config.deployment))
    errors.extend(xcode_errors(config.xcode_version))
    errors.extend(bundle_id_errors("apple_app", config.bundle_id))
    errors.extend(bundle_version_errors("apple_app", config.version))
    errors.extend(app_resource_errors(config.resources))
    errors.extend(entitlement_errors(config.entitlements))
    errors.extend(signing_errors(config.environment, config.provisioning_profile, config.signing_identity))
    if config.library == "":
        errors.append("apple_app: app composes one qualified Rust library cell in library")
    if config.export_ipa:
        errors.append("apple_app: export is a distinct explicitly applied operation; Check never exports an IPA")
    return errors

def slice_errors(slices):
    """Returns one error string per rejected framework slice selection."""
    if len(slices) == 0:
        return ["apple_framework: framework needs at least one 'environment/arch' slice"]
    errors = []
    seen = {}
    has_device = False
    has_simulator = False
    for slice in slices:
        if slice in seen:
            errors.append("apple_framework: duplicate slice '" + slice + "'")
            continue
        seen[slice] = True
        parts = slice.split("/")
        if len(parts) != 2 or parts[0] == "" or parts[1] == "":
            errors.append("apple_framework: slice '" + slice + "' is not 'environment/arch'")
            continue
        environment = parts[0]
        arch = parts[1]
        if environment not in environments():
            errors.append("apple_framework: unknown environment '" + environment + "' in slice '" + slice + "': want " + ", ".join(environments()))
            continue
        if arch not in arches(environment):
            errors.append("apple_framework: arch '" + arch + "' is outside " + environment + " in slice '" + slice + "': want " + ", ".join(arches(environment)))
            continue
        if environment == "device":
            has_device = True
        else:
            has_simulator = True
    if not has_device:
        errors.append("apple_framework: framework needs the device/arm64 slice")
    if not has_simulator:
        errors.append("apple_framework: framework needs at least one simulator slice")
    return errors

def framework_resource_errors(resources):
    """Returns one error string per empty or absolute framework resource path."""
    errors = []
    for resource in resources:
        if resource == "":
            errors.append("apple_framework: resource names must not be empty")
        elif resource.startswith("/"):
            errors.append("apple_framework: resource '" + resource + "' must be a relative workspace path")
    return errors

def default_framework():
    """Returns the default embeddable framework composition."""
    return struct(
        bundle_id = "",
        resources = [],
        slices = [],
        version = "",
    )

def framework_errors(config):
    """Returns one error string per rejected field of one framework composition."""
    errors = list(bundle_id_errors("apple_framework", config.bundle_id))
    errors.extend(bundle_version_errors("apple_framework", config.version))
    errors.extend(slice_errors(config.slices))
    errors.extend(framework_resource_errors(config.resources))
    return errors

def _app_selection_impl(ctx):
    failures = app_errors(struct(
        arch = ctx.attr.arch,
        bundle_id = ctx.attr.bundle_id,
        deployment = ctx.attr.deployment,
        entitlements = ctx.attr.entitlements,
        environment = ctx.attr.environment,
        export_ipa = ctx.attr.export_ipa,
        library = ctx.attr.library,
        provisioning_profile = ctx.attr.provisioning_profile,
        resources = list(ctx.attr.resources),
        signing_identity = ctx.attr.signing_identity,
        version = ctx.attr.version,
        xcode_version = ctx.attr.xcode_version,
    ))
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_app_selection = rule(
    implementation = _app_selection_impl,
    attrs = {
        "arch": attr.string(mandatory = True),
        "bundle_id": attr.string(mandatory = True),
        "deployment": attr.string(mandatory = True),
        "entitlements": attr.string(mandatory = True),
        "environment": attr.string(mandatory = True),
        "export_ipa": attr.bool(mandatory = True),
        "library": attr.string(mandatory = True),
        "provisioning_profile": attr.string(mandatory = True),
        "resources": attr.string_list(mandatory = True),
        "signing_identity": attr.string(mandatory = True),
        "version": attr.string(mandatory = True),
        "xcode_version": attr.string(mandatory = True),
    },
)

def apple_ios_app(
        name,
        environment = "",
        arch = "arm64",
        bundle_id = "",
        version = "",
        deployment = "",
        xcode_version = "",
        library = "",
        resources = [],
        entitlements = "",
        provisioning_profile = "",
        signing_identity = "",
        export_ipa = False):
    """Validates one simulator application composition; nothing builds or launches."""
    _app_selection(
        name = name + "_selection",
        arch = arch,
        bundle_id = bundle_id,
        deployment = deployment,
        entitlements = entitlements,
        environment = environment,
        export_ipa = export_ipa,
        library = library,
        provisioning_profile = provisioning_profile,
        resources = resources,
        signing_identity = signing_identity,
        version = version,
        xcode_version = xcode_version,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )

def _framework_selection_impl(ctx):
    failures = framework_errors(struct(
        bundle_id = ctx.attr.bundle_id,
        resources = list(ctx.attr.resources),
        slices = list(ctx.attr.slices),
        version = ctx.attr.version,
    ))
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_framework_selection = rule(
    implementation = _framework_selection_impl,
    attrs = {
        "bundle_id": attr.string(mandatory = True),
        "resources": attr.string_list(mandatory = True),
        "slices": attr.string_list(mandatory = True),
        "version": attr.string(mandatory = True),
    },
)

def apple_ios_framework(
        name,
        bundle_id = "",
        version = "",
        slices = [],
        resources = []):
    """Validates one embeddable framework composition; nothing builds or signs."""
    _framework_selection(
        name = name + "_selection",
        bundle_id = bundle_id,
        resources = resources,
        slices = slices,
        version = version,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
