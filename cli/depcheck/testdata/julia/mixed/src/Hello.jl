module Hello
using JSON
using Known: thing

hello() = JSON.json("hello")
end
