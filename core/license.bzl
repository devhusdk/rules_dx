"""License ids accepted through dx_licenses and the gate every restricted download reads."""

visibility(["//android/...", "//apple/...", "//cc/...", "//core/...", "//cuda/...", "//msix/..."])

APPLE_XCODE = "apple-xcode-license"
ANDROID_SDK = "android-sdk-license"
CUDA_EULA = "cuda-eula"
WINDOWS_SDK_BUILDTOOLS = "windows-sdk-buildtools"
VISUAL_STUDIO_BUILD_TOOLS = "visual-studio-build-tools"

TERMS_LABEL = "//core:licenses/pins.json"

LICENSE_IDS = [APPLE_XCODE, ANDROID_SDK, CUDA_EULA, WINDOWS_SDK_BUILDTOOLS, VISUAL_STUDIO_BUILD_TOOLS]

# A JSON object from each accepted id to the terms_sha256 it was accepted at. Readers
# that only test `id in accepted` work on it unchanged.
ACCEPTED_LABEL = "@dx_licenses//:accepted.json"

def unknown_ids(ids):
    """Returns the ids that are not a known license id."""
    return [name for name in ids if name not in LICENSE_IDS]

def accept_line(terms, ids):
    """Returns the dx_licenses.accept tag that accepts the current terms of ids; terms is the decoded TERMS_LABEL."""
    quoted = ", ".join(['"' + name + '"' for name in ids])
    pins = ", ".join(['"' + name + '": "' + terms[name]["terms_sha256"] + '"' for name in ids])
    return "dx_licenses.accept(ids = [" + quoted + "], terms_sha256 = {" + pins + "})"

def terms_errors(terms, tags):
    """Returns one error per pinned terms hash in the root module's accept tags that is not the current one in terms."""
    errors = []
    for tag in tags:
        for name, pinned in sorted(tag.terms_sha256.items()):
            if name not in tag.ids:
                errors.append("dx_licenses: terms_sha256 names " + name + ", which the same tag's ids do not list")
            elif name in terms and pinned != terms[name]["terms_sha256"]:
                errors.append("dx_licenses: terms of " + name + " changed: " + terms[name]["url"] + "; re-accept with " + accept_line(terms, [name]))
    return errors

def accepted_terms(terms, ids):
    """Returns the accepted.json object: each id mapped to the terms hash in terms it was accepted at."""
    return {name: terms[name]["terms_sha256"] for name in sorted(ids)}

def accepted_ids(mctx):
    """Returns the ids the root module accepted, read from the dx_licenses repo."""
    return sorted(json.decode(mctx.read(Label(ACCEPTED_LABEL))).keys())

def unaccepted(prefix, what, url, name):
    """Returns the error shown when the license is not accepted."""
    return prefix + ": the " + what + " (" + url + ") is not accepted; add dx_licenses.accept(ids = [\"" + name + "\"]) to the root MODULE.bazel"

def accepted(mctx, known):
    """Returns whether the root module accepted the known license."""
    return known in accepted_ids(mctx)
