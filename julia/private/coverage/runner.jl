# Runs the Julia test under --code-coverage=user, then writes LCOV for the
# first-party sources listed in the config to $COVERAGE_OUTPUT_FILE.
using CoverageTools

function read_config(path)
    config = Dict("workspace" => "", "upstream" => "", "source" => String[])
    for line in eachline(path)
        isempty(line) && continue
        key, value = split(line, '\t'; limit = 2)
        if key == "source"
            push!(config["source"], value)
        else
            config[key] = value
        end
    end
    return config
end

function manifest_entry(line)
    startswith(line, ' ') || return split(line, ' '; limit = 2)
    key, value = split(line[2:end], ' '; limit = 2)
    unescape(text) = replace(text, "\\s" => " ", "\\n" => "\n", "\\b" => "\\")
    return [unescape(key), unescape(value)]
end

function manifest_path()
    path = get(ENV, "RUNFILES_MANIFEST_FILE", "")
    isempty(path) || return path
    dir = get(ENV, "RUNFILES_DIR", "")
    isempty(dir) && return ""
    return joinpath(dir, "MANIFEST")
end

function rlocation(key)
    dir = get(ENV, "RUNFILES_DIR", "")
    if !isempty(dir)
        path = joinpath(dir, key)
        ispath(path) && return path
    end
    manifest = manifest_path()
    isfile(manifest) || return nothing
    for line in eachline(manifest)
        entry = manifest_entry(rstrip(line, '\r'))
        length(entry) == 2 && entry[1] == key && return entry[2]
    end
    return nothing
end

function lcov_records(workspace, sources)
    records = FileCoverage[]
    for source in sources
        file = rlocation(workspace * "/" * source)
        file === nothing && continue
        isfile(file) || continue
        record = process_file(file)
        push!(records, FileCoverage(source, record.source, record.coverage))
    end
    return records
end

function main()
    config_key = ENV["DX_JULIA_COVERAGE_CONFIG"]
    config_path = rlocation(config_key)
    config_path === nothing && error("not in runfiles: $config_key")
    config = read_config(config_path)
    upstream = rlocation(config["upstream"])
    upstream === nothing && error("not in runfiles: $(config["upstream"])")
    env = copy(ENV)
    env["RULES_JULIA_COMPILED_MODULES"] = "no"
    process = run(ignorestatus(Cmd(`$upstream $ARGS`; env = env)))
    records = lcov_records(config["workspace"], config["source"])
    LCOV.writefile(ENV["COVERAGE_OUTPUT_FILE"], records)
    exit(process.exitcode)
end

main()
