"""Paths of an MSVC toolset and Windows SDK in the windows_support layout."""

# (host, target) pairs with a complete tool closure in the VS manifest.
SUPPORTED_CELLS = [("x64", "x64"), ("x64", "arm64"), ("arm64", "arm64")]

def msvc_layout(msvc_version, sdk_version, host = "x64", target = "x64", vc_root = "", sdk_root = ""):
    """Return tool and directory paths of the MSVC toolset and the Windows SDK."""
    vc = vc_root + "sysroot/Contents/VC/Tools/MSVC/" + msvc_version
    sdk_include = sdk_root + "sysroot/base/c/Include/" + sdk_version
    bin_dir = vc + "/bin/Host{}/{}".format(host, target)
    bin_dirs = [bin_dir]
    if host != target:
        bin_dirs.append(vc + "/bin/Host{}/{}".format(host, host))
    bin_dirs.append(sdk_root + "sysroot/base/c/bin/{}/{}".format(sdk_version, host))
    return struct(
        cl = bin_dir + "/cl.exe",
        link = bin_dir + "/link.exe",
        lib = bin_dir + "/lib.exe",
        ml = bin_dir + "/ml64.exe" if target == "x64" else None,
        include_dirs = [vc + "/include"] + [sdk_include + "/" + d for d in ["ucrt", "shared", "um", "winrt"]],
        lib_dirs = [vc + "/lib/" + target] + ["{}sysroot/{}/c/{}/{}".format(sdk_root, target, d, target) for d in ["ucrt", "um"]],
        bin_dirs = bin_dirs,
    )
