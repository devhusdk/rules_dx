"""Declared Xcode selection and managed macOS execution requirements."""

LOCAL_ROUTE = "local"

MANAGED_ROUTE = "managed"

MINIMUM_XCODE = "16.0"

XCODE_LINEAGE = "16."

MINIMUM_MACOS = "14.5"

QUALIFIED_HOST = "macos_arm64"

REQUIRED_LICENSES = ["apple-xcode-license"]

_SDK_FLOORS = {
    "iphoneos": "17.0",
    "iphonesimulator": "17.0",
    "macosx": "14.0",
}

_HOST_CONSTRAINTS = {
    "macos_arm64": ["@platforms//os:macos", "@platforms//cpu:arm64"],
}

_DIGITS = "0123456789"

AppleToolingInfo = provider(
    doc = "One validated Xcode selection with its execution route.",
    fields = {
        "credential_file": "Path to the managed executor credential; never its contents.",
        "deployment": "Declared minimum OS version, or empty.",
        "endpoint": "Managed executor endpoint, or empty.",
        "exec_constraints": "Platform constraint labels the executor must match.",
        "host": "Execution host name.",
        "macos_version": "Minimum executor macOS version.",
        "route": "Execution route: local or managed.",
        "sdk": "SDK identity name.",
        "xcode_version": "Declared Xcode version, or empty.",
    },
)

def routes():
    """Returns the supported provisioning route names."""
    return sorted([LOCAL_ROUTE, MANAGED_ROUTE])

def hosts():
    """Returns the qualified execution host names."""
    return sorted(_HOST_CONSTRAINTS.keys())

def host_constraints(host):
    """Returns the platform constraint values for one execution host."""
    return list(_HOST_CONSTRAINTS[host]) if host in _HOST_CONSTRAINTS else []

def sdk_names():
    """Returns the qualified SDK identity names."""
    return sorted(_SDK_FLOORS.keys())

def deployment_floor(sdk):
    """Returns the minimum deployment target for one SDK, or empty."""
    return _SDK_FLOORS[sdk] if sdk in _SDK_FLOORS else ""

def _numbers(version):
    numbers = []
    for part in version.split("."):
        if part == "":
            return []
        for char in part.elems():
            if char not in _DIGITS:
                return []
        numbers.append(int(part))
    return numbers

def version_below(version, minimum):
    """Returns whether one dotted version sorts below another."""
    left = _numbers(version)
    right = _numbers(minimum)
    if len(left) == 0 or len(right) == 0:
        return False
    width = max(len(left), len(right))
    for index in range(width):
        first = left[index] if index < len(left) else 0
        second = right[index] if index < len(right) else 0
        if first != second:
            return first < second
    return False

def version_matches(declared, resolved):
    """Returns whether a resolved version carries every declared component."""
    wanted = declared.split(".")
    actual = resolved.split(".")
    for index in range(len(wanted)):
        have = actual[index] if index < len(actual) else "0"
        if have.isdigit() and wanted[index].isdigit():
            if int(have) != int(wanted[index]):
                return False
        elif have != wanted[index]:
            return False
    return True

def version_errors(field, version, minimum):
    """Returns one error string per malformed or downgraded dotted version."""
    if version == "":
        return []
    if len(_numbers(version)) == 0:
        return ["apple: " + field + " '" + version + "' is not a dotted version"]
    if version_below(version, minimum):
        return ["apple: " + field + " '" + version + "' is below " + minimum]
    return []

def route_errors(route):
    """Returns one error string when the execution route leaves the set."""
    if route == "":
        return ["apple: select one execution route: want " + ", ".join(routes())]
    if route != LOCAL_ROUTE and route != MANAGED_ROUTE:
        return ["apple: unknown route '" + route + "': want " + ", ".join(routes())]
    return []

def host_errors(host):
    """Returns one error string when the execution host leaves the set."""
    if host in _HOST_CONSTRAINTS:
        return []
    return ["apple: unknown host '" + host + "': want " + ", ".join(hosts())]

def xcode_errors(version):
    """Returns one error string when the Xcode version leaves the lineage."""
    if version == "":
        return []
    if len(_numbers(version)) == 0:
        return ["apple: xcode_version '" + version + "' is not a dotted version"]
    if not version.startswith(XCODE_LINEAGE):
        return ["apple: xcode '" + version + "' is outside " + XCODE_LINEAGE.rstrip(".")]
    return []

def sdk_errors(sdk, deployment):
    """Returns one error string per rejected SDK or deployment selection."""
    if sdk == "":
        return ["apple: select one SDK: want " + ", ".join(sdk_names())]
    if sdk not in _SDK_FLOORS:
        return ["apple: sdk '" + sdk + "' is outside " + ", ".join(sdk_names())]
    return version_errors("deployment_target", deployment, _SDK_FLOORS[sdk]) if deployment != "" else []

def license_errors(accept_licenses):
    """Returns one error string when the Xcode license stays unaccepted."""
    missing = [name for name in REQUIRED_LICENSES if name not in accept_licenses]
    if len(missing) > 0:
        return ["apple: accept " + ", ".join(missing) + " before provisioning"]
    return []

