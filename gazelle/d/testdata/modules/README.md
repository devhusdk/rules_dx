# D Module Generation

One `d_library` per directory. A module declaration must match the file path,
and the path above the module name becomes the package-relative `imports`
entry. A file without a declaration is the module named after the file.
Imports resolve to the library that declares exactly that module.
`import("file")` files go to `string_srcs`. DTrace scripts and Make
dependency files that end in `.d` stay out.
