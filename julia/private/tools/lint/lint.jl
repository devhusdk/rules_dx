# Loading Pkg and REPL first matches the precompile environment, so the
# prebuilt JuliaWorkspaces cache is reused instead of recompiled.
using Pkg, REPL
using JuliaWorkspaces

const filepath2uri = JuliaWorkspaces.URIs2.filepath2uri
const REPORT_VERSION = 1

function json_string(s::AbstractString)
    io = IOBuffer()
    write(io, '"')
    for c in s
        if c == '"'
            write(io, "\\\"")
        elseif c == '\\'
            write(io, "\\\\")
        elseif c == '\n'
            write(io, "\\n")
        elseif c == '\r'
            write(io, "\\r")
        elseif c == '\t'
            write(io, "\\t")
        elseif c < ' '
            write(io, "\\u", string(UInt16(c), base = 16, pad = 4))
        else
            write(io, c)
        end
    end
    write(io, '"')
    return String(take!(io))
end

function option(args, name)
    index = findfirst(==(name), args)
    (index === nothing || index == length(args)) && return nothing
    return args[index+1]
end

# Diagnostic ranges are 1-based byte offsets; columns count characters.
function line_col(text::String, byte::Int)
    byte = clamp(byte, 1, ncodeunits(text) + 1)
    line = 1
    line_start = 1
    index = 1
    while index < byte
        if text[index] == '\n'
            line += 1
            line_start = nextind(text, index)
        end
        index = nextind(text, index)
    end
    prefix = text[line_start:prevind(text, byte)]
    return line, length(prefix) + 1
end

# Semantic rules only run for files that belong to a project, so the files get
# an in-memory empty project and manifest at their common directory.
function common_dir(files)
    parts = [splitpath(dirname(abspath(file))) for file in files]
    common = parts[1]
    for other in parts[2:end]
        keep = 0
        while keep < min(length(common), length(other)) && common[keep+1] == other[keep+1]
            keep += 1
        end
        common = common[1:keep]
    end
    return isempty(common) ? "/" : joinpath(common...)
end

function report_line(file, text, diagnostic)
    line, col = line_col(text, first(diagnostic.range))
    rule = diagnostic.code === nothing ? "lint" : String(diagnostic.code)
    severity = String(diagnostic.severity)
    return "{\"v\":$REPORT_VERSION,\"file\":$(json_string(file)),\"line\":$line,\"col\":$col," *
           "\"severity\":$(json_string(severity)),\"rule\":$(json_string(rule)),\"message\":$(json_string(diagnostic.message))}"
end

function main(args)
    scratch = option(args, "--scratch")
    report = option(args, "--report")
    config = option(args, "--config")
    files = String[]
    index = 1
    while index <= length(args)
        if args[index] in ("--scratch", "--report", "--config")
            index += 2
        else
            push!(files, args[index])
            index += 1
        end
    end
    if scratch === nothing || report === nothing || isempty(files)
        println(stderr, "usage: lint.jl --scratch DIR --report OUT [--config JuliaLint.toml] FILE...")
        return 2
    end
    mkpath(scratch)
    workspace = JuliaWorkspace(
        dynamic = DynamicOff,
        symbolcache_download = false,
        resolve_workspace_environments = false,
        store_path = scratch,
    )
    texts = Dict{Any,Tuple{String,String}}()
    root = common_dir(files)
    textfiles = TextFile[
        TextFile(filepath2uri(joinpath(root, "Project.toml")), SourceText("", "toml")),
        TextFile(filepath2uri(joinpath(root, "Manifest.toml")), SourceText("manifest_format = \"2.0\"\n", "toml")),
    ]
    if config !== nothing
        push!(textfiles, TextFile(filepath2uri(joinpath(root, "JuliaLint.toml")), SourceText(read(config, String), "toml")))
    end
    for file in files
        text = read(file, String)
        uri = filepath2uri(abspath(file))
        texts[uri] = (file, text)
        push!(textfiles, TextFile(uri, SourceText(text, "julia")))
    end
    JuliaWorkspaces.add_files!(workspace, textfiles)
    diagnostics = get_diagnostics_blocking(workspace)
    lines = String[]
    for file in files
        uri = filepath2uri(abspath(file))
        found = get(diagnostics, uri, nothing)
        found === nothing && continue
        for diagnostic in sort(found; by = d -> first(d.range))
            push!(lines, report_line(file, texts[uri][2], diagnostic))
        end
    end
    write(report, join(lines, "\n") * (isempty(lines) ? "" : "\n"))
    return 0
end

try
    exit(main(ARGS))
catch err
    println(stderr, "lint.jl: error: $(sprint(showerror, err))")
    exit(2)
end
