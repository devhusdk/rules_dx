"""Module extension for the host `swift` command."""

# Libraries swift-package loads that a plain host may lack.
_HOST_LIBS = ["libxml2.so.2", "libicuuc.so.70", "libicudata.so.70"]

def _swift_host_impl(repository_ctx):
    if repository_ctx.os.name.startswith("mac"):
        script = 'exec xcrun swift "$@"'
    else:
        arch = "aarch64" if repository_ctx.os.arch in ["aarch64", "arm64"] else "x86_64"
        swift = repository_ctx.path(Label("@dx_swift_linux_{}//:usr/bin/swift".format(arch)))
        sysroot = repository_ctx.path(Label("@dx_swift_sysroot_linux_{}//:REPO.bazel".format(arch))).dirname
        host_libs = sysroot.get_child("host_libs")
        for lib in _HOST_LIBS:
            repository_ctx.symlink(sysroot.get_child("usr/lib/{}-linux-gnu/{}".format(arch, lib)), "libs/" + lib)
        libs = repository_ctx.path("libs")
        script = 'LD_LIBRARY_PATH="{libs}:{host_libs}${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}" exec "{swift}" "$@"'.format(
            libs = libs,
            host_libs = host_libs,
            swift = swift,
        )
    repository_ctx.file("swift", "#!/bin/sh\n{}\n".format(script), executable = True)
    repository_ctx.file("BUILD.bazel", 'exports_files(["swift"], visibility = ["//visibility:public"])\n')

_swift_host = repository_rule(
    implementation = _swift_host_impl,
    doc = "A `swift` command that runs the managed toolchain on Linux and the Xcode one on macOS.",
)

def _swift_host_ext_impl(module_ctx):
    _swift_host(name = "dx_swift_host")
    return module_ctx.extension_metadata(reproducible = True)

swift_host = module_extension(implementation = _swift_host_ext_impl)
