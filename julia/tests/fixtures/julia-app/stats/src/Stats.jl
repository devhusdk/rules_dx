module Stats

using OrderedCollections: OrderedDict

export mean, variance, quantile, histogram

function check(xs)
    isempty(xs) && throw(ArgumentError("empty input"))
    any(isnan, xs) && throw(ArgumentError("NaN in input"))
    return nothing
end

function mean(xs::AbstractVector{<:Real})
    check(xs)
    return sum(xs) / length(xs)
end

function variance(xs::AbstractVector{<:Real})
    m = mean(xs)
    return sum(x -> (x - m)^2, xs) / length(xs)
end

function quantile(xs::AbstractVector{<:Real}, p::Real)
    check(xs)
    0 <= p <= 1 || throw(ArgumentError("p must be in [0, 1]"))
    sorted = sort(xs)
    pos = 1 + p * (length(sorted) - 1)
    lo = floor(Int, pos)
    hi = ceil(Int, pos)
    return sorted[lo] + (pos - lo) * (sorted[hi] - sorted[lo])
end

"""
    histogram(xs, width)

Count values per bucket of the given width. Buckets are returned in
ascending order.
"""
function histogram(xs::AbstractVector{<:Real}, width::Real)
    check(xs)
    width > 0 || throw(ArgumentError("width must be positive"))
    counts = Dict{Float64,Int}()
    for x in xs
        key = floor(x / width) * width
        counts[key] = get(counts, key, 0) + 1
    end
    out = OrderedDict{Float64,Int}()
    for key in sort!(collect(keys(counts)))
        out[key] = counts[key]
    end
    return out
end

end
