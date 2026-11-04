"""Directory layout of an MSVC toolset and Windows SDK under one root."""

# (host, target) pairs with a complete tool closure in the VS manifest.
SUPPORTED_CELLS = [("x64", "x64"), ("x64", "arm64"), ("arm64", "arm64")]

def msvc_layout(msvc_version, sdk_version, host = "x64", target = "x64"):
    """Return paths relative to the layout root.

    Args:
        msvc_version: MSVC toolset version, such as "14.44.35207".
        sdk_version: Windows SDK version, such as "10.0.26100.0".
        host: Architecture the tools run on, "x64" or "arm64".
        target: Architecture the tools build for, "x64" or "arm64".

    Returns:
        A struct of tool paths and include, lib and bin directory lists.
    """
    vc = "VC/Tools/MSVC/" + msvc_version
    sdk_include = "Windows Kits/10/Include/" + sdk_version
    sdk_lib = "Windows Kits/10/Lib/" + sdk_version
    bin_dir = vc + "/bin/Host{}/{}".format(host, target)
    bin_dirs = [bin_dir]
    if host != target:
        bin_dirs.append(vc + "/bin/Host{}/{}".format(host, host))
    bin_dirs.append("Windows Kits/10/bin/{}/{}".format(sdk_version, host))
    return struct(
        cl = bin_dir + "/cl.exe",
        link = bin_dir + "/link.exe",
        lib = bin_dir + "/lib.exe",
        ml = bin_dir + "/ml64.exe" if target == "x64" else None,
        include_dirs = [vc + "/include"] + [sdk_include + "/" + d for d in ["ucrt", "shared", "um", "winrt"]],
        lib_dirs = [vc + "/lib/" + target] + [sdk_lib + "/" + d + "/" + target for d in ["ucrt", "um"]],
        bin_dirs = bin_dirs,
    )
