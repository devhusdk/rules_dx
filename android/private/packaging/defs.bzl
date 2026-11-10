"""Rust Android application packaging over the qualified upstream backend."""

load("@rules_android//providers:providers.bzl", "AndroidBundleInfo", "ApkInfo")
load("@rules_android//rules:rules.bzl", "android_binary")
load("@rules_android//rules/android_application:android_application_rule.bzl", "android_application")
load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:resources.bzl", "dx_heavy_resources")
load("@rules_java//java/common:java_common.bzl", "java_common")
load("//android/platforms:defs.bzl", "api_floor")
load("//android/private/sdk:repos.bzl", "BUILD_TOOLS", "SDK_PLATFORM")
load(":repos.bzl", "ABI_PLATFORMS", "abi_errors", "abi_platforms", "app_id_errors", "manifest_lib_name")
load(":validate.bzl", "android_aab_check_errors", "android_apk_manifest_errors", "android_apk_set_errors", "android_release_sign_errors", "rust_android_apk_errors")

_TARGET_SDK = SDK_PLATFORM.removeprefix("android-")

_ZIPALIGN = Label("@dx_android_sdk//:zipalign")

_APKSIGNER_JAR = Label("@dx_android_sdk//:apksigner_jar")

_JAVA_RUNTIME = Label("@rules_java//toolchains:current_java_runtime")

_BUNDLETOOL = Label("//android/private/packaging:bundletool")

_DEBUG_KEYSTORE = Label("@rules_android//tools/android:debug_keystore")

_OPS = Label("//android/private/packaging/ops:android_ops")

_ADB = Label("@dx_android_sdk//:adb")

def _key(ctx, short_path):
    if short_path.startswith("../"):
        return short_path.removeprefix("../")
    return ctx.workspace_name + "/" + short_path

def _ops_launcher(ctx, lines, runfiles):
    text = "\n".join(["program=" + _key(ctx, ctx.executable._ops.short_path)] + lines) + "\n"
    launcher, _, launcher_runfiles = write_native_launcher(ctx, text)
    merged = launcher_runfiles.merge_all([ctx.attr._ops[DefaultInfo].default_runfiles] + runfiles)
    return DefaultInfo(executable = launcher, runfiles = merged)

def _manifest_text(app_id, activity, api_level, library, permissions, version_code, version_name):
    lines = [
        "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\"",
        "    package=\"" + app_id + "\"",
        "    android:versionCode=\"" + str(version_code) + "\"",
        "    android:versionName=\"" + version_name + "\">",
        "",
        "    <uses-sdk android:minSdkVersion=\"" + str(api_level) +
        "\" android:targetSdkVersion=\"" + _TARGET_SDK + "\" />",
        "",
    ]
    for permission in permissions:
        lines.append("    <uses-permission android:name=\"" + permission + "\" />")
    if len(permissions) > 0:
        lines.append("")
    lines.extend([
        "    <application android:label=\"@string/app_name\">",
        "        <activity android:name=\"" + activity + "\"",
        "            android:exported=\"true\"",
        "            android:configChanges=\"orientation|keyboardHidden|screenSize\">",
        "            <meta-data android:name=\"android.app.lib_name\" android:value=\"" + library + "\" />",
        "            <intent-filter>",
        "                <action android:name=\"android.intent.action.MAIN\" />",
        "                <category android:name=\"android.intent.category.LAUNCHER\" />",
        "            </intent-filter>",
        "        </activity>",
        "    </application>",
        "</manifest>",
        "",
    ])
    return "\n".join(lines)

