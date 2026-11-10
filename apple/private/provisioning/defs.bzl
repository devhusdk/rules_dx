load(":validate.bzl", "HOST_CONSTRAINTS", _LOCAL_ROUTE = "LOCAL_ROUTE", _MANAGED_ROUTE = "MANAGED_ROUTE", _MINIMUM_MACOS = "MINIMUM_MACOS", _QUALIFIED_HOST = "QUALIFIED_HOST", _REQUIRED_LICENSES = "REQUIRED_LICENSES", "SDK_FLOORS", _XCODE_LINEAGE = "XCODE_LINEAGE", "provisioning_errors")
LOCAL_ROUTE = _LOCAL_ROUTE
MANAGED_ROUTE = _MANAGED_ROUTE
MINIMUM_MACOS = _MINIMUM_MACOS
QUALIFIED_HOST = _QUALIFIED_HOST
REQUIRED_LICENSES = _REQUIRED_LICENSES
XCODE_LINEAGE = _XCODE_LINEAGE
"""Declared Xcode selection and managed macOS execution requirements."""

MINIMUM_XCODE = "16.0"

_SDK_CEILINGS = {
    "iphoneos": "18.0",
    "iphonesimulator": "18.0",
    "macosx": "15.0",
}

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

def host_constraints(host):
    """Returns the platform constraint values for one execution host."""
    return list(HOST_CONSTRAINTS[host]) if host in HOST_CONSTRAINTS else []

def deployment_floor(sdk):
    """Returns the minimum deployment target for one SDK, or empty."""
    return SDK_FLOORS[sdk] if sdk in SDK_FLOORS else ""

def ceiling_for(sdk):
    """Returns the Xcode 16 deployment ceiling for one SDK, or empty."""
    return _SDK_CEILINGS[sdk] if sdk in _SDK_CEILINGS else ""

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
    ), ctx.label.name)
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
