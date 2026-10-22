"""Rust Android application packaging over the qualified upstream backend."""

load("@rules_android//rules:rules.bzl", "android_binary")
load("@rules_shell//shell:sh_binary.bzl", "sh_binary")
load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("//android/platforms:defs.bzl", "api_floor")
load("//android/sdk:repos.bzl", "BUILD_TOOLS", "SDK_PLATFORM")
load(":repos.bzl", "ABI_PLATFORMS", "abi_errors", "app_id_errors")

_TARGET_SDK = SDK_PLATFORM.removeprefix("android-")

def rust_android_apk_errors(apk_name, app_id, abis, api_level, version_code):
    """Returns one error string per rejected application package selection."""
    errors = []
    if apk_name == "":
        errors.append("rust_android_apk: name must not be empty")
    errors.extend(app_id_errors(app_id))
    errors.extend(abi_errors(abis))
    if len(abis) == 0:
        errors.append("rust_android_apk: abis must name at least one ABI")
    if api_level != 0 and api_level < api_floor():
        errors.append("rust_android_apk: api_level " + str(api_level) + " is below " + str(api_floor()))
    if version_code <= 0:
        errors.append("rust_android_apk: version_code " + str(version_code) + " must be positive")
    return errors

def apk_manifest_errors(app_id, api_level, version_code, activity):
    """Returns one error string per rejected generated-manifest selection."""
    errors = list(app_id_errors(app_id))
    if api_level != 0 and api_level < api_floor():
        errors.append("rust_android_apk: api_level " + str(api_level) + " is below " + str(api_floor()))
    if version_code <= 0:
        errors.append("rust_android_apk: version_code " + str(version_code) + " must be positive")
    if activity == "":
        errors.append("rust_android_apk: activity must not be empty")
    return errors

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
    attrs = {
        "abis": attr.string_list(mandatory = True),
        "api_level": attr.int(mandatory = True),
        "apk_name": attr.string(mandatory = True),
        "app_id": attr.string(mandatory = True),
        "version_code": attr.int(mandatory = True),
    },
)

def _apk_manifest_impl(ctx):
    failures = apk_manifest_errors(ctx.attr.app_id, ctx.attr.api_level, ctx.attr.version_code, ctx.attr.activity)
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
    attrs = {
        "activity": attr.string(mandatory = True),
        "api_level": attr.int(mandatory = True),
        "app_id": attr.string(mandatory = True),
        "library": attr.string(mandatory = True),
        "permissions": attr.string_list(mandatory = True),
        "version_code": attr.int(mandatory = True),
        "version_name": attr.string(mandatory = True),
    },
    outputs = {"manifest": "%{name}.xml"},
)

def rust_android_apk(
        name,
        app_id,
        deps,
        abis = ["arm64-v8a", "x86_64"],
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
        tags = [],
        **kwargs):
    """Packages one installable APK from native CcInfo deps with a Rust entry point.

    The upstream android_binary links one lib/<name>.so per selected ABI from
    the given static native deps and signs the APK with the debug key. The
    <name>_unsigned_file output is the package that release signing takes. The
    default manifest launches a NativeActivity loading that library. Package
    outputs stay explicit-label only so setups without an SDK keep working.
    """
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
            library = name,
            permissions = permissions,
            version_code = version_code,
            version_name = version_name,
            tags = ["manual"],
            visibility = ["//visibility:private"],
        )
    _apk_file(
        name = name + "_file",
        abis = abis,
        src = ":" + name,
        validation = ":" + name + "_validation",
        tags = ["manual"],
        visibility = visibility,
    )
    _apk_file(
        name = name + "_unsigned_file",
        abis = abis,
        src = ":" + name,
        unsigned = True,
        validation = ":" + name + "_validation",
        tags = ["manual"],
        visibility = visibility,
    )
    android_binary(
        name = name,
        assets = assets,
        assets_dir = assets_dir,
        custom_package = custom_package if custom_package != "" else app_id,
        manifest = manifest,
        resource_files = resource_files,
        deps = deps,
        tags = sorted(set(tags + ["manual"])),
        visibility = visibility,
        **kwargs
    )

def _abi_transition_impl(settings, attr):
    current = settings["//command_line_option:android_platforms"]
    if len(attr.abis) == 0:
        return {"//command_line_option:android_platforms": current}
    return {"//command_line_option:android_platforms": [ABI_PLATFORMS[abi] for abi in attr.abis if abi in ABI_PLATFORMS]}

_abi_transition = transition(
    implementation = _abi_transition_impl,
    inputs = ["//command_line_option:android_platforms"],
    outputs = ["//command_line_option:android_platforms"],
)

def _apk_file_impl(ctx):
    src = ctx.attr.src[0] if type(ctx.attr.src) == "list" else ctx.attr.src
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
    attrs = {
        "abis": attr.string_list(default = []),
        "src": attr.label(allow_files = True, mandatory = True, cfg = _abi_transition),
        "unsigned": attr.bool(default = False),
        "validation": attr.label(),
    },
)

