"""Shared Apple bundle checks for iOS and macOS rules."""

load("//apple/provisioning:defs.bzl", "version_matches")
load("//libs/starlark:resources.bzl", "dx_apple_compile_resources")

_ID_CHARS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_"

_DIGITS = "0123456789"

TOOL_PATH = "/usr/bin:/bin:/usr/sbin:/sbin"

_COMPILED_RESOURCES = ["metal", "mlmodel", "mlpackage", "storyboard", "xcstrings", "xib"]

_GROUPED_RESOURCES = (".alticon", ".atlas", ".bundle", ".icon", ".lproj", ".mlpackage", ".scnassets", ".xcassets", ".xcdatamodel", ".xcdatamodeld", ".xcmappingmodel", ".xcstickers")

_SIGNING_CERTIFICATE = str(Label("@rules_apple//apple/build_settings:signing_certificate_name"))

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

def display(label):
    """Returns a label as users write it in their own repository."""
    if label.repo_name == "":
        return "//{}:{}".format(label.package, label.name)
    return str(label)

def label_text(value):
    """Returns a label or None as a string."""
    return "" if value == None else str(value)

def archives(cc_info):
    """Returns the static archives one CcInfo links."""
    found = []
    for linker_input in cc_info.linking_context.linker_inputs.to_list():
        for library in linker_input.libraries:
            archive = library.static_library or library.pic_static_library
            if archive != None:
                found.append(archive)
    return found

def framework_binaries(cc_info):
    """Returns the dynamic framework binaries one CcInfo links."""
    binaries = []
    for linker_input in cc_info.linking_context.linker_inputs.to_list():
        for library in linker_input.libraries:
            if library.dynamic_library != None:
                binaries.append(library.dynamic_library)
    return binaries

def flat_resources(files):
    """Returns the sorted resource basenames copied into a bundle as is."""
    names = []
    for file in files:
        if file.extension in _COMPILED_RESOURCES:
            continue
        if any([part.endswith(_GROUPED_RESOURCES) for part in file.dirname.split("/")]):
            continue
        if file.basename not in names:
            names.append(file.basename)
    return sorted(names)

def resolved_xcode(ctx, kind, owner):
    """Returns the Xcode config and version, failing when none or another one resolved."""
    xcode_config = ctx.attr._xcode_config[apple_common.XcodeVersionConfig]
    if xcode_config.xcode_version() == None:
        fail("{}: no Xcode resolved for this build; building the {} needs a macOS host with Xcode".format(kind, owner))
    xcode = str(xcode_config.xcode_version())
    if ctx.attr.xcode_version and not version_matches(ctx.attr.xcode_version, xcode):
        fail("{}: {} selects Xcode {} but Bazel resolved {}; pass --xcode_version={}".format(
            kind,
            owner,
            ctx.attr.xcode_version,
            xcode,
            ctx.attr.xcode_version,
        ))
    return xcode_config, xcode

def check_cell(ctx, kind, cell, sdk, mach_o_platform, platform, platform_type, mnemonic, xcode_config, xcode, inputs):
    """Runs the library check over one cell and returns its report."""
    arch = cell.split("/")[1]
    deployment = str(xcode_config.minimum_os_for_platform_type(platform_type))
    report = ctx.actions.declare_file(ctx.label.name + ".txt")
    args = ctx.actions.args()
    args.add(kind)
    args.add(report)
    args.add_all([
        cell,
        sdk,
        mach_o_platform,
        arch,
        deployment,
        xcode,
        str(xcode_config.sdk_version_for_platform(platform)),
    ])
    args.add_all(inputs)
    env = {"PATH": TOOL_PATH}
    env.update(apple_common.apple_host_system_env(xcode_config))
    env.update(apple_common.target_apple_env(xcode_config, platform))
    ctx.actions.run(
        executable = ctx.executable._check,
        arguments = [args],
        inputs = inputs,
        outputs = [report],
        env = env,
        execution_requirements = xcode_config.execution_info(),
        mnemonic = mnemonic,
        resource_set = dx_apple_compile_resources,
        progress_message = "Checking the {} library cell of %{{label}}".format(cell),
    )
    return report

def _adhoc_signing_impl(_settings, _attr):
    return {_SIGNING_CERTIFICATE: "-"}

adhoc_signing_transition = transition(
    implementation = _adhoc_signing_impl,
    inputs = [],
    outputs = [_SIGNING_CERTIFICATE],
)
