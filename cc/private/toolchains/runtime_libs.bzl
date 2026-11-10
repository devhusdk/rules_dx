"""Selects the libstdc++ and libgcc_s shared objects from a GCC archive."""

_NAMES = ["libgcc_s.so.1", "libstdc++.so.6"]

def _gcc_runtime_libs_impl(ctx):
    outs = []
    for name in _NAMES:
        src = [f for f in ctx.files.srcs if f.path.endswith("/lib64/" + name)]
        if len(src) != 1:
            fail("expected exactly one lib64/{} in srcs, found {}".format(name, len(src)))
        out = ctx.actions.declare_file("{}/{}".format(ctx.label.name, name))
        ctx.actions.symlink(output = out, target_file = src[0])
        outs.append(out)
    return [DefaultInfo(files = depset(outs))]

gcc_runtime_libs = rule(
    implementation = _gcc_runtime_libs_impl,
    attrs = {"srcs": attr.label_list(allow_files = True)},
)
