module Greet

export greeting

greeting(name::AbstractString) = "Hello, $(name)!"

end
