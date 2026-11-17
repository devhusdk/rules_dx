using Report

function capture(path, digits)
    out = IOBuffer()
    err = IOBuffer()
    code = run_report(path, digits; io = out, err = err)
    return code, String(take!(out)), String(take!(err))
end

function expect(name, ok)
    ok || error("report: $name")
end

mktempdir() do dir
    good = joinpath(dir, "good.csv")
    write(good, "sensor,value\na,1.0\nb,2.0\nc,3.0\n")
    bad = joinpath(dir, "bad.csv")
    write(bad, "sensor,value\na,1.0\nb,oops\n")
    empty = joinpath(dir, "empty.csv")
    write(empty, "sensor,value\n")

    code, out, err = capture(good, 2)
    expect("good exit code $code", code == 0)
    expect("good stderr $(repr(err))", err == "")
    expect("good readings in $(repr(out))", occursin("readings: 3\n", out))
    expect("good mean in $(repr(out))", occursin("mean: 2.0\n", out))
    expect("good archive in $(repr(out))", occursin("archive: ok\n", out))

    code, out, err = capture(bad, 2)
    expect("bad exit code $code", code == 1)
    expect("bad stdout $(repr(out))", out == "")
    expect("bad stderr $(repr(err))", occursin("line 3: malformed row", err))

    code, _, err = capture(empty, 2)
    expect("empty exit code $code", code == 1)
    expect("empty stderr $(repr(err))", occursin("no readings", err))

    code, _, err = capture(joinpath(dir, "missing.csv"), 2)
    expect("missing exit code $code", code == 2)
    expect("missing stderr $(repr(err))", occursin("cannot read", err))
end
