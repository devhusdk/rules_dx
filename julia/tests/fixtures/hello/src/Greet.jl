module Greet

export greeting

greeting(name::AbstractString) = "Hello, $(name)!"

precompile(greeting, (String,))

end
