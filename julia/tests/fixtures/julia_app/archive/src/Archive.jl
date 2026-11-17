module Archive

using Bzip2_jll: libbzip2

export compress, decompress

const BZ_OK = Cint(0)

"""
    compress(data) -> Vector{UInt8}

Compress bytes with bzip2.
"""
function compress(data::AbstractVector{UInt8})
    src = Vector{UInt8}(data)
    cap = length(src) + div(length(src), 100) + 600
    dest = Vector{UInt8}(undef, cap)
    len = Ref(Cuint(cap))
    rc = ccall(
        (:BZ2_bzBuffToBuffCompress, libbzip2),
        Cint,
        (Ptr{UInt8}, Ptr{Cuint}, Ptr{UInt8}, Cuint, Cint, Cint, Cint),
        dest,
        len,
        src,
        length(src),
        9,
        0,
        0,
    )
    rc == BZ_OK || error("bzip2 compress failed with code $(rc)")
    return resize!(dest, Int(len[]))
end

"""
    decompress(data, size) -> Vector{UInt8}

Decompress bzip2 bytes that expand to at most `size` bytes. Corrupted input
raises an error.
"""
function decompress(data::AbstractVector{UInt8}, size::Integer)
    src = Vector{UInt8}(data)
    dest = Vector{UInt8}(undef, size)
    len = Ref(Cuint(size))
    rc = ccall(
        (:BZ2_bzBuffToBuffDecompress, libbzip2),
        Cint,
        (Ptr{UInt8}, Ptr{Cuint}, Ptr{UInt8}, Cuint, Cint, Cint),
        dest,
        len,
        src,
        length(src),
        0,
        0,
    )
    rc == BZ_OK || error("bzip2 decompress failed with code $(rc)")
    return resize!(dest, Int(len[]))
end

end
