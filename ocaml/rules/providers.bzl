"""Providers the OCaml wrappers export for quality tools. No external loads."""

OCamlCmtInfo = provider(
    doc = "The typed-tree files one OCaml target compiled, for tools that read .cmt and .cmti.",
    fields = {
        "owned": "List of structs (source, cmt, signature, cmti): the hand-written .ml (or None), its .cmt (or None), the hand-written .mli (or None) and its .cmti (or None).",
    },
)

def cmt_entries(srcs, cmts, cmtis):
    """Returns the OCamlCmtInfo.owned entries for one target.

    Args:
      srcs: The target's hand-written .ml and .mli files.
      cmts: The .cmt files the compile produced.
      cmtis: The .cmti files the compile produced; only listed with a hand-written .mli in srcs, so a module that takes its signature from an ocaml_signature target does not repeat that target's .cmti.
    """
    impl = [f for f in srcs if f.basename.endswith(".ml")]
    sig = [f for f in srcs if f.basename.endswith(".mli")]
    cmt = cmts[0] if len(cmts) == 1 else None
    cmti = cmtis[0] if len(cmtis) == 1 and sig else None
    if cmt == None and cmti == None:
        return []
    return [struct(
        source = impl[0] if impl and cmt else None,
        cmt = cmt,
        signature = sig[0] if sig and cmti else None,
        cmti = cmti,
    )]

def bin_annot_opts(opts):
    """Returns opts with -bin-annot added unless the target sets -no-bin-annot or already has -bin-annot.

    Args:
      opts: The target's opts, a list or a select, or None.
    """
    if opts == None:
        return ["-bin-annot"]
    if type(opts) == "list":
        if "-no-bin-annot" in opts or "-bin-annot" in opts:
            return opts
        return opts + ["-bin-annot"]
    return opts + ["-bin-annot"]
