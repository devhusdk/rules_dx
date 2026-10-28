"""Analysis check that OCaml wrappers expose their typed-tree files."""

load("//ocaml/rules:providers.bzl", "OCamlCmtInfo")

def _impl(ctx):
    entries = ctx.attr.target[OCamlCmtInfo].owned
    got = sorted([
        (e.source.basename if e.source else "", e.cmt.basename.lower() if e.cmt else "", e.signature.basename if e.signature else "", e.cmti.basename.lower() if e.cmti else "")
        for e in entries
    ])
    want = sorted([tuple(w.split(":")) for w in ctx.attr.expect])
    if got != want:
        fail("OCamlCmtInfo of %s is %s, want %s" % (ctx.attr.target.label, got, want))
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "ok\n")
    return [DefaultInfo(files = depset([out]))]

cmt_check = rule(
    implementation = _impl,
    doc = "Fails analysis unless the target's OCamlCmtInfo lists exactly the expected source:cmt:signature:cmti entries.",
    attrs = {
        "target": attr.label(mandatory = True, providers = [OCamlCmtInfo]),
        "expect": attr.string_list(mandatory = True),
    },
)
