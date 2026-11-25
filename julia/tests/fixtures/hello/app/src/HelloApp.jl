module HelloApp

using Greet: greeting

function julia_main()::Cint
    println(greeting("julia"))
    return 0
end

end