def default_selection(route):
    """Returns the default provisioning selection for one route."""
    return struct(
        accept_licenses = list(REQUIRED_LICENSES),
        allow_download = False,
        api_token = "",
        credential_file = "",
        deployment = "",
        endpoint = "",
        host = QUALIFIED_HOST if route != "" else "",
        macos_version = "",
        route = route,
        sdk = "",
        xcode_version = "",
    )

def provisioning_errors(selection):
    """Returns one error string per rejected provisioning selection field."""
    errors = list(route_errors(selection.route))
    errors.extend(host_errors(selection.host))
    errors.extend(license_errors(selection.accept_licenses))
    errors.extend(sdk_errors(selection.sdk, selection.deployment))
    errors.extend(version_errors("macos_version", selection.macos_version, MINIMUM_MACOS))
    if selection.api_token != "":
        errors.append("apple: api_token must stay empty; pass credential_file instead")
    if selection.allow_download:
        errors.append("apple: Xcode has no unattended download; use the local route or a managed endpoint")
    if selection.route == LOCAL_ROUTE:
        if selection.host != "" and selection.host != QUALIFIED_HOST:
            errors.append("apple: local route needs a macos_arm64 executor; Xcode and the simulator do not run on '" + selection.host + "'")
        if selection.xcode_version == "":
            errors.append("apple: local route needs the installed Xcode version in xcode_version")
        errors.extend(xcode_errors(selection.xcode_version))
    if selection.route == MANAGED_ROUTE:
        if selection.endpoint == "":
            errors.append("apple: managed route needs an endpoint naming the macOS executor")
        if selection.credential_file == "":
            errors.append("apple: managed route needs credential_file; credentials never travel inline")
        errors.extend(xcode_errors(selection.xcode_version))
    return errors

def _provisioning_selection_impl(ctx):
    failures = provisioning_errors(struct(
        accept_licenses = list(ctx.attr.accept_licenses),
        allow_download = ctx.attr.allow_download,
        api_token = ctx.attr.api_token,
        credential_file = ctx.attr.credential_file,
        deployment = ctx.attr.deployment,
        endpoint = ctx.attr.endpoint,
        host = ctx.attr.host,
        macos_version = ctx.attr.macos_version,
        route = ctx.attr.route,
        sdk = ctx.attr.sdk,
        xcode_version = ctx.attr.xcode_version,
    ))
    if len(failures) > 0:
        fail("; ".join(failures))
    return [AppleToolingInfo(
        credential_file = ctx.attr.credential_file,
        deployment = ctx.attr.deployment,
        endpoint = ctx.attr.endpoint,
        exec_constraints = host_constraints(ctx.attr.host),
        host = ctx.attr.host,
        macos_version = ctx.attr.macos_version or MINIMUM_MACOS,
        route = ctx.attr.route,
        sdk = ctx.attr.sdk,
        xcode_version = ctx.attr.xcode_version,
    )]

_provisioning_selection = rule(
    implementation = _provisioning_selection_impl,
    doc = "Fails analysis when an Xcode provisioning selection is invalid.",
    provides = [AppleToolingInfo],
    attrs = {
        "accept_licenses": attr.string_list(
            doc = "License identifiers the selection accepts, including the Xcode license.",
            mandatory = True,
        ),
        "allow_download": attr.bool(
            doc = "Must stay false, since Xcode has no unattended download.",
            mandatory = True,
        ),
        "api_token": attr.string(
            doc = "API token field, which must stay empty.",
            mandatory = True,
        ),
        "credential_file": attr.string(
            doc = "Credential file path for the managed route, never inline.",
            mandatory = True,
        ),
        "deployment": attr.string(
            doc = "Minimum deployment target version for the selected SDK.",
            mandatory = True,
        ),
        "endpoint": attr.string(
            doc = "Endpoint naming the managed macOS executor.",
            mandatory = True,
        ),
        "host": attr.string(doc = "Execution host the selection runs on.", mandatory = True),
        "macos_version": attr.string(
            doc = "macOS version of the executor, defaulting to the minimum.",
            mandatory = True,
        ),
        "route": attr.string(
            doc = "Provisioning route, either local or managed.",
            mandatory = True,
        ),
        "sdk": attr.string(doc = "Apple SDK name the selection builds against.", mandatory = True),
        "xcode_version": attr.string(
            doc = "Installed Xcode version the selection requires.",
            mandatory = True,
        ),
    },
)

def apple_provisioning(
        name,
        route = "",
        accept_licenses = [],
        allow_download = False,
        api_token = "",
        credential_file = "",
        deployment = "",
        endpoint = "",
        host = "",
        macos_version = "",
        sdk = "",
        xcode_version = "",
        **kwargs):
    """Validates one Apple toolchain provisioning selection; nothing builds."""
    _provisioning_selection(
        name = name,
        accept_licenses = accept_licenses,
        allow_download = allow_download,
        api_token = api_token,
        credential_file = credential_file,
        deployment = deployment,
        endpoint = endpoint,
        host = host,
        macos_version = macos_version,
        route = route,
        sdk = sdk,
        xcode_version = xcode_version,
        **kwargs
    )
