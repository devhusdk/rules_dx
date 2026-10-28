import Runfiles
using Report

function main(args)
    digits = parse(Int, get(ENV, "JULIA_APP_DIGITS", "2"))
    path = if isempty(args)
        Runfiles.rlocation("_main/examples/julia-app/data/readings.csv")
    else
        args[1]
    end
    return run_report(path, digits)
end

exit(main(ARGS))
