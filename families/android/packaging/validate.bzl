load("//platforms:defs.bzl", "api_floor")
load(":repos.bzl", "abi_errors", "app_id_errors")

def rust_android_apk_errors(apk_name, app_id, abis, api_level, version_code):
    """Returns one error string per rejected application package selection."""
    errors = []
    if apk_name == "":
        errors.append("rust_android_apk: name must not be empty")
    errors.extend(app_id_errors(app_id))
    errors.extend(abi_errors(abis))
    if len(abis) == 0:
        errors.append("rust_android_apk: platforms must name at least one ABI")
    if api_level != 0 and api_level < api_floor():
        errors.append("rust_android_apk: api_level " + str(api_level) + " is below " + str(api_floor()))
    if version_code <= 0:
        errors.append("rust_android_apk: version_code " + str(version_code) + " must be positive")
    return errors

def android_apk_manifest_errors(app_id, api_level, version_code, activity):
    """Returns one error string per rejected generated-manifest selection."""
    errors = list(app_id_errors(app_id))
    if api_level != 0 and api_level < api_floor():
        errors.append("rust_android_apk: api_level " + str(api_level) + " is below " + str(api_floor()))
    if version_code <= 0:
        errors.append("rust_android_apk: version_code " + str(version_code) + " must be positive")
    if activity == "":
        errors.append("rust_android_apk: activity must not be empty")
    return errors

KNOWN_SCHEMES = ["v1", "v2", "v3", "v4"]

def android_release_sign_errors(schemes, min_sdk):
    """Returns one error string per rejected release signing selection."""
    errors = []
    if len(schemes) == 0:
        errors.append("android_sign: schemes must name at least one signature scheme")
    for scheme in schemes:
        if scheme not in KNOWN_SCHEMES:
            errors.append("android_sign: scheme '" + scheme + "' is unknown: want " + ", ".join(KNOWN_SCHEMES))
    if len(schemes) != len(dict([(scheme, True) for scheme in schemes])):
        errors.append("android_sign: schemes holds duplicates")
    if "v4" in schemes and "v2" not in schemes and "v3" not in schemes:
        errors.append("android_sign: scheme v4 needs v2 or v3")
    if min_sdk != 0 and min_sdk < api_floor():
        errors.append("android_sign: min_sdk " + str(min_sdk) + " is below " + str(api_floor()))
    return errors

def android_aab_check_errors(abis, resources):
    """Returns one error string per rejected App Bundle check selection."""
    errors = ["android_check: " + e.removeprefix("rust_android_apk: ") for e in abi_errors(abis)]
    for resource in resources:
        parts = resource.split("/")
        if len(parts) != 2 or parts[0] == "" or parts[1] == "":
            errors.append("android_check: resource '" + resource + "' must be type/name")
    return errors

def android_apk_set_errors(label, aab_count, spec_count):
    """Returns one error string per rejected APK set selection."""
    errors = []
    if aab_count != 1:
        errors.append("android_apk_set: '" + label + "' aab built " + str(aab_count) + " aab files, want exactly one")
    if spec_count != 1:
        errors.append("android_apk_set: '" + label + "' device_spec names " + str(spec_count) + " json files, want exactly one")
    return errors
