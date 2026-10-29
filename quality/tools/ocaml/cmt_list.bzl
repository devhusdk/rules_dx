"""Writes the -cmt-list file zanuda reads from an OCaml target's OCamlCmtInfo."""

load("//ocaml/rules:providers.bzl", "OCamlCmtInfo")

def _line(kind, source, cmt):
    return "\t".join([kind, source.short_path, cmt.short_path])

def _impl(ctx):
    lines = []
    files = []
    for entry in ctx.attr.target[OCamlCmtInfo].owned:
        if entry.cmt:
            lines.append(_line("impl", entry.source, entry.cmt))
            files += [entry.source, entry.cmt]
        if entry.cmti:
            lines.append(_line("intf", entry.signature, entry.cmti))
            files += [entry.signature, entry.cmti]
    if not lines:
        fail("%s has no .cmt files; was it built with -no-bin-annot?" % ctx.attr.target.label)
    out = ctx.actions.declare_file(ctx.label.name + ".list")
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(files = depset([out]), runfiles = ctx.runfiles(files = [out] + files))]

ocaml_cmt_list = rule(
    implementation = _impl,
    doc = "Lists the sources and .cmt/.cmti files of one OCaml target as 'kind<TAB>source<TAB>cmt' lines, paths relative to the runfiles root.",
    attrs = {
        "target": attr.label(mandatory = True, providers = [OCamlCmtInfo]),
    },
)
