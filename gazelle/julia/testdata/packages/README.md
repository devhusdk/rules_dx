# Julia Packages

Each `Project.toml` with a `name` and a `src/<name>.jl` gets one
`julia_library` named after the package, from `src/**/*.jl`, and a
`julia_test` when `test/runtests.jl` exists.

A dependency resolves to the `julia_library` with its name. Standard
libraries need nothing. Any other package needs one `# gazelle:resolve julia`
line. A `Project.toml` without a name is an environment and gets no rules.
A nested `Project.toml` starts another package. A rule whose sources moved
out of `src/` and `test/` is never swept, so a handwritten `julia_binary`
stays.
