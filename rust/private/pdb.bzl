"""Link flags that keep an MSVC PDB free of the build directory."""

# rustc links Windows with rust-lld, which writes relative paths into the PDB
# against this fixed root. rules_rust already remaps the execroot (#8410).
# /pdbaltpath keeps only the PDB basename in the exe, not its absolute path.
REPRODUCIBLE_PDB_LINK_FLAGS = [
    "-Clink-arg=/pdbsourcepath:X:\\dx",
    "-Clink-arg=/pdbaltpath:%_PDB%",
]