def _apk_validation_impl(ctx):
    failures = rust_android_apk_errors(
        ctx.attr.apk_name,
        ctx.attr.app_id,
        ctx.attr.abis,
        ctx.attr.api_level,
        ctx.attr.version_code,
    )
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_apk_validation = rule(
    implementation = _apk_validation_impl,
    doc = "Fails analysis when an APK package declaration is invalid.",
    attrs = {
        "abis": attr.string_list(
            doc = "ABIs the APK packages, checked for validity.",
            mandatory = True,
        ),
        "api_level": attr.int(
            doc = "Android API level the APK targets, checked for validity.",
            mandatory = True,
        ),
        "apk_name": attr.string(
            doc = "Name of the APK file, checked for validity.",
            mandatory = True,
        ),
        "app_id": attr.string(
            doc = "Application id of the APK, checked for validity.",
            mandatory = True,
        ),
        "version_code": attr.int(
            doc = "Integer version code of the APK, checked for validity.",
            mandatory = True,
        ),
    },
)

def _apk_manifest_impl(ctx):
    failures = android_apk_manifest_errors(ctx.attr.app_id, ctx.attr.api_level, ctx.attr.version_code, ctx.attr.activity)
    if len(failures) > 0:
        fail("; ".join(failures))
    content = _manifest_text(
        ctx.attr.app_id,
        ctx.attr.activity,
        ctx.attr.api_level,
        ctx.attr.library,
        ctx.attr.permissions,
        ctx.attr.version_code,
        ctx.attr.version_name,
    )
    ctx.actions.write(output = ctx.outputs.manifest, content = content)
    return []

_apk_manifest = rule(
    implementation = _apk_manifest_impl,
    doc = "Writes the AndroidManifest.xml for one APK.",
    attrs = {
        "activity": attr.string(
            doc = "Fully qualified activity class the manifest launches.",
            mandatory = True,
        ),
        "api_level": attr.int(
            doc = "Android API level written into the manifest.",
            mandatory = True,
        ),
        "app_id": attr.string(doc = "Application id written into the manifest.", mandatory = True),
        "library": attr.string(doc = "Native library name the activity loads.", mandatory = True),
        "permissions": attr.string_list(
            doc = "Android permissions requested by the manifest.",
            mandatory = True,
        ),
        "version_code": attr.int(
            doc = "Integer version code written into the manifest.",
            mandatory = True,
        ),
        "version_name": attr.string(
            doc = "Human-readable version name written into the manifest.",
            mandatory = True,
        ),
    },
    outputs = {"manifest": "%{name}.xml"},
)

def abis_for_platforms(platforms):
    """Returns the ABI name of each platform label; an unknown label stays as its text for abi_errors."""
    if type(platforms) != "list":
        return platforms
    by_label = {label: abi for abi, label in ABI_PLATFORMS.items()}
    labels = [str(native.package_relative_label(platform)) for platform in platforms]
    return [by_label.get(label, label) for label in labels]

