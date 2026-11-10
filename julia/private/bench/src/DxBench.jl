module DxBench

export run_benchmarks

json_string(s) = string('"', replace(s, "\\" => "\\\\", "\"" => "\\\""), '"')

function run_benchmarks(cases; seconds = 0.5)
    results = String[]
    for (name, fn) in cases
        fn()
        times = Float64[]
        deadline = time_ns() + round(UInt64, seconds * 1.0e9)
        while length(times) < 20 || (length(times) < 1000 && time_ns() < deadline)
            start = time_ns()
            fn()
            push!(times, (time_ns() - start) / 1.0e9)
        end
        mean = sum(times) / length(times)
        variance = length(times) > 1 ? sum((t - mean)^2 for t in times) / (length(times) - 1) : 0.0
        push!(
            results,
            string(
                "{\"name\":", json_string(string(name)), ",\"unit\":\"s\",\"mean\":", mean,
                ",\"stddev\":", sqrt(variance), ",\"iterations\":", length(times), "}",
            ),
        )
    end
    dir = get(ENV, "TEST_UNDECLARED_OUTPUTS_DIR", nothing)
    dir === nothing && error("TEST_UNDECLARED_OUTPUTS_DIR is not set")
    open(joinpath(dir, "dx-bench.json"), "w") do io
        write(io, "{\"schema\":\"dx-bench/v1\",\"results\":[", join(results, ","), "]}")
    end
    return length(results)
end

end
