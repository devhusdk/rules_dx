# Loads one Julia library and writes the docstrings of its public API, read through Documenter's
# DocSystem, as versioned JSON for julia_extract.
using Documenter

const FORMAT = 1
const USAGE = "usage: doc_dump --module NAME --own FILE --paths FILE --out FILE"

function fail_with(message)
    println(stderr, "doc_dump: ", message)
    exit(1)
end

function parse_args(argv)
    flags = ["--module", "--own", "--paths", "--out"]
    options = Dict{String,String}()
    i = 1
    while i <= length(argv)
        flag = argv[i]
        flag in flags || fail_with("unknown argument '$flag'\n$USAGE")
        (i < length(argv) && !isempty(argv[i+1])) || fail_with("$flag needs a value\n$USAGE")
        haskey(options, flag) && fail_with("$flag given more than once")
        options[flag] = argv[i+1]
        i += 2
    end
    missing_flags = filter(flag -> !haskey(options, flag), flags)
    isempty(missing_flags) || fail_with("$(join(missing_flags, ", ")) is required\n$USAGE")
    return options
end

function read_rows(path, columns)
    isfile(path) || fail_with("cannot read manifest $path")
    rows = Vector{Vector{String}}()
    for line in split(read(path, String), '\n'; keepempty = false)
        row = split(line, '\t'; keepempty = true)
        length(row) == columns || fail_with("$path has a malformed line: $(repr(line))")
        push!(rows, String.(row))
    end
    return rows
end

function json_string(text::AbstractString)
    io = IOBuffer()
    print(io, '"')
    for c in text
        if c == '"'
            print(io, "\\\"")
        elseif c == '\\'
            print(io, "\\\\")
        elseif c == '\n'
            print(io, "\\n")
        elseif c == '\r'
            print(io, "\\r")
        elseif c == '\t'
            print(io, "\\t")
        elseif c < ' '
            print(io, "\\u", string(UInt16(c); base = 16, pad = 4))
        else
            print(io, c)
        end
    end
    print(io, '"')
    return String(take!(io))
end

struct Own
    rel::String
    generated::Bool
end

function kind_of(mod::Module, name::Symbol, value)
    value isa Module && return "module"
    if value isa Function
        return startswith(string(name), "@") ? "macro" : "function"
    end
    if value isa DataType || value isa UnionAll
        base = Base.unwrap_unionall(value)
        if base isa DataType && base.name.name === name
            isabstracttype(base) && return "abstract"
            isprimitivetype(base) && return "primitive"
            return ismutabletype(base) ? "mutable_struct" : "struct"
        end
        return "alias"
    end
    value isa Union && return "alias"
    return isconst(mod, name) ? "constant" : "variable"
end

function signature_of(kind, name, typesig)
    kind == "abstract" && return "abstract type $name"
    kind == "primitive" && return "primitive type $name"
    kind == "struct" && return "struct $name"
    kind == "mutable_struct" && return "mutable struct $name"
    kind == "constant" && return "const $name"
    if kind == "function" || kind == "macro"
        typesig === Union{} && return string(name)
        base = Base.unwrap_unionall(typesig)
        base isa DataType || return string(name)
        params = join(["::" * string(p) for p in base.parameters], ", ")
        return "$name($params)"
    end
    return ""
end

function collect_objects!(objects, seen, mod::Module, path::String, owns)
    mod in seen && return
    push!(seen, mod)
    for name in sort!(collect(names(mod)); by = string)
        string(name)[1] == '#' && continue
        isdefined(mod, name) || continue
        value = getfield(mod, name)
        if value isa Module
            if value !== mod && parentmodule(value) === mod
                add_object!(objects, mod, name, value, path, owns)
                collect_objects!(objects, seen, value, path * "." * string(name), owns)
            end
            continue
        end
        add_object!(objects, mod, name, value, path, owns)
    end
end

