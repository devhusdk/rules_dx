"""License ids accepted through dx_licenses and the gate every restricted download reads."""

visibility(["//android/...", "//apple/...", "//cc/...", "//core/...", "//cuda/...", "//msix/..."])

APPLE_XCODE = "apple-xcode-license"
ANDROID_SDK = "android-sdk-license"
CUDA_EULA = "cuda-eula"
WINDOWS_SDK_BUILDTOOLS = "windows-sdk-buildtools"
VISUAL_STUDIO_BUILD_TOOLS = "visual-studio-build-tools"

# The terms of each license and the sha256 of their text. An acceptance that pins an
# older hash fails until the root module re-accepts, so a vendor's new terms never
# carry over silently. HTML pages are hashed after dropping script, style and markup
# and collapsing whitespace; other files as served. tools/licenses/terms_check.py
# computes the hash and parses the `<ID>: struct(url = ..., terms_sha256 = ...)`
# entries below, and the scheduled terms-check workflow runs it.
LICENSES = {
    APPLE_XCODE: struct(url = "https://www.apple.com/legal/sla/docs/xcode.pdf", terms_sha256 = "5e6bd16bcba3dd591bde1c57f7c6f19f08a194c84cdc54485c4660f90e2a26ec"),
    ANDROID_SDK: struct(url = "https://developer.android.com/studio/terms", terms_sha256 = "99fc9fca01e2afc2b8fd4c11f4602c1213059a5249b6afcadf6eb37b09c1f101"),
    CUDA_EULA: struct(url = "https://docs.nvidia.com/cuda/eula/index.html", terms_sha256 = "d6de69eacd5644f0f84d2289a54d07d29017377d37ab28d0227b3df3509b1a60"),
    WINDOWS_SDK_BUILDTOOLS: struct(url = "https://aka.ms/WinSDKLicenseURL", terms_sha256 = "dd07eb178e00c6bba4148457fc00ff77cd4887eb521d504186fe59c9ec8bbe62"),
    VISUAL_STUDIO_BUILD_TOOLS: struct(url = "https://visualstudio.microsoft.com/license-terms/", terms_sha256 = "9284a30da91bfdf5c56a681a6c007a0376cec75f069596dc6d28c3218c9e23d3"),
}

LICENSE_IDS = [APPLE_XCODE, ANDROID_SDK, CUDA_EULA, WINDOWS_SDK_BUILDTOOLS, VISUAL_STUDIO_BUILD_TOOLS]

# A JSON object from each accepted id to the terms_sha256 it was accepted at. Readers
# that only test `id in accepted` work on it unchanged.
ACCEPTED_LABEL = "@dx_licenses//:accepted.json"

def unknown_ids(ids):
    """Returns the ids that are not a known license id."""
    return [name for name in ids if name not in LICENSE_IDS]

def accept_line(ids):
    """Returns the dx_licenses.accept tag that accepts the current terms of ids."""
    quoted = ", ".join(['"' + name + '"' for name in ids])
    pins = ", ".join(['"' + name + '": "' + LICENSES[name].terms_sha256 + '"' for name in ids])
    return "dx_licenses.accept(ids = [" + quoted + "], terms_sha256 = {" + pins + "})"

def terms_errors(tags):
    """Returns one error per pinned terms hash in tags that is not the current one.

    Args:
        tags: the root module's accept tags, each with ids and terms_sha256.
    """
    errors = []
    for tag in tags:
        for name, pinned in sorted(tag.terms_sha256.items()):
            if name not in tag.ids:
                errors.append("dx_licenses: terms_sha256 names " + name + ", which the same tag's ids do not list")
            elif name in LICENSES and pinned != LICENSES[name].terms_sha256:
                errors.append("dx_licenses: terms of " + name + " changed: " + LICENSES[name].url + "; re-accept with " + accept_line([name]))
    return errors

def accepted_terms(ids):
    """Returns the accepted.json object: each id mapped to the terms hash it was accepted at."""
    return {name: LICENSES[name].terms_sha256 for name in sorted(ids)}

def accepted_ids(mctx):
    """Returns the ids the root module accepted, read from the dx_licenses repo."""
    return sorted(json.decode(mctx.read(Label(ACCEPTED_LABEL))).keys())

def unaccepted(prefix, what, url, name):
    """Returns the error shown when the license is not accepted."""
    return prefix + ": the " + what + " (" + url + ") is not accepted; add dx_licenses.accept(ids = [\"" + name + "\"]) to the root MODULE.bazel"

def accepted(mctx, known):
    """Returns whether the root module accepted the known license."""
    return known in accepted_ids(mctx)