def _adb_selection_impl(ctx):
    failures = []
    if ctx.attr.serial == "":
        failures.append("dx_android_adb: serial names the target device or emulator")
    if ctx.attr.app_id != "":
        failures.extend(app_id_errors(ctx.attr.app_id))
    if ctx.attr.require_activity and ctx.attr.activity == "":
        failures.append("dx_android_adb: activity must not be empty")
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_adb_selection = rule(
    implementation = _adb_selection_impl,
    attrs = {
        "activity": attr.string(default = ""),
        "app_id": attr.string(default = ""),
        "require_activity": attr.bool(default = False),
        "serial": attr.string(default = ""),
    },
)

def android_adb_install(name, apk, serial = "", visibility = None):
    """Declares an explicit APK install launcher; nothing runs at build time."""
    _adb_selection(
        name = name + "_selection",
        serial = serial,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _apk_file(
        name = name + "_apk",
        src = apk,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    sh_binary(
        name = name,
        srcs = ["//android/packaging:adb_ops.sh"],
        data = [":" + name + "_apk", ":" + name + "_selection"],
        env = {
            "DX_ADB_APK": "$(location :" + name + "_apk)",
            "DX_ADB_OP": "install",
            "DX_ADB_SERIAL": serial,
        },
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
    sh_binary(
        name = name,
        srcs = ["//android/packaging:adb_ops.sh"],
        data = [":" + name + "_selection"],
        env = {
            "DX_ADB_ACTIVITY": activity,
            "DX_ADB_APP": app_id,
            "DX_ADB_OP": "start",
            "DX_ADB_SERIAL": serial,
        },
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
    sh_binary(
        name = name,
        srcs = ["//android/packaging:adb_ops.sh"],
        data = [":" + name + "_selection"],
        env = {
            "DX_ADB_OP": "log",
            "DX_ADB_SERIAL": serial,
        },
        tags = ["manual"],
        visibility = visibility,
    )

KNOWN_SCHEMES = ["v1", "v2", "v3", "v4"]

def release_sign_errors(schemes, min_sdk):
    """Returns one error string per rejected release signing selection."""
    errors = []
    if len(schemes) == 0:
        errors.append("android_apk_release_sign: schemes must name at least one signature scheme")
    for scheme in schemes:
        if scheme not in KNOWN_SCHEMES:
            errors.append("android_apk_release_sign: scheme '" + scheme + "' is unknown: want " + ", ".join(KNOWN_SCHEMES))
    if len(schemes) != len(dict([(scheme, True) for scheme in schemes])):
        errors.append("android_apk_release_sign: schemes holds duplicates")
    if min_sdk != 0 and min_sdk < api_floor():
        errors.append("android_apk_release_sign: min_sdk " + str(min_sdk) + " is below " + str(api_floor()))
    return errors

def _sign_selection_impl(ctx):
    failures = release_sign_errors(ctx.attr.schemes, ctx.attr.min_sdk)
    apks = [file for file in ctx.attr.apk[DefaultInfo].files.to_list() if file.extension == "apk"]
    if len(apks) != 1:
        failures.append("android_apk_release_sign: '" + str(ctx.attr.apk.label) + "' built " + str(len(apks)) + " apk files, want exactly one")
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_sign_selection = rule(
    implementation = _sign_selection_impl,
    attrs = {
        "apk": attr.label(allow_files = True, mandatory = True),
        "min_sdk": attr.int(default = 0),
        "schemes": attr.string_list(mandatory = True),
    },
)

def android_apk_release_sign(name, apk, schemes = ["v1", "v2", "v3"], min_sdk = 0, visibility = None):
    """Declares an explicit release signing launcher; builds never sign or read keys."""
    _sign_selection(
        name = name + "_selection",
        apk = apk,
        min_sdk = min_sdk,
        schemes = schemes,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    sh_binary(
        name = name,
        srcs = [Label("//android/packaging:sign_apk.sh")],
        data = [apk, ":" + name + "_selection"],
        env = {
            "DX_SIGN_APK": "$(rootpath " + apk + ")",
            "DX_SIGN_BUILD_TOOLS": BUILD_TOOLS,
            "DX_SIGN_MIN_SDK": str(min_sdk) if min_sdk != 0 else "",
            "DX_SIGN_SCHEMES": ",".join(schemes),
        },
        tags = ["manual"],
        visibility = visibility,
    )

def android_apk_check(name, apk, size = "small", visibility = None):
    """Declares a nonmutating test that the package is an unsigned APK ready to sign."""
    sh_test(
        name = name,
        srcs = [Label("//android/packaging:check_apk.sh")],
        args = ["$(rootpath " + apk + ")"],
        data = [apk],
        size = size,
        visibility = visibility,
    )
