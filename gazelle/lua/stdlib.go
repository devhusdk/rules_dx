package lua

var stdlib = map[string]struct{}{
	"coroutine": {}, "debug": {}, "io": {}, "math": {}, "os": {},
	"package": {}, "string": {}, "table": {}, "utf8": {}, "runfiles": {},
}

func IsStdLib(name string) bool {
	_, ok := stdlib[name]
	return ok
}
