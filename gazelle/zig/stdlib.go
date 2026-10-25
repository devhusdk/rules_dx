package zig

var stdlibModules = map[string]bool{
	"bazel_builtin": true,
	"builtin":       true,
	"c":             true,
	"root":          true,
	"std":           true,
}

func IsStdLib(name string) bool { return stdlibModules[name] }
