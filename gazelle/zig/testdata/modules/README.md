# Zig Module Generation

One `zig_library` per directory. The root file is `<dir>.zig`, `root.zig` or
`lib.zig`. Test files and `build.zig` stay out. `@import` names resolve to
the library with that name.
`.zon` imports and `@embedFile` files go to `extra_srcs`.
