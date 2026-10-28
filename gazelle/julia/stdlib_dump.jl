# Prints the sorted "Name uuid" lines of the standard libraries that ship with
# the running Julia. With --check <file>, diffs that list against <file>
# instead and exits non-zero on drift.
using TOML

function stdlibs()
    lines = String[]
    for dir in readdir(Sys.STDLIB)
        project = joinpath(Sys.STDLIB, dir, "Project.toml")
        isfile(project) || continue
        info = TOML.parsefile(project)
        haskey(info, "uuid") || continue
        push!(lines, string(get(info, "name", dir), " ", info["uuid"]))
    end
    return sort!(lines)
end

function main(args)
    want = stdlibs()
    if length(args) == 2 && args[1] == "--check"
        have = filter(!isempty, split(read(args[2], String), "\n"))
        missing_lines = setdiff(want, have)
        extra_lines = setdiff(have, want)
        isempty(missing_lines) && isempty(extra_lines) && return 0
        println(stderr, "stdlib.txt drifted from Julia $(VERSION); regenerate with: bazel run //gazelle/julia:stdlib_dump > gazelle/julia/stdlib.txt")
        foreach(l -> println(stderr, "missing: ", l), missing_lines)
        foreach(l -> println(stderr, "extra: ", l), extra_lines)
        out = get(ENV, "TEST_UNDECLARED_OUTPUTS_DIR", "")
        isempty(out) || write(joinpath(out, "stdlib.txt"), join(want, "\n") * "\n")
        return 1
    end
    foreach(println, want)
    return 0
end

exit(main(ARGS))
