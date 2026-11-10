"""Canonical host, execution, and target platform identities."""

load(":hub.bzl", "TOOL_PLATFORMS")

_OSES = ["linux", "macos", "windows"]

_CPUS = ["x86_64", "arm64"]

def normalize_os(raw):
    """Returns the OS token if canonical, or empty when unknown."""
    return raw if raw in _OSES else ""

def normalize_cpu(raw):
    """Returns the CPU token if canonical, or empty when unknown."""
    return raw if raw in _CPUS else ""

def platform_key(os_raw, cpu_raw):
    """Returns the canonical platform key for raw OS/CPU tokens, or empty."""
    os = normalize_os(os_raw)
    cpu = normalize_cpu(cpu_raw)
    if os == "" or cpu == "":
        return ""
    return os + "_" + cpu

def is_supported_key(key):
    """Reports whether a canonical key is a supported execution platform."""
    return key in TOOL_PLATFORMS

def _key_error(key, raw, role):
    """Returns the explicit diagnostic for one unknown platform key, or empty."""
    if key != "":
        return ""
    return role + " platform: unknown os/cpu '" + raw + "'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)"

def platform_key_error(os_raw, cpu_raw, role):
    """Returns the explicit diagnostic for unknown platform tokens, or empty."""
    return _key_error(platform_key(os_raw, cpu_raw), str(os_raw) + "/" + str(cpu_raw), role)

def _identity(role, os_raw, cpu_raw):
    """Builds one role-named platform identity for raw OS/CPU tokens."""
    return struct(
        cpu = normalize_cpu(cpu_raw),
        key = platform_key(os_raw, cpu_raw),
        os = normalize_os(os_raw),
        raw = str(os_raw) + "/" + str(cpu_raw),
        role = role,
    )

def host_identity(os_raw, cpu_raw):
    """Builds the CLI host identity for raw OS/CPU tokens."""
    return _identity("host", os_raw, cpu_raw)

def execution_identity(os_raw, cpu_raw):
    """Builds the tool execution identity for raw OS/CPU tokens."""
    return _identity("execution", os_raw, cpu_raw)

def target_identity(os_raw, cpu_raw):
    """Builds the user target identity for raw OS/CPU tokens."""
    return _identity("target", os_raw, cpu_raw)

def identity_role(ident):
    """Returns the role naming one platform identity."""
    return ident.role

def identity_key(ident):
    """Returns the canonical key of one platform identity."""
    return ident.key

def identity_error(ident):
    """Returns the explicit diagnostic for an unknown platform identity, or empty."""
    return _key_error(ident.key, ident.raw, ident.role)

def target_is_windows(ctx, windows_os):
    """Reports whether the target platform is Windows."""
    return ctx.target_platform_has_constraint(windows_os)

def execution_is_windows(ctx, windows_os):
    """Reports whether the execution platform is Windows from execution configuration."""
    return ctx.target_platform_has_constraint(windows_os)

def artifact_platform_key(artifact):
    """Returns the canonical platform key for one artifact metadata record."""
    key = platform_key(artifact["os"], artifact["cpu"])
    if key == "":
        return str(artifact["os"]) + "_" + str(artifact["cpu"])
    return key

def platform_repeat_message(first, second):
    """Returns the error for a second dx_tools.platform tag, naming both modules."""
    return "dx_tools.platform may be declared at most once; declared by %s, then again by %s" % (first, second)
