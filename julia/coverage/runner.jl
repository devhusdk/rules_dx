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

function lcov_records(runfiles, workspace, sources)
    records = FileCoverage[]
    for source in sources
        file = joinpath(runfiles, workspace, source)
        isfile(file) || continue
        record = process_file(file)
        push!(records, FileCoverage(source, record.source, record.coverage))
    end
    return records
end

function main()
    runfiles = ENV["RUNFILES_DIR"]
    config = read_config(joinpath(runfiles, ENV["DX_JULIA_COVERAGE_CONFIG"]))
    upstream = joinpath(runfiles, config["upstream"])
    env = copy(ENV)
    env["RULES_JULIA_COMPILED_MODULES"] = "no"
    process = run(ignorestatus(Cmd(`$upstream $ARGS`; env = env)))
    records = lcov_records(runfiles, config["workspace"], config["source"])
    LCOV.writefile(ENV["COVERAGE_OUTPUT_FILE"], records)
    exit(process.exitcode)
end

main()
