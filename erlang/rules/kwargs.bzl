load("//erlang/toolchain:cells.bzl", "otp_compatible_with")
load("//erlang/toolchain:pins.bzl", "CELLS", "OTP_UNPORTED_CELLS")
load("//libs/starlark:wrapper.bzl", "dx_language_kwargs")

ERLANG_CELLS = sorted([cell for cell in CELLS.keys() if cell not in OTP_UNPORTED_CELLS])

ERLANG_COMPATIBLE_WITH = otp_compatible_with(ERLANG_CELLS)

def erlang_app_forward_kwargs(testonly, nifs, tags, kwargs):
    """Returns the kwargs the public erlang_app forwarder keeps."""
    out = {"testonly": True} if testonly else {}
    if nifs:
        out["nifs"] = nifs
    if tags != None:
        out["tags"] = tags
    if kwargs.get("target_compatible_with", None) != None:
        out["target_compatible_with"] = kwargs["target_compatible_with"]
    return out

def erlang_kwargs(kwargs):
    """Returns kwargs with the Erlang platform guard appended."""
    return dx_language_kwargs(kwargs, [], ERLANG_COMPATIBLE_WITH)
