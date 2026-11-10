using TOML
using JET

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

function report_line(report)
    kind = replace(String(nameof(typeof(report))), r"Report$" => "")
    frames = Tuple{String,Int}[]
    if hasproperty(report, :vst)
        for frame in report.vst
            push!(frames, (String(frame.file), Int(frame.line)))
        end
    elseif hasproperty(report, :file) && hasproperty(report, :line)
        push!(frames, (String(report.file), Int(report.line)))
    end
    message = if hasproperty(report, :vst)
        sprint(JET.print_report_message, report)
    elseif hasproperty(report, :msg)
        String(report.msg)
    else
        sprint(show, report)
    end
    message = replace(message, r"Main\.var\"##JETVirtualModule#\d+\"\." => "")
    parts = String[]
    for (file, line) in frames
        push!(parts, "{\"file\":$(json_string(file)),\"line\":$line}")
    end
    return "{\"v\":$REPORT_VERSION,\"kind\":$(json_string(kind)),\"message\":$(json_string(message)),\"frames\":[$(join(parts, ","))]}"
end

function add_include(scratch, include_path)
    staged = joinpath(scratch, "dx-jet", "rf", include_path)
    isdir(staged) && push!(LOAD_PATH, staged)
end

function option(args, name)
    index = findfirst(==(name), args)
    (index === nothing || index == length(args)) && return nothing
    return args[index+1]
end

function main(args)
    config = option(args, "--config")
    scratch = option(args, "--scratch")
    report = option(args, "--report")
    if config === nothing || scratch === nothing || report === nothing
        println(stderr, "usage: jet.jl --config CONFIG --scratch DIR --report OUT")
        return 2
    end
    settings = TOML.parsefile(config)
    foreach(inc -> add_include(scratch, inc), get(settings, "includes", String[]))
    lines = String[]
    for root in get(settings, "roots", String[])
        result = JET.report_file(
            joinpath(scratch, root);
            analyze_from_definitions = true,
            ignore_missing_comparison = true,
            toplevel_logger = nothing,
        )
        for found in JET.get_reports(result)
            found isa JET.ToplevelWarningReport && continue
            push!(lines, report_line(found))
        end
    end
    write(report, join(lines, "\n") * (isempty(lines) ? "" : "\n"))
    return 0
end

try
    exit(main(ARGS))
catch err
    println(stderr, "jet.jl: error: $(sprint(showerror, err))")
    exit(2)
end
