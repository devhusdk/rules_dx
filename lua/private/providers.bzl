"""Providers of the Lua core rules."""

LuaInfo = provider(
    doc = "Lua modules a target makes loadable through require.",
    fields = {
        "modules": "Depset of (module name, File) tuples, this target and its deps.",
        "runfiles": "Runfiles the modules need at run time: sources, data and the runfiles of deps.",
    },
)

LuaEntryInfo = provider(
    doc = "Entry data of one Lua executable, for a wrapper that writes its own entry file.",
    fields = {
        "main": "Canonical rlocation of the main chunk.",
        "source_repo": "Canonical name of the repository that owns the executable, empty for the main repository.",
        "modules": "Depset of (module name, File) tuples the executable can require.",
    },
)

LUA_ENTRY_SUFFIX = ".luaentry"

LUA_COVERAGE_MODULE = "dx_coverage"

LUA_ENTRY_FORMAT = """\
One <name>.luaentry file sits next to each Lua executable. Lines end in \\n.
Line 1 is the canonical rlocation of the main chunk.
Line 2 is the canonical name of the repository that owns the executable, empty for the main repository.
Each further line is <module>\\t<canonical rlocation>, sorted by module name.
When a module named dx_coverage is listed, the launcher requires it before main.
That is the coverage hook: a wrapper adds it by writing its own entry file from LuaEntryInfo.
"""
