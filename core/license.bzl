"""License ids accepted through dx_licenses and the gate every restricted download reads."""

APPLE_XCODE = "apple-xcode-license"
ANDROID_SDK = "android-sdk-license"
CUDA_EULA = "cuda-eula"
WINDOWS_SDK_BUILDTOOLS = "windows-sdk-buildtools"
VISUAL_STUDIO_BUILD_TOOLS = "visual-studio-build-tools"

LICENSE_IDS = [APPLE_XCODE, ANDROID_SDK, CUDA_EULA, WINDOWS_SDK_BUILDTOOLS, VISUAL_STUDIO_BUILD_TOOLS]

ACCEPTED_LABEL = "@dx_licenses//:accepted.json"

def unknown_ids(ids):
    """Returns the ids that are not a known license id."""
    return [name for name in ids if name not in LICENSE_IDS]

def accepted_ids(mctx):
    """Returns the ids the root module accepted, read from the dx_licenses repo."""
    return json.decode(mctx.read(Label(ACCEPTED_LABEL)))

def unaccepted(prefix, what, url, name):
    """Returns the error shown when the license is not accepted."""
    return prefix + ": the " + what + " (" + url + ") is not accepted; add dx_licenses.accept(ids = [\"" + name + "\"]) to the root MODULE.bazel"

def accepted(mctx, known):
    """Returns whether the root module accepted the known license."""
    return known in accepted_ids(mctx)
