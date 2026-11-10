LOCAL_ROUTE = "local"

MANAGED_ROUTE = "managed"

XCODE_LINEAGE = "16."

MINIMUM_MACOS = "14.5"

QUALIFIED_HOST = "macos_arm64"

REQUIRED_LICENSES = ["apple-xcode-license"]

SDK_FLOORS = {
    "iphoneos": "17.0",
    "iphonesimulator": "17.0",
    "macosx": "14.0",
}

HOST_CONSTRAINTS = {
    "macos_arm64": ["@platforms//os:macos", "@platforms//cpu:arm64"],
}

_DIGITS = "0123456789"

def provisioning_routes():
    """Returns the supported provisioning route names."""
    return sorted([LOCAL_ROUTE, MANAGED_ROUTE])

def provisioning_hosts():
    """Returns the qualified execution host names."""
    return sorted(HOST_CONSTRAINTS.keys())

def provisioning_sdk_names():
    """Returns the qualified SDK identity names."""
    return sorted(SDK_FLOORS.keys())

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

def provisioning_version_below(version, minimum):
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

def provisioning_version_errors(field, version, minimum):
    """Returns one error string per malformed or downgraded dotted version."""
    if version == "":
        return []
    if len(_numbers(version)) == 0:
        return ["apple: " + field + " '" + version + "' is not a dotted version"]
    if provisioning_version_below(version, minimum):
        return ["apple: " + field + " '" + version + "' is below " + minimum]
    return []

def provisioning_route_errors(route):
    """Returns one error string when the execution route leaves the set."""
    if route == "":
        return ["apple: select one execution route: want " + ", ".join(provisioning_routes())]
    if route != LOCAL_ROUTE and route != MANAGED_ROUTE:
        return ["apple: unknown route '" + route + "': want " + ", ".join(provisioning_routes())]
    return []

def provisioning_host_errors(host):
    """Returns one error string when the execution host leaves the set."""
    if host in HOST_CONSTRAINTS:
        return []
    return ["apple: unknown host '" + host + "': want " + ", ".join(provisioning_hosts())]

def provisioning_xcode_errors(version):
    """Returns one error string when the Xcode version leaves the lineage."""
    if version == "":
        return []
    if len(_numbers(version)) == 0:
        return ["apple: xcode_version '" + version + "' is not a dotted version"]
    if not version.startswith(XCODE_LINEAGE):
        return ["apple: xcode '" + version + "' is outside " + XCODE_LINEAGE.rstrip(".")]
    return []

def provisioning_sdk_errors(sdk, deployment):
    """Returns one error string per rejected SDK or deployment selection."""
    if sdk == "":
        return ["apple: select one SDK: want " + ", ".join(provisioning_sdk_names())]
    if sdk not in SDK_FLOORS:
        return ["apple: sdk '" + sdk + "' is outside " + ", ".join(provisioning_sdk_names())]
    return provisioning_version_errors("deployment_target", deployment, SDK_FLOORS[sdk]) if deployment != "" else []

def provisioning_license_errors(accept_licenses, target = ""):
    """Returns one error string when the Xcode license stays unaccepted."""
    missing = [name for name in REQUIRED_LICENSES if name not in accept_licenses]
    if len(missing) > 0:
        message = "apple: accept " + ", ".join(missing) + " before provisioning; add them to the accept_licenses attribute"
        if target != "":
            message += " of " + target
        return [message]
    return []

def provisioning_default_selection(route):
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

def provisioning_errors(selection, target = ""):
    """Returns one error string per rejected provisioning selection field."""
    errors = list(provisioning_route_errors(selection.route))
    errors.extend(provisioning_host_errors(selection.host))
    errors.extend(provisioning_license_errors(selection.accept_licenses, target))
    errors.extend(provisioning_sdk_errors(selection.sdk, selection.deployment))
    errors.extend(provisioning_version_errors("macos_version", selection.macos_version, MINIMUM_MACOS))
    if selection.api_token != "":
        errors.append("apple: api_token must stay empty; pass credential_file instead")
    if selection.allow_download:
        errors.append("apple: Xcode has no unattended download; use the local route or a managed endpoint")
    if selection.route == LOCAL_ROUTE:
        if selection.host != "" and selection.host != QUALIFIED_HOST:
            errors.append("apple: local route needs a macos_arm64 executor; Xcode and the simulator do not run on '" + selection.host + "'")
        if selection.xcode_version == "":
            errors.append("apple: local route needs the installed Xcode version in xcode_version")
        errors.extend(provisioning_xcode_errors(selection.xcode_version))
    if selection.route == MANAGED_ROUTE:
        if selection.endpoint == "":
            errors.append("apple: managed route needs an endpoint naming the macOS executor")
        if selection.credential_file == "":
            errors.append("apple: managed route needs credential_file; credentials never travel inline")
        errors.extend(provisioning_xcode_errors(selection.xcode_version))
    return errors
