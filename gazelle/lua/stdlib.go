package lua

// stdlib lists the modules the Lua standard library provides, plus runfiles,
// which the launcher preloads (see lua/README.md, Modules and data files).
var stdlib = map[string]struct{}{
	"coroutine": {}, "debug": {}, "io": {}, "math": {}, "os": {},
	"package": {}, "string": {}, "table": {}, "utf8": {}, "runfiles": {},
}

// IsStdLib reports whether name needs no dependency.
func IsStdLib(name string) bool {
	_, ok := stdlib[name]
	return ok
}
