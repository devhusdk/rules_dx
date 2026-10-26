package common

import "strings"

var nodeBuiltins = map[string]struct{}{
	"assert": {}, "async_hooks": {}, "buffer": {}, "child_process": {},
	"cluster": {}, "console": {}, "constants": {}, "crypto": {},
	"dgram": {}, "diagnostics_channel": {}, "dns": {}, "domain": {},
	"events": {}, "fs": {}, "fs/promises": {}, "http": {}, "http2": {},
	"https": {}, "inspector": {}, "inspector/promises": {}, "module": {},
	"net": {}, "os": {}, "path": {}, "path/posix": {}, "path/win32": {},
	"perf_hooks": {}, "process": {}, "punycode": {}, "querystring": {},
	"readline": {}, "readline/promises": {}, "repl": {}, "stream": {}, "stream/consumers": {}, "stream/promises": {},
	"stream/web": {}, "string_decoder": {}, "sys": {}, "timers": {}, "timers/promises": {}, "tls": {},
	"trace_events": {}, "tty": {}, "url": {}, "util": {},
	"util/types": {}, "v8": {}, "vm": {}, "wasi": {},
	"worker_threads": {}, "zlib": {},
}

// nodePrefixedBuiltins only resolve with the node: prefix.
var nodePrefixedBuiltins = map[string]struct{}{
	"ffi": {}, "sea": {}, "sqlite": {}, "test": {}, "test/reporters": {},
}

func IsNodeBuiltin(name string) bool {
	rest, prefixed := strings.CutPrefix(name, "node:")
	if _, ok := nodeBuiltins[rest]; ok {
		return true
	}
	if prefixed {
		_, ok := nodePrefixedBuiltins[rest]
		return ok
	}
	return false
}
