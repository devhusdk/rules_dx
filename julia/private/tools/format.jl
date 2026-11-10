using JuliaFormatter
using JuliaSyntax

function syntax_errors(path, text)
    stream = JuliaSyntax.ParseStream(text)
    JuliaSyntax.parse!(stream; rule = :all)
    errors = String[]
    if any(d -> d.level == :error, stream.diagnostics)
        source = JuliaSyntax.SourceFile(text; filename = path)
        for d in stream.diagnostics
            d.level == :error || continue
            line, col = JuliaSyntax.source_location(source, JuliaSyntax.first_byte(d))
            push!(errors, "$path:$line:$col: error: $(d.message)")
        end
    end
    return errors
end

function main(args)
    if length(args) < 2 || !(args[1] in ("--check", "--apply"))
        println(stderr, "usage: format.jl --check|--apply FILE...")
        return 2
    end
    apply = args[1] == "--apply"
    unformatted = String[]
    failed = false
    for path in args[2:end]
        text = try
            read(path, String)
        catch err
            println(stderr, "$path: error: $(sprint(showerror, err))")
            return 2
        end
        errors = syntax_errors(path, text)
        if !isempty(errors)
            foreach(e -> println(stderr, e), errors)
            failed = true
            continue
        end
        formatted = JuliaFormatter.format_text(text)
        formatted == text && continue
        push!(unformatted, path)
        apply && write(path, formatted)
    end
    foreach(println, unformatted)
    return (failed || (!isempty(unformatted) && !apply)) ? 1 : 0
end

try
    exit(main(ARGS))
catch err
    println(stderr, "format.jl: error: $(sprint(showerror, err))")
    exit(2)
end
