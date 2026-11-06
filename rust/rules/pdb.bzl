"""Link flags that keep an MSVC PDB free of the build directory."""

# rustc links Windows with rust-lld, which writes relative paths into the PDB
# against this fixed root. rules_rust already remaps the execroot (#8410).
REPRODUCIBLE_PDB_LINK_FLAGS = [
    "-Clink-arg=/pdbsourcepath:X:\\dx",
]
