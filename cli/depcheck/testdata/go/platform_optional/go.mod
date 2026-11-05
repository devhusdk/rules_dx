module example.com/hello

go 1.21

require (
	example.com/greet v1.0.0
	example.com/winonly v1.0.0 // depcheck:platform
	example.com/optionalfeat v1.0.0 // depcheck:optional
)
