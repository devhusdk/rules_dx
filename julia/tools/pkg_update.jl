using Pkg

function main(args)
    if isempty(args)
        println(stderr, "usage: pkg_update.jl SET_DIR [PACKAGE...]")
        return 2
    end
    dir, packages = args[1], args[2:end]
    mktempdir() do temp
        for file in ("Project.toml", "Manifest.toml")
            source = joinpath(dir, file)
            isfile(source) && cp(source, joinpath(temp, file))
        end
        Pkg.activate(temp)
        if isempty(packages)
            Pkg.update()
        else
            Pkg.update(packages)
        end
        cp(joinpath(temp, "Manifest.toml"), joinpath(dir, "Manifest.toml"); force = true)
    end
    return 0
end

exit(main(ARGS))
