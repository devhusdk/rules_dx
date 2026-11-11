"""The gazelle arguments of `//dx:generate`."""

# dx's own config tree and installed packages are never generated.
GENERATE_EXCLUDES = [
    "-exclude=.dx",
    "-exclude=**/node_modules",
]
