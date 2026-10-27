# Module-Per-File F# Generation

Verifies files that each declare a top-level `module`. `module Helper`
and `module Demo` form one package. `Demo.fs` calls `Helper.suffix`, so
`srcs` list `Helper.fs` first.
