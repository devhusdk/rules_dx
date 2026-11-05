module example.com/hello

go 1.21

require (
	example.com/greet v1.0.0
	example.com/testhelper v1.0.0 // depcheck:test
)