function add_object!(objects, mod::Module, name::Symbol, value, path::String, owns)
    binding = value isa Module ? Base.Docs.Binding(value, name) : Base.Docs.Binding(mod, name)
    docs = Documenter.DocSystem.getdocs(binding)
    kind = kind_of(mod, name, value)
    texts = String[]
    signatures = String[]
    file = nothing
    line = 0
    for docstr in docs
        source = get(docstr.data, :path, nothing)
        entry = source === nothing ? nothing : get(owns, abspath(source), nothing)
        entry === nothing && continue
        file === nothing && (file = entry; line = Int(get(docstr.data, :linenumber, 0)))
        push!(texts, rstrip(join(string.(docstr.text))))
        signature = signature_of(kind, name, get(docstr.data, :typesig, Union{}))
        isempty(signature) || signature in signatures || push!(signatures, signature)
    end
    if file === nothing
        # Undocumented: public API all the same, when the binding comes from this library.
        if !(value isa Module) && parentmodule_of(value, mod) !== mod
            return
        end
        file = sort!(collect(values(owns)); by = o -> o.rel)[1]
    end
    full = path * "." * string(name)
    push!(objects, (
        path = full,
        name = string(name),
        parent = path,
        kind = kind,
        signatures = signatures,
        doc = join(texts, "\n\n"),
        file = file.rel,
        generated = file.generated,
        line = line,
    ))
end

function parentmodule_of(value, fallback)
    try
        value isa Function && return parentmodule(value)
        value isa DataType && return parentmodule(value)
        value isa UnionAll && return parentmodule(Base.unwrap_unionall(value))
    catch
    end
    return fallback
end

function write_json(path, root_name, objects)
    io = IOBuffer()
    println(io, "{")
    println(io, "  \"format\": $FORMAT,")
    println(io, "  \"julia\": ", json_string(string(VERSION)), ",")
    println(io, "  \"documenter\": ", json_string(string(pkgversion(Documenter))), ",")
    println(io, "  \"objects\": [")
    for (i, o) in enumerate(objects)
        println(io, "    {")
        println(io, "      \"path\": ", json_string(o.path), ",")
        println(io, "      \"name\": ", json_string(o.name), ",")
        println(io, "      \"module\": ", json_string(o.parent), ",")
        println(io, "      \"kind\": ", json_string(o.kind), ",")
        println(io, "      \"signatures\": [", join(json_string.(o.signatures), ", "), "],")
        println(io, "      \"doc\": ", json_string(o.doc), ",")
        println(io, "      \"file\": ", json_string(o.file), ",")
        println(io, "      \"generated\": ", o.generated, ",")
        println(io, "      \"line\": ", o.line)
        println(io, "    }", i == length(objects) ? "" : ",")
    end
    println(io, "  ]")
    println(io, "}")
    write(path, take!(io))
end

function main()
    options = parse_args(copy(ARGS))
    own_rows = read_rows(options["--own"], 3)
    isempty(own_rows) && fail_with("--own lists no source file")
    owns = Dict{String,Own}()
    for (exec, rel, generated) in own_rows
        owns[abspath(exec)] = Own(rel, generated == "1")
        isfile(exec) || fail_with("source file $exec does not exist")
    end
    for line in split(read(options["--paths"], String), '\n'; keepempty = false)
        push!(LOAD_PATH, abspath(line))
    end
    name = Symbol(options["--module"])
    try
        Core.eval(Main, :(using $name))
    catch error
        fail_with("cannot load module $name: $(sprint(showerror, error))")
    end
    mod = Base.invokelatest(getfield, Main, name)
    mod isa Module || fail_with("$name is not a module")
    objects = NamedTuple[]
    seen = Set{Module}()
    # The root module is documented as the first object; its members follow.
    add_object!(objects, mod, name, mod, "", owns)
    isempty(objects) && fail_with("module $name has no documentation from the library's own files")
    objects[1] = merge(objects[1], (path = string(name), parent = ""))
    collect_objects!(objects, seen, mod, string(name), owns)
    write_json(options["--out"], string(name), objects)
end

main()
