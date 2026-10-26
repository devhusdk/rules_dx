"""Rust Android application packaging over the qualified upstream backend."""

load("@rules_android//providers:providers.bzl", "AndroidBundleInfo", "ApkInfo")
load("@rules_android//rules:rules.bzl", "android_binary")
load("@rules_android//rules/android_application:android_application_rule.bzl", "android_application")
load("@rules_java//java/common:java_common.bzl", "java_common")
load("@rules_shell//shell:sh_binary.bzl", "sh_binary")
load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("//android/platforms:defs.bzl", "api_floor")
load("//android/sdk:repos.bzl", "SDK_PLATFORM")
load(":repos.bzl", "abi_errors", "abi_platforms", "app_id_errors", "manifest_lib_name")

_TARGET_SDK = SDK_PLATFORM.removeprefix("android-")

_ZIPALIGN = Label("@dx_android_sdk//:zipalign")

_APKSIGNER_JAR = Label("@dx_android_sdk//:apksigner_jar")

_JAVA_RUNTIME = Label("@rules_java//toolchains:current_java_runtime")

_BUNDLETOOL = Label("//android/packaging:bundletool")

_DEBUG_KEYSTORE = Label("@rules_android//tools/android:debug_keystore")

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
    """Packages one APK and one unsigned App Bundle from native CcInfo deps with a Rust entry point."""
    inner = ":" + name + "_bin"
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
    android_application(
        name = name + "_bundle",
        base_module = inner,
        custom_package = package,
        manifest_values = dict(kwargs.get("manifest_values", {}), applicationId = app_id),
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    plain = not _has_select([abis, app_id, api_level, version_code])
    invalid = plain and len(rust_android_apk_errors(name, app_id, abis, api_level, version_code)) > 0
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

def _abi_transition_impl(settings, attr):
    current = settings["//command_line_option:android_platforms"]
    if len(attr.abis) == 0:
        return {"//command_line_option:android_platforms": current}
    errors = abi_errors(attr.abis)
    if len(errors) > 0:
        fail("; ".join(errors))
    return {"//command_line_option:android_platforms": abi_platforms(attr.abis)}

_abi_transition = transition(
    implementation = _abi_transition_impl,
    inputs = ["//command_line_option:android_platforms"],
    outputs = ["//command_line_option:android_platforms"],
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
    data = [":" + name + "_selection"]
    env = {"DX_ADB_SERIAL": serial}
    if apk_set != None:
        data.append(apk_set)
        env["DX_ADB_APKS"] = "$(rootpath " + apk_set + ")"
        env["DX_ADB_OP"] = "install-multiple"
    if apk != None:
        _apk_file(
            name = name + "_apk",
            src = apk,
            tags = ["manual"],
            visibility = ["//visibility:private"],
        )
        data.insert(0, ":" + name + "_apk")
        env["DX_ADB_APK"] = "$(location :" + name + "_apk)"
        env["DX_ADB_OP"] = "install"
    sh_binary(
        name = name,
        srcs = [Label("//android/packaging:adb_ops.sh")],
        data = data,
        env = env,
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
        srcs = [Label("//android/packaging:adb_ops.sh")],
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
        srcs = [Label("//android/packaging:adb_ops.sh")],
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
    if "v4" in schemes and "v2" not in schemes and "v3" not in schemes:
        errors.append("android_apk_release_sign: scheme v4 needs v2 or v3")
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
    doc = "Fails analysis when a release signing declaration is invalid.",
    attrs = {
        "apk": attr.label(
            doc = "APK file the release signing launcher signs.",
            allow_files = True,
            mandatory = True,
        ),
        "min_sdk": attr.int(
            doc = "Minimum SDK level the signature covers, zero leaves it unset.",
            default = 0,
        ),
        "schemes": attr.string_list(
            doc = "Signature schemes to apply, such as v1, v2, and v3.",
            mandatory = True,
        ),
    },
)

def _rel(path):
    if path.startswith("/") or path.startswith("../"):
        return path
    if path.startswith("external/"):
        return "../" + path.removeprefix("external/")
    return "../" + path

def _sign_launcher_impl(ctx):
    env = {}
    runfiles = [ctx.runfiles(files = [ctx.file.input, ctx.file.script])]
    if ctx.attr.kind == "apk":
        java = ctx.attr._java_runtime[java_common.JavaRuntimeInfo]
        env["DX_SIGN_APK"] = ctx.file.input.short_path
        env["DX_SIGN_JAVA"] = _rel(java.java_executable_runfiles_path)
        env["DX_SIGN_ZIPALIGN"] = ctx.file.zipalign.short_path
        env["DX_SIGN_APKSIGNER_JAR"] = ctx.file.apksigner_jar.short_path
        env["DX_SIGN_MIN_SDK"] = str(ctx.attr.min_sdk) if ctx.attr.min_sdk != 0 else ""
        env["DX_SIGN_SCHEMES"] = ",".join(ctx.attr.schemes)
        runfiles.append(ctx.runfiles(files = [ctx.file.zipalign, ctx.file.apksigner_jar]))
    else:
        java = ctx.attr._java_runtime[java_common.JavaRuntimeInfo]
        env["DX_SIGN_AAB"] = ctx.file.input.short_path
        env["DX_SIGN_BUNDLETOOL"] = ctx.executable.bundletool.short_path
        env["DX_SIGN_JAVABASE"] = _rel(java.java_home_runfiles_path)
        runfiles.append(ctx.attr.bundletool[DefaultInfo].default_runfiles)
        runfiles.append(ctx.runfiles(files = [ctx.executable.bundletool]))
    runfiles.append(ctx.runfiles(transitive_files = ctx.attr._java_runtime[DefaultInfo].files))
    out = ctx.actions.declare_file(ctx.label.name)
    lines = ["#!/bin/sh"]
    for key, value in sorted(env.items()):
        lines.append("export {0}=\"${{{0}-{1}}}\"".format(key, value))
    lines.append("exec \"$PWD/{}\" \"$@\"".format(ctx.file.script.short_path))
    ctx.actions.write(out, "\n".join(lines) + "\n", is_executable = True)
    return [DefaultInfo(executable = out, runfiles = ctx.runfiles().merge_all(runfiles))]

_sign_launcher = rule(
    implementation = _sign_launcher_impl,
    doc = "Signing launcher whose tools (zipalign, apksigner, bundletool, JDK) are built for the execution platform.",
    executable = True,
    attrs = {
        "apksigner_jar": attr.label(cfg = "exec", allow_single_file = True),
        "bundletool": attr.label(cfg = "exec", executable = True),
        "input": attr.label(allow_single_file = True, mandatory = True),
        "kind": attr.string(mandatory = True, values = ["apk", "aab"]),
        "min_sdk": attr.int(default = 0),
        "schemes": attr.string_list(),
        "script": attr.label(allow_single_file = True, mandatory = True),
        "selection": attr.label(),
        "zipalign": attr.label(cfg = "exec", allow_single_file = True),
        "_java_runtime": attr.label(default = _JAVA_RUNTIME, cfg = "exec", providers = [java_common.JavaRuntimeInfo]),
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
    _sign_launcher(
        name = name,
        kind = "apk",
        input = apk,
        script = Label("//android/packaging:sign_apk.sh"),
        selection = ":" + name + "_selection",
        min_sdk = min_sdk,
        schemes = schemes,
        zipalign = _ZIPALIGN,
        apksigner_jar = _APKSIGNER_JAR,
        tags = ["manual"],
        visibility = visibility,
    )

def aab_sign_errors(label, aab_count):
    """Returns one error string per rejected App Bundle signing selection."""
    if aab_count != 1:
        return ["android_aab_release_sign: '" + label + "' built " + str(aab_count) + " aab files, want exactly one"]
    return []

def _aab_sign_selection_impl(ctx):
    aabs = [file for file in ctx.attr.aab[DefaultInfo].files.to_list() if file.extension == "aab"]
    failures = aab_sign_errors(str(ctx.attr.aab.label), len(aabs))
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_aab_sign_selection = rule(
    implementation = _aab_sign_selection_impl,
    doc = "Fails analysis unless the signing input builds exactly one App Bundle.",
    attrs = {
        "aab": attr.label(allow_files = True, mandatory = True, doc = "Target that builds the unsigned App Bundle."),
    },
)

def android_aab_release_sign(name, aab, visibility = None):
    """Declares an explicit App Bundle upload signing launcher; builds never sign or read keys."""
    _aab_sign_selection(
        name = name + "_selection",
        aab = aab,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _sign_launcher(
        name = name,
        kind = "aab",
        input = aab,
        script = Label("//android/packaging:sign_aab.sh"),
        selection = ":" + name + "_selection",
        bundletool = _BUNDLETOOL,
        tags = ["manual"],
        visibility = visibility,
    )

def android_apk_check(name, apk, size = "small", tags = None, visibility = None):
    """Declares a nonmutating test that the package is an unsigned APK ready to sign."""
    sh_test(
        name = name,
        srcs = [Label("//android/packaging:check_apk.sh")],
        args = ["$(rootpath " + apk + ")"],
        data = [apk],
        size = size,
        tags = tags,
        visibility = visibility,
    )

def aab_check_errors(abis, resources):
    """Returns one error string per rejected App Bundle check selection."""
    errors = list(abi_errors(abis))
    for resource in resources:
        parts = resource.split("/")
        if len(parts) != 2 or parts[0] == "" or parts[1] == "":
            errors.append("android_aab_check: resource '" + resource + "' must be type/name")
    return errors

def _aab_check_selection_impl(ctx):
    failures = aab_check_errors(ctx.attr.abis, ctx.attr.resources)
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_aab_check_selection = rule(
    implementation = _aab_check_selection_impl,
    doc = "Fails analysis when an App Bundle check names an unknown ABI or a malformed resource.",
    attrs = {
        "abis": attr.string_list(default = [], doc = "ABIs the bundle must package."),
        "resources": attr.string_list(default = [], doc = "type/name resources the bundle must resolve."),
    },
)

def android_aab_check(name, aab, abis = [], resources = [], size = "small", tags = None, visibility = None):
    """Declares a test that bundletool accepts the App Bundle with the given ABIs and resources."""
    _aab_check_selection(
        name = name + "_selection",
        abis = abis,
        resources = resources,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    args = ["$(rootpath " + str(_BUNDLETOOL) + ")", "$(rootpath " + aab + ")"]
    for abi in abis:
        args.extend(["--abi", abi])
    for resource in resources:
        args.extend(["--resource", resource])
    sh_test(
        name = name,
        srcs = [Label("//android/packaging:check_aab.sh")],
        args = args,
        data = [aab, _BUNDLETOOL, ":" + name + "_selection"],
        size = size,
        tags = tags,
        visibility = visibility,
    )

def apk_set_errors(label, aab_count, spec_count):
    """Returns one error string per rejected APK set selection."""
    errors = []
    if aab_count != 1:
        errors.append("android_apk_set: '" + label + "' aab built " + str(aab_count) + " aab files, want exactly one")
    if spec_count != 1:
        errors.append("android_apk_set: '" + label + "' device_spec names " + str(spec_count) + " json files, want exactly one")
    return errors

def _apk_set_impl(ctx):
    aabs = [file for file in ctx.files.aab if file.extension == "aab"]
    specs = [file for file in ctx.files.device_spec if file.extension == "json"]
    failures = apk_set_errors(str(ctx.label), len(aabs), len(specs))
    if len(failures) > 0:
        fail("; ".join(failures))
    out_dir = ctx.actions.declare_directory(ctx.label.name + "_apks")
    apks = ctx.actions.declare_file(ctx.label.name + ".apks")
    bundletool = ctx.attr._bundletool[DefaultInfo].files_to_run
    ctx.actions.run(
        executable = ctx.executable._build_apk_set,
        arguments = [
            bundletool.executable.path,
            aabs[0].path,
            specs[0].path,
            ctx.file._keystore.path,
            apks.path,
            out_dir.path,
        ],
        inputs = [aabs[0], specs[0], ctx.file._keystore],
        outputs = [apks, out_dir],
        tools = [bundletool],
        mnemonic = "DxApkSet",
        progress_message = "Building APK set %{label}",
        use_default_shell_env = True,
    )
    return [
        DefaultInfo(files = depset([out_dir])),
        OutputGroupInfo(apks = depset([apks])),
    ]

android_apk_set = rule(
    implementation = _apk_set_impl,
    doc = "Builds the debug-signed split APKs that one App Bundle installs on one device spec.",
    attrs = {
        "aab": attr.label(allow_files = True, mandatory = True, doc = "Target that builds the App Bundle."),
        "device_spec": attr.label(allow_files = True, mandatory = True, doc = "bundletool device spec JSON."),
        "_build_apk_set": attr.label(
            default = Label("//android/packaging:build_apk_set.sh"),
            allow_single_file = True,
            executable = True,
            cfg = "exec",
        ),
        "_bundletool": attr.label(default = _BUNDLETOOL, executable = True, cfg = "exec"),
        "_keystore": attr.label(default = _DEBUG_KEYSTORE, allow_single_file = True),
    },
)
