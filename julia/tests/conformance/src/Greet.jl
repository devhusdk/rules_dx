module Greet

export greet

greet(name::AbstractString) = "hello $(name)"

end
