"""Directory layout of an MSVC toolset and Windows SDK under one root."""

def msvc_layout(msvc_version, sdk_version):
    """Return paths relative to the layout root.

    Args:
        msvc_version: MSVC toolset version, such as "14.44.35207".
        sdk_version: Windows SDK version, such as "10.0.26100.0".

    Returns:
        A struct of tool paths and include, lib and bin directory lists.
    """
    vc = "VC/Tools/MSVC/" + msvc_version
    sdk_include = "Windows Kits/10/Include/" + sdk_version
    sdk_lib = "Windows Kits/10/Lib/" + sdk_version
    bin_dir = vc + "/bin/Hostx64/x64"
    return struct(
        cl = bin_dir + "/cl.exe",
        link = bin_dir + "/link.exe",
        lib = bin_dir + "/lib.exe",
        ml64 = bin_dir + "/ml64.exe",
        include_dirs = [vc + "/include"] + [sdk_include + "/" + d for d in ["ucrt", "shared", "um", "winrt"]],
        lib_dirs = [vc + "/lib/x64"] + [sdk_lib + "/" + d + "/x64" for d in ["ucrt", "um"]],
        bin_dirs = [bin_dir, "Windows Kits/10/bin/" + sdk_version + "/x64"],
    )