def rust_android_apk(
        name,
        app_id,
        deps,
        platforms = [Label("//android/platforms:arm64-v8a"), Label("//android/platforms:x86_64")],
        activity = "android.app.NativeActivity",
        api_level = 0,
        assets = [],
        assets_dir = "",
        custom_package = "",
        manifest = None,
        permissions = [],
        resource_files = [],
        version_code = 1,
        version_name = "1.0",
        visibility = None,
        tags = None,
        **kwargs):
    """Packages one APK and one unsigned App Bundle from native CcInfo deps with a Rust entry point."""
    tags = tags or []
    inner = ":" + name + "_bin"
    abis = abis_for_platforms(platforms)
    package = custom_package if custom_package != "" else app_id
    effective_api = api_level if api_level != 0 else api_floor()
    _apk_validation(
        name = name + "_validation",
        abis = abis,
        api_level = api_level,
        apk_name = name,
        app_id = app_id,
        version_code = version_code,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    if manifest == None:
        manifest = ":" + name + "_manifest"
        _apk_manifest(
            name = name + "_manifest",
            activity = activity,
            api_level = effective_api,
            app_id = app_id,
            library = manifest_lib_name(name),
            permissions = permissions,
            version_code = version_code,
            version_name = version_name,
            tags = ["manual"],
            visibility = ["//visibility:private"],
        )
    _apk_file(
        name = name + "_file",
        src = ":" + name,
        tags = ["manual"],
        visibility = visibility,
    )
    _apk_file(
        name = name + "_unsigned_file",
        src = ":" + name,
        unsigned = True,
        tags = ["manual"],
        visibility = visibility,
    )
    plain = not _has_select([abis, app_id, api_level, version_code])
    invalid = plain and len(rust_android_apk_errors(name, app_id, abis, api_level, version_code)) > 0
    android_application(
        name = name + "_bundle",
        base_module = inner,
        custom_package = package,
        manifest_values = dict(kwargs.get("manifest_values", {}), applicationId = app_id),
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _aab_file(
        name = name + "_aab_file",
        abis = abis,
        src = None if invalid else ":" + name + "_bundle",
        validation = ":" + name + "_validation",
        tags = ["manual"],
        visibility = visibility,
    )
    testonly = kwargs.get("testonly", False)
    android_binary(
        name = name + "_bin",
        assets = assets,
        assets_dir = assets_dir,
        custom_package = package,
        manifest = manifest,
        resource_files = resource_files,
        deps = deps,
        tags = sorted(set(tags + ["manual"])),
        visibility = ["//visibility:private"],
        **kwargs
    )
    _rust_android_apk(
        name = name,
        abis = abis,
        binary = None if invalid else inner,
        testonly = testonly,
        validation = ":" + name + "_validation",
        tags = sorted(set(tags + ["manual"])),
        visibility = visibility,
    )

def _has_select(values):
    for value in values:
        if type(value) not in ["string", "int", "list", "bool"]:
            return True
    return False

# rules_android 0.8.0 reads its Starlark flag before the native option (rules/utils.bzl
# _get_android_platforms) and aliases `--android_platforms` to it, so the transition writes
# both. Drop the native option once Bazel 10 removes it.
_NATIVE_PLATFORMS = "//command_line_option:android_platforms"
_STARLARK_PLATFORMS = "@rules_android//flags:android_platforms"

def abi_transition_settings(abis, settings):
    """Returns the platform settings the ABI transition writes (pure, for tests)."""
    if len(abis) == 0:
        return {
            _NATIVE_PLATFORMS: settings[_NATIVE_PLATFORMS],
            _STARLARK_PLATFORMS: settings[_STARLARK_PLATFORMS],
        }
    errors = abi_errors(abis)
    if len(errors) > 0:
        fail("; ".join(errors))
    platforms = abi_platforms(abis)
    return {_NATIVE_PLATFORMS: platforms, _STARLARK_PLATFORMS: platforms}

def _abi_transition_impl(settings, attr):
    return abi_transition_settings(attr.abis, settings)

_abi_transition = transition(
    implementation = _abi_transition_impl,
    inputs = [_STARLARK_PLATFORMS, _NATIVE_PLATFORMS],
    outputs = [_STARLARK_PLATFORMS, _NATIVE_PLATFORMS],
)

def _rust_android_apk_impl(ctx):
    outputs = []
    binary = ctx.attr.binary[0] if type(ctx.attr.binary) == "list" and ctx.attr.binary else None
    if binary:
        for file in binary[DefaultInfo].files.to_list():
            if file.extension != "apk":
                continue
            suffix = "_unsigned.apk" if file.basename.endswith("_unsigned.apk") else ".apk"
            out = ctx.actions.declare_file(ctx.label.name + suffix)
            ctx.actions.symlink(output = out, target_file = file)
            outputs.append(out)
    providers = [DefaultInfo(files = depset(outputs))]
    if binary and ApkInfo in binary:
        providers.append(binary[ApkInfo])
    if binary and OutputGroupInfo in binary:
        providers.append(binary[OutputGroupInfo])
    return providers

_rust_android_apk = rule(
    implementation = _rust_android_apk_impl,
    doc = "Builds the inner android_binary for the selected ABIs.",
    attrs = {
        "abis": attr.string_list(default = [], doc = "ABIs the APK packages."),
        "binary": attr.label(cfg = _abi_transition, doc = "Inner android_binary."),
        "validation": attr.label(doc = "Selection target that fails analysis on a rejected package."),
    },
)

DxApkSetInfo = provider(
    doc = "A built APK set: the extracted split APK directory, the .apks archive, and the device spec.",
    fields = {
        "apks": "The .apks archive File.",
        "apks_dir": "Tree artifact of extracted split APKs.",
        "device_spec": "The device spec JSON File.",
    },
)

def _apk_set_dir_impl(ctx):
    src = ctx.attr.src
    if DxApkSetInfo not in src:
        fail("android_adb_install: '" + str(src.label) + "' is not an android_apk_set")
    return [DefaultInfo(files = depset([src[DxApkSetInfo].apks_dir]))]

_apk_set_dir = rule(
    implementation = _apk_set_dir_impl,
    doc = "Exposes the extracted split APK directory of an android_apk_set.",
    attrs = {
        "src": attr.label(doc = "An android_apk_set target.", allow_files = True, mandatory = True),
    },
)

def _apk_file_impl(ctx):
    src = ctx.attr.src
    kind = "unsigned" if ctx.attr.unsigned else "signed"
    apks = [
        file
        for file in src[DefaultInfo].files.to_list()
        if file.extension == "apk" and file.basename.endswith("_unsigned.apk") == ctx.attr.unsigned
    ]
    if len(apks) != 1:
        fail("android_apk_file: '" + str(src.label) + "' built " + str(len(apks)) + " " + kind + " apk files, want exactly one")
    return [DefaultInfo(files = depset(apks))]

_apk_file = rule(
    implementation = _apk_file_impl,
    doc = "Exposes the one signed or unsigned APK an android_binary builds.",
    attrs = {
        "src": attr.label(
            doc = "Target whose default outputs include the built APK.",
            allow_files = True,
            mandatory = True,
        ),
        "unsigned": attr.bool(
            doc = "Selects the unsigned APK instead of the signed one.",
            default = False,
        ),
    },
)

def _aab_file_impl(ctx):
    if ctx.attr.src == None or (type(ctx.attr.src) == "list" and len(ctx.attr.src) == 0):
        fail("android_aab_file: no app bundle; the package declaration was rejected")
    src = ctx.attr.src[0] if type(ctx.attr.src) == "list" else ctx.attr.src
    if AndroidBundleInfo not in src:
        fail("android_aab_file: '" + str(src.label) + "' provides no app bundle")
    return [DefaultInfo(files = depset([src[AndroidBundleInfo].unsigned_aab]))]

_aab_file = rule(
    implementation = _aab_file_impl,
    doc = "Exposes the unsigned App Bundle of one android_application for the selected ABIs.",
    attrs = {
        "abis": attr.string_list(default = [], doc = "ABIs the bundle packages."),
        "src": attr.label(cfg = _abi_transition, doc = "android_application that builds the bundle; unset when validation rejects the package."),
        "validation": attr.label(doc = "Selection target that fails analysis on a rejected package."),
    },
)

def _adb_selection_impl(ctx):
    failures = []
    if ctx.attr.serial == "":
        failures.append("dx_android_adb: serial names the target device or emulator")
    if ctx.attr.require_install and ctx.attr.apk_count + ctx.attr.apk_set_count != 1:
        failures.append("dx_android_adb: install takes exactly one of apk or apk_set")
    if ctx.attr.app_id != "":
        failures.extend(app_id_errors(ctx.attr.app_id))
    if ctx.attr.require_activity and ctx.attr.activity == "":
        failures.append("dx_android_adb: activity must not be empty")
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_adb_selection = rule(
    implementation = _adb_selection_impl,
    doc = "Fails analysis when an adb launcher declaration is invalid.",
    attrs = {
        "activity": attr.string(
            doc = "Activity launched on the device, required when require_activity is set.",
            default = "",
        ),
        "apk_count": attr.int(
            doc = "Number of apk attributes supplied to the install launcher.",
            default = 0,
        ),
        "apk_set_count": attr.int(
            doc = "Number of apk_set attributes supplied to the install launcher.",
            default = 0,
        ),
        "app_id": attr.string(
            doc = "Application id to launch, validated when non-empty.",
            default = "",
        ),
        "require_activity": attr.bool(
            doc = "Whether an empty activity fails analysis.",
            default = False,
        ),
        "require_install": attr.bool(
            doc = "Whether exactly one of apk or apk_set must be supplied.",
            default = False,
        ),
        "serial": attr.string(
            doc = "Device or emulator serial the adb operation targets.",
            default = "",
        ),
    },
)

def _adb_launcher_impl(ctx):
    lines = ["arg=adb"]
    files = []
    for flag, found in [("--adb=", ctx.files._adb), ("--apk=", ctx.files.apk), ("--apks=", ctx.files.apk_set)]:
        if len(found) > 0:
            lines.append("prefixed=" + flag + "\t" + _key(ctx, found[0].short_path))
            files.append(found[0])
    for flag, value in [("--op=", ctx.attr.op), ("--serial=", ctx.attr.serial), ("--app=", ctx.attr.app_id), ("--activity=", ctx.attr.activity)]:
        if value != "":
            lines.append("arg=" + flag + value)
    return [_ops_launcher(ctx, lines, [ctx.runfiles(files = files)])]

_adb_launcher = rule(
    implementation = _adb_launcher_impl,
    doc = "Launcher that runs one adb operation against one device.",
    executable = True,
    attrs = {
        "activity": attr.string(default = "", doc = "Activity the start operation launches."),
        "apk": attr.label(allow_files = True, doc = "The APK the install operation installs."),
        "apk_set": attr.label(allow_files = True, doc = "Directory of split APKs the install-multiple operation installs."),
        "app_id": attr.string(default = "", doc = "Application id the start operation launches."),
        "op": attr.string(mandatory = True, values = ["install", "install-multiple", "start", "log"], doc = "The adb operation."),
        "selection": attr.label(doc = "Selection target that fails analysis on a rejected declaration."),
        "serial": attr.string(default = "", doc = "Device or emulator serial the operation targets."),
        "_adb": attr.label(default = _ADB, cfg = "exec", allow_files = True, doc = "Pinned adb; empty without the platform-tools package."),
        "_ops": attr.label(default = _OPS, cfg = "target", executable = True, doc = "The Android operations program."),
    } | NATIVE_LAUNCHER_ATTRS,
)

def android_adb_install(name, apk = None, serial = "", apk_set = None, visibility = None):
    """Declares an explicit APK or APK set install launcher; nothing runs at build time."""
    _adb_selection(
        name = name + "_selection",
        apk_count = 0 if apk == None else 1,
        apk_set_count = 0 if apk_set == None else 1,
        require_install = True,
        serial = serial,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    apk_file = None
    apk_set_dir = None
    if apk_set != None:
        _apk_set_dir(
            name = name + "_apk_set",
            src = apk_set,
            tags = ["manual"],
            visibility = ["//visibility:private"],
        )
        apk_set_dir = ":" + name + "_apk_set"
    if apk != None:
        _apk_file(
            name = name + "_apk",
            src = apk,
            tags = ["manual"],
            visibility = ["//visibility:private"],
        )
        apk_file = ":" + name + "_apk"
    _adb_launcher(
        name = name,
        apk = apk_file,
        apk_set = apk_set_dir,
        op = "install" if apk != None else "install-multiple",
        selection = ":" + name + "_selection",
        serial = serial,
        tags = ["manual"],
        visibility = visibility,
    )

def android_adb_start(name, app_id, activity = "android.app.NativeActivity", serial = "", visibility = None):
    """Declares an explicit application launch launcher; nothing runs at build time."""
    _adb_selection(
        name = name + "_selection",
        activity = activity,
        app_id = app_id,
        require_activity = True,
        serial = serial,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _adb_launcher(
        name = name,
        activity = activity,
        app_id = app_id,
        op = "start",
        selection = ":" + name + "_selection",
        serial = serial,
        tags = ["manual"],
        visibility = visibility,
    )

def android_adb_log(name, serial = "", visibility = None):
    """Declares an explicit logcat dump launcher; nothing runs at build time."""
    _adb_selection(
        name = name + "_selection",
        serial = serial,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _adb_launcher(
        name = name,
        op = "log",
        selection = ":" + name + "_selection",
        serial = serial,
        tags = ["manual"],
        visibility = visibility,
    )

DEFAULT_SCHEMES = ["v1", "v2", "v3"]

def _sign_selection_impl(ctx):
    files = [file for file in ctx.attr.package[DefaultInfo].files.to_list() if file.extension in ["apk", "aab"]]
    if len(files) != 1:
        fail("android_sign: '" + str(ctx.attr.package.label) + "' built " + str(len(files)) + " .apk/.aab files, want exactly one; point package at one APK or App Bundle")
    if files[0].extension == "aab":
        if len(ctx.attr.schemes) > 0 or ctx.attr.min_sdk != 0:
            fail("android_sign: schemes and min_sdk apply to APKs only")
        return []
    failures = android_release_sign_errors(ctx.attr.schemes or DEFAULT_SCHEMES, ctx.attr.min_sdk)
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_sign_selection = rule(
    implementation = _sign_selection_impl,
    doc = "Fails analysis when a release signing declaration is invalid.",
    attrs = {
        "min_sdk": attr.int(
            doc = "Minimum SDK level the signature covers, zero leaves it unset.",
            default = 0,
        ),
        "package": attr.label(
            doc = "APK or App Bundle the release signing launcher signs.",
            allow_files = True,
            mandatory = True,
        ),
        "schemes": attr.string_list(
            doc = "Signature schemes to apply, such as v1, v2, and v3.",
        ),
    },
)

def _java_key(ctx, path):
    if path.startswith("/"):
        return path
    if path.startswith("external/"):
        return path.removeprefix("external/")
    return _key(ctx, path)

def _tool_line(ctx, flag, path):
    key = _java_key(ctx, path)
    if key.startswith("/"):
        return "arg=" + flag + key
    return "prefixed=" + flag + "\t" + key

def _sign_launcher_impl(ctx):
    is_apk = ctx.file.input.extension == "apk"
    java = ctx.attr._java_runtime[java_common.JavaRuntimeInfo]
    runfiles = [
        ctx.runfiles(files = [ctx.file.input], transitive_files = ctx.attr._java_runtime[DefaultInfo].files),
    ]
    lines = [
        "arg=sign-apk" if is_apk else "arg=sign-aab",
        _tool_line(ctx, "--apk=" if is_apk else "--aab=", ctx.file.input.short_path),
        _tool_line(ctx, "--java=", java.java_executable_runfiles_path),
    ]
    if is_apk:
        lines.append(_tool_line(ctx, "--zipalign=", ctx.file._zipalign.short_path))
        lines.append(_tool_line(ctx, "--apksigner-jar=", ctx.file._apksigner_jar.short_path))
        lines.append("arg=--build-tools=" + BUILD_TOOLS)
        if ctx.attr.min_sdk != 0:
            lines.append("arg=--min-sdk=" + str(ctx.attr.min_sdk))
        lines.append("arg=--schemes=" + ",".join(ctx.attr.schemes))
        runfiles.append(ctx.runfiles(files = [ctx.file._zipalign, ctx.file._apksigner_jar]))
    else:
        lines.append(_tool_line(ctx, "--bundletool=", ctx.executable._bundletool.short_path))
        runfiles.append(ctx.attr._bundletool[DefaultInfo].default_runfiles)
        runfiles.append(ctx.runfiles(files = [ctx.executable._bundletool]))
    return [_ops_launcher(ctx, lines, runfiles)]

_sign_launcher = rule(
    implementation = _sign_launcher_impl,
    doc = "Signing launcher whose tools (zipalign, apksigner, bundletool, JDK) are built for the execution platform.",
    executable = True,
    attrs = {
        "input": attr.label(allow_single_file = True, mandatory = True, doc = "The APK or AAB to sign."),
        "min_sdk": attr.int(default = 0, doc = "Lowest Android API level the signature must support."),
        "schemes": attr.string_list(doc = "APK signature schemes to use."),
        "selection": attr.label(doc = "Signing key selection to use."),
        "_apksigner_jar": attr.label(default = _APKSIGNER_JAR, cfg = "exec", allow_single_file = True),
        "_bundletool": attr.label(default = _BUNDLETOOL, cfg = "exec", executable = True),
        "_java_runtime": attr.label(default = _JAVA_RUNTIME, cfg = "exec", providers = [java_common.JavaRuntimeInfo]),
        "_ops": attr.label(default = _OPS, cfg = "target", executable = True, doc = "The Android operations program."),
        "_zipalign": attr.label(default = _ZIPALIGN, cfg = "exec", allow_single_file = True),
    } | NATIVE_LAUNCHER_ATTRS,
)

def android_sign(name, package, schemes = None, min_sdk = 0, visibility = None):
    """Declares an explicit release signing launcher for an APK or App Bundle; builds never sign or read keys."""
    _sign_selection(
        name = name + "_selection",
        package = package,
        min_sdk = min_sdk,
        schemes = schemes or [],
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _sign_launcher(
        name = name,
        input = package,
        selection = ":" + name + "_selection",
        min_sdk = min_sdk,
        schemes = schemes or DEFAULT_SCHEMES,
        tags = ["manual"],
        visibility = visibility,
    )

def _check_selection_impl(ctx):
    files = [file for file in ctx.attr.package[DefaultInfo].files.to_list() if file.extension in ["apk", "aab"]]
    if len(files) != 1:
        fail("android_check: '" + str(ctx.attr.package.label) + "' built " + str(len(files)) + " .apk/.aab files, want exactly one; point package at one APK or App Bundle")
    failures = android_aab_check_errors(ctx.attr.abis, ctx.attr.resources)
    if files[0].extension == "apk" and (len(ctx.attr.abis) > 0 or len(ctx.attr.resources) > 0):
        failures.append("android_check: platforms and resources apply to App Bundles only")
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_check_selection = rule(
    implementation = _check_selection_impl,
    doc = "Fails analysis when a package check names a bad package, ABI or resource.",
    attrs = {
        "package": attr.label(allow_files = True, mandatory = True, doc = "The APK or App Bundle to check."),
        "abis": attr.string_list(default = [], doc = "ABIs the bundle must package."),
        "resources": attr.string_list(default = [], doc = "type/name resources the bundle must resolve."),
    },
)

def android_check_selection(name, package, platforms = [], resources = [], target_compatible_with = None):
    """Declares the validation of an android_check without its test, for analysis-failure proofs."""
    _check_selection(
        name = name + "_selection",
        package = package,
        abis = abis_for_platforms(platforms),
        resources = resources,
        tags = ["manual"],
        target_compatible_with = target_compatible_with,
        visibility = ["//visibility:private"],
    )

def _package_check_test_impl(ctx):
    packages = [file for file in ctx.files.package if file.extension in ["apk", "aab"]]
    if len(packages) != 1:
        fail("android_check: '" + str(ctx.attr.package.label) + "' built " + str(len(packages)) + " .apk/.aab files, want exactly one; point package at one APK or App Bundle")
    lines = [
        "arg=check",
        "path=" + _key(ctx, ctx.executable._bundletool.short_path),
        "path=" + _key(ctx, packages[0].short_path),
    ]
    for abi in ctx.attr.abis:
        lines.extend(["arg=--abi", "arg=" + abi])
    for resource in ctx.attr.resources:
        lines.extend(["arg=--resource", "arg=" + resource])
    runfiles = [
        ctx.runfiles(files = [packages[0], ctx.executable._bundletool]),
        ctx.attr._bundletool[DefaultInfo].default_runfiles,
    ]
    return [_ops_launcher(ctx, lines, runfiles)]

_package_check_test = rule(
    implementation = _package_check_test_impl,
    doc = "Tests that the package is an unsigned APK or a valid App Bundle.",
    test = True,
    attrs = {
        "abis": attr.string_list(default = [], doc = "ABIs the bundle must package."),
        "package": attr.label(allow_files = True, mandatory = True, doc = "The APK or App Bundle to check."),
        "resources": attr.string_list(default = [], doc = "type/name resources the bundle must resolve."),
        "selection": attr.label(doc = "Selection target that fails analysis on a rejected declaration."),
        "_bundletool": attr.label(default = _BUNDLETOOL, cfg = "target", executable = True, doc = "Pinned bundletool."),
        "_ops": attr.label(default = _OPS, cfg = "target", executable = True, doc = "The Android operations program."),
    } | NATIVE_LAUNCHER_ATTRS,
)

def android_check(name, package, platforms = [], resources = [], size = "small", tags = None, target_compatible_with = None, visibility = None):
    """Declares a test that the package is an unsigned APK or a valid App Bundle."""
    abis = abis_for_platforms(platforms)
    android_check_selection(
        name = name,
        package = package,
        platforms = platforms,
        resources = resources,
        target_compatible_with = target_compatible_with,
    )
    _package_check_test(
        name = name,
        abis = abis,
        package = package,
        resources = resources,
        selection = ":" + name + "_selection",
        size = size,
        tags = tags,
        target_compatible_with = target_compatible_with,
        visibility = visibility,
    )

def _apk_set_impl(ctx):
    aabs = [file for file in ctx.files.aab if file.extension == "aab"]
    specs = [file for file in ctx.files.device_spec if file.extension == "json"]
    failures = android_apk_set_errors(str(ctx.label), len(aabs), len(specs))
    if len(failures) > 0:
        fail("; ".join(failures))
    out_dir = ctx.actions.declare_directory(ctx.label.name + "_apks")
    apks = ctx.actions.declare_file(ctx.label.name + ".apks")
    bundletool = ctx.attr._bundletool[DefaultInfo].files_to_run
    java = ctx.attr._java_runtime[java_common.JavaRuntimeInfo]
    ctx.actions.run(
        executable = ctx.executable._ops,
        arguments = [
            "apk-set",
            bundletool.executable.path,
            aabs[0].path,
            specs[0].path,
            ctx.file._keystore.path,
            apks.path,
            out_dir.path,
        ],
        inputs = depset([aabs[0], specs[0], ctx.file._keystore], transitive = [ctx.attr._java_runtime[DefaultInfo].files]),
        outputs = [apks, out_dir],
        tools = [bundletool],
        env = {"JAVA_HOME": java.java_home},
        mnemonic = "DxApkSet",
        resource_set = dx_heavy_resources,
        progress_message = "Building APK set %{label}",
    )
    return [
        DefaultInfo(files = depset([out_dir])),
        DxApkSetInfo(apks_dir = out_dir, apks = apks, device_spec = specs[0]),
        OutputGroupInfo(apks = depset([apks])),
    ]

android_apk_set = rule(
    implementation = _apk_set_impl,
    doc = "Builds the debug-signed split APKs that one App Bundle installs on one device spec.",
    attrs = {
        "aab": attr.label(allow_files = True, mandatory = True, doc = "Target that builds the App Bundle."),
        "device_spec": attr.label(allow_files = True, mandatory = True, doc = "bundletool device spec JSON."),
        "_bundletool": attr.label(default = _BUNDLETOOL, executable = True, cfg = "exec", doc = "Pinned bundletool."),
        "_java_runtime": attr.label(default = _JAVA_RUNTIME, cfg = "exec", providers = [java_common.JavaRuntimeInfo]),
        "_keystore": attr.label(default = _DEBUG_KEYSTORE, allow_single_file = True, doc = "Public rules_android debug keystore; debug installs only."),
        "_ops": attr.label(default = _OPS, executable = True, cfg = "exec", doc = "The Android operations program."),
    },
)
