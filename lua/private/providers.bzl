"""Providers of the Lua core rules."""

LuaInfo = provider(
    doc = "Lua modules a target makes loadable through require.",
    fields = {
        "modules": "Depset of (module name, File) tuples, this target and its deps.",
        "runfiles": "Runfiles the modules need at run time: sources, data and the runfiles of deps.",
    },
)

LUA_ENTRY_SUFFIX = ".luaentry"

LUA_ENTRY_FORMAT = """\
One <name>.luaentry file sits next to each Lua executable. Lines end in \\n.
Line 1 is the canonical rlocation of the main chunk.
Line 2 is the canonical name of the repository that owns the executable, empty for the main repository.
Each further line is <module>\\t<canonical rlocation>, sorted by module name.
"""
