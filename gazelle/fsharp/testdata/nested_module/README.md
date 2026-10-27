# Nested F# Module Generation

Verifies a nested `module Utils =` inside a namespace file. It does not
count as a second package. `App.fs` calls `Utils.twice`, so `srcs` list
`Tools.fs` before `App.fs`.
