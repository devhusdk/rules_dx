module Report

using Archive: compress, decompress
using Stats: histogram, mean, quantile, variance

export parse_readings, run_report

"""
    parse_readings(lines) -> Vector{Float64}

Read `sensor,value` rows after the header. Throws `ArgumentError` naming the
line for a malformed row.
"""
function parse_readings(lines::AbstractVector{<:AbstractString})
    isempty(lines) && throw(ArgumentError("no header"))
    values = Float64[]
    for (i, line) in enumerate(lines[2:end])
        fields = split(line, ',')
        number = length(fields) == 2 ? tryparse(Float64, fields[2]) : nothing
        number === nothing && throw(ArgumentError("line $(i + 1): malformed row"))
        push!(values, number)
    end
    return values
end

fmt(x, digits) = string(round(x; digits = digits))

"""
    run_report(path, digits; io, err) -> Int

Print a summary of the readings in `path` and return the exit code.
"""
function run_report(
    path::AbstractString,
    digits::Integer;
    io::IO = stdout,
    err::IO = stderr,
)
    if !isfile(path)
        println(err, "error: cannot read $(path)")
        return 2
    end
    text = read(path, String)
    values = try
        parse_readings(split(chomp(text), '\n'))
    catch e
        e isa ArgumentError || rethrow()
        println(err, "error: $(e.msg)")
        return 1
    end
    if isempty(values)
        println(err, "error: no readings")
        return 1
    end
    println(io, "readings: ", length(values))
    println(io, "mean: ", fmt(mean(values), digits))
    println(io, "variance: ", fmt(variance(values), digits))
    println(io, "median: ", fmt(quantile(values, 0.5), digits))
    for (bucket, count) in histogram(values, 5)
        println(io, "bucket ", fmt(bucket, 0), ": ", count)
    end
    raw = Vector{UInt8}(text)
    ok = decompress(compress(raw), length(raw)) == raw
    println(io, "archive: ", ok ? "ok" : "mismatch")
    return ok ? 0 : 1
end

end
