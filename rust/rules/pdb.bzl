"""Link flags that keep an MSVC PDB free of the build directory."""

# Bazel turns $$ into $, then process_wrapper replaces ${pwd} with the
# execroot, so the linker maps it to a fixed drive path in the PDB (#8410).
REPRODUCIBLE_PDB_LINK_FLAGS = [
    "-Clink-arg=/pdbsourcepath:X:\\dx",
    "-Clink-arg=/pathmap:$${pwd}=X:\\dx",
]
