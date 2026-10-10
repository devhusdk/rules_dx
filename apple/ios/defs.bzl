"""Configurable Rust iOS device and simulator library selections."""

load(
    "//apple/provisioning:defs.bzl",
    _XCODE_LINEAGE = "XCODE_LINEAGE",
    _deployment_floor = "deployment_floor",
)

DEVICE_ENV = "device"

SIMULATOR_ENV = "simulator"

_DEVICE_ARCH = "arm64"

_SIMULATOR_ARCHES = ["arm64", "x86_64"]

_ENVIRONMENTS = {
    "device": struct(
        arches = [_DEVICE_ARCH],
        sdk = "iphoneos",
        triples = {"arm64": "aarch64-apple-ios"},
    ),
    "simulator": struct(
        arches = _SIMULATOR_ARCHES,
        sdk = "iphonesimulator",
        triples = {
            "arm64": "aarch64-apple-ios-sim",
            "x86_64": "x86_64-apple-ios-sim",
        },
    ),
}

_SDK_CEILINGS = {
    "iphoneos": "18.0",
    "iphonesimulator": "18.0",
}

_DIGITS = "0123456789"

def environments():
    """Returns the supported iOS library environment names."""
    return sorted(_ENVIRONMENTS.keys())

def arches(environment):
    """Returns the supported architectures for one environment."""
    return list(_ENVIRONMENTS[environment].arches) if environment in _ENVIRONMENTS else []

def sdk_for(environment):
    """Returns the SDK identity for one environment, or empty."""
    return _ENVIRONMENTS[environment].sdk if environment in _ENVIRONMENTS else ""

def triple_for(environment, arch):
    """Returns the Rust target triple for one environment and arch, or empty."""
    if environment not in _ENVIRONMENTS:
        return ""
    triples = _ENVIRONMENTS[environment].triples
    return triples[arch] if arch in triples else ""

def ceiling_for(sdk):
    """Returns the Xcode 16 deployment ceiling for one SDK, or empty."""
    return _SDK_CEILINGS[sdk] if sdk in _SDK_CEILINGS else ""

def environment_errors(environment):
    """Returns one error string when the library environment leaves the set."""
    if environment == "":
        return ["apple_ios: select one environment: want " + ", ".join(environments())]
    if environment not in _ENVIRONMENTS:
        return ["apple_ios: unknown environment '" + environment + "': want " + ", ".join(environments())]
    return []

def arch_errors(environment, arch):
    """Returns one error string per rejected environment and arch pairing."""
    if environment not in _ENVIRONMENTS:
        return []
    if arch == "":
        return ["apple_ios: select one arch for '" + environment + "': want " + ", ".join(arches(environment))]
    if arch in _ENVIRONMENTS[environment].arches:
        return []
    if environment == DEVICE_ENV:
        return ["apple_ios: device builds need arch 'arm64'; '" + arch + "' runs on the simulator"]
    return ["apple_ios: arch '" + arch + "' is outside " + environment + ": want " + ", ".join(arches(environment))]

def triple_errors(environment, arch, triple):
    """Returns one error string when the Rust triple disagrees with the cell."""
    if triple == "":
        return []
    if environment not in _ENVIRONMENTS or arch not in _ENVIRONMENTS[environment].arches:
        return []
    want = triple_for(environment, arch)
    if triple == want:
        return []
    return ["apple_ios: triple '" + triple + "' disagrees with " + environment + "/" + arch + ": want '" + want + "'"]

def sdk_errors(environment, sdk):
    """Returns one error string when the SDK disagrees with the environment."""
    if sdk == "":
        return []
    if environment not in _ENVIRONMENTS:
        return []
    want = sdk_for(environment)
    if sdk == want:
        return []
    return ["apple_ios: sdk '" + sdk + "' disagrees with '" + environment + "': want '" + want + "'"]

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

def _above(version, ceiling):
    left = _parts(version)
    right = _parts(ceiling)
    if len(left) == 0 or len(right) == 0:
        return False
    width = max(len(left), len(right))
    for index in range(width):
        first = left[index] if index < len(left) else 0
        second = right[index] if index < len(right) else 0
        if first != second:
            return first > second
    return False

