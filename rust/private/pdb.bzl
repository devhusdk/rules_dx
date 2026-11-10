"""Link flags that keep an MSVC PDB free of the build directory."""

REPRODUCIBLE_PDB_LINK_FLAGS = [
    "-Clink-arg=/pdbsourcepath:X:\\dx",
    "-Clink-arg=/pdbaltpath:%_PDB%",
]
