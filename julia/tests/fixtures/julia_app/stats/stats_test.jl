using Stats: histogram, mean, quantile, variance

function expect(name, got, want)
    got == want || error("$name: got $(repr(got)), want $(repr(want))")
end

function expect_argument_error(name, f)
    try
        f()
    catch e
        e isa ArgumentError && return nothing
        rethrow()
    end
    error("$name: no ArgumentError")
end

xs = [1.0, 2.0, 3.0, 4.0]
expect("mean", mean(xs), 2.5)
expect("variance", variance(xs), 1.25)
expect("quantile 0", quantile(xs, 0.0), 1.0)
expect("quantile 0.5", quantile(xs, 0.5), 2.5)
expect("quantile 1", quantile(xs, 1.0), 4.0)
buckets = histogram([5.0, 1.0, 1.5, 12.0], 5)
expect("histogram keys", collect(keys(buckets)), [0.0, 5.0, 10.0])
expect("histogram values", collect(values(buckets)), [2, 1, 1])
expect_argument_error("empty mean", () -> mean(Float64[]))
expect_argument_error("NaN mean", () -> mean([1.0, NaN]))
expect_argument_error("quantile p", () -> quantile(xs, 2))