def _below(version, minimum):
    left = _parts(version)
    right = _parts(minimum)
    if len(left) == 0 or len(right) == 0:
        return False
    width = max(len(left), len(right))
    for index in range(width):
        first = left[index] if index < len(left) else 0
        second = right[index] if index < len(right) else 0
        if first != second:
            return first < second
    return False

def xcode_errors(version):
    """Returns one error string when the Xcode version leaves the lineage."""
    if version == "":
        return []
    if len(_parts(version)) == 0:
        return ["apple_ios: xcode_version '" + version + "' is not a dotted version"]
    if not version.startswith(_XCODE_LINEAGE):
        return ["apple_ios: xcode '" + version + "' is outside " + _XCODE_LINEAGE.rstrip(".")]
    return []

def deployment_errors(environment, deployment):
    """Returns one error string per rejected deployment target selection."""
    if deployment == "":
        return []
    if len(_parts(deployment)) == 0:
        return ["apple_ios: deployment_target '" + deployment + "' is not a dotted version"]
    sdk = sdk_for(environment)
    floor = _deployment_floor(sdk) if sdk != "" else ""
    errors = []
    if floor != "" and _below(deployment, floor):
        errors.append("apple_ios: deployment_target '" + deployment + "' is below " + floor)
    ceiling = ceiling_for(sdk)
    if ceiling != "" and _above(deployment, ceiling):
        errors.append("apple_ios: deployment_target '" + deployment + "' is above the Xcode 16 '" + sdk + "' ceiling " + ceiling)
    return errors

def framework_errors(frameworks):
    """Returns one error string per empty linked framework name."""
    errors = []
    for framework in frameworks:
        if framework == "":
            errors.append("apple_ios: framework names must not be empty")
    return errors

def execution_errors(environment):
    """Returns the pending-execution diagnostic for one environment."""
    return ["apple_ios: " + environment + " execution stays pending: run on a qualified macOS executor with Xcode; no simulator or device launch is qualified from this host"]

def default_config(environment):
    """Returns the default library configuration for one environment."""
    return struct(
        arch = "",
        deployment = "",
        environment = environment,
        crate_features = [],
        frameworks = [],
        rustc_flags = [],
        sdk = "",
        triple = "",
        xcode_version = "",
    )

def config_errors(config):
    """Returns one error string per rejected field of one library configuration."""
    errors = list(environment_errors(config.environment))
    errors.extend(arch_errors(config.environment, config.arch))
    errors.extend(triple_errors(config.environment, config.arch, config.triple))
    errors.extend(sdk_errors(config.environment, config.sdk))
    errors.extend(deployment_errors(config.environment, config.deployment))
    errors.extend(xcode_errors(config.xcode_version))
    errors.extend(framework_errors(config.frameworks))
    return errors

def _ios_lib_selection_impl(ctx):
    failures = config_errors(struct(
        arch = ctx.attr.arch,
        deployment = ctx.attr.deployment,
        environment = ctx.attr.environment,
        crate_features = list(ctx.attr.crate_features),
        frameworks = list(ctx.attr.frameworks),
        rustc_flags = list(ctx.attr.rustc_flags),
        sdk = ctx.attr.sdk,
        triple = ctx.attr.triple,
        xcode_version = ctx.attr.xcode_version,
    ))
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_ios_lib_selection = rule(
    implementation = _ios_lib_selection_impl,
    attrs = {
        "arch": attr.string(mandatory = True),
        "deployment": attr.string(mandatory = True),
        "environment": attr.string(mandatory = True),
        "crate_features": attr.string_list(mandatory = True),
        "frameworks": attr.string_list(mandatory = True),
        "rustc_flags": attr.string_list(mandatory = True),
        "sdk": attr.string(mandatory = True),
        "triple": attr.string(mandatory = True),
        "xcode_version": attr.string(mandatory = True),
    },
)

def apple_ios_lib(
        name,
        environment = "",
        arch = "",
        triple = "",
        sdk = "",
        deployment = "",
        xcode_version = "",
        crate_features = [],
        rustc_flags = [],
        frameworks = []):
    """Validates one Rust iOS library selection; nothing builds or launches."""
    _ios_lib_selection(
        name = name + "_selection",
        arch = arch,
        crate_features = crate_features,
        deployment = deployment,
        environment = environment,
        frameworks = frameworks,
        rustc_flags = rustc_flags,
        sdk = sdk,
        triple = triple,
        xcode_version = xcode_version,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
