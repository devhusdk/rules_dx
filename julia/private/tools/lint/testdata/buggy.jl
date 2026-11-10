module Buggy
function total(x)
    for i in 1:length(x)
        println(x[i])
    end
    if true
        return 1
    end
    return 0
end
end
