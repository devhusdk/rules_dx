load("//bundle:validate.bzl", "bundle_id_errors")

def macos_bundle_errors(kind, label, platform_type, bundle_extension, bundle_id):
    """Returns the errors that keep one bundle from macOS signing."""
    if platform_type != "macos" or bundle_extension != ".app":
        return ["{}: '{}' is not a macOS application".format(kind, label)]
    return bundle_id_errors(kind, bundle_id)
