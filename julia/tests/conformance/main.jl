using Greet: greet

function rlocation(path)
    dir = get(ENV, "RUNFILES_DIR", "")
    isempty(dir) || return joinpath(dir, path)
    key = "$path "
    for line in eachline(ENV["RUNFILES_MANIFEST_FILE"])
        startswith(line, key) && return rstrip(chopprefix(line, key), '\r')
    end
    return error("not in runfiles manifest: $path")
end

if length(ARGS) == 1
    print(read(rlocation(ARGS[1]), String))
else
    println(greet("world"))
end
