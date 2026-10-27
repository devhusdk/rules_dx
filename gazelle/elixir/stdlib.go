package elixir

import (
	_ "embed"
	"strings"
)

//go:embed stdlib_modules.txt
var stdlibModulesText string

var stdlibModules = parseStdlib(stdlibModulesText)

func parseStdlib(text string) map[string]bool {
	out := make(map[string]bool)
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		if line != "" && !strings.HasPrefix(line, "#") {
			out[line] = true
		}
	}
	return out
}

// IsStdLib reports whether name is a module that ships with the pinned Elixir.
func IsStdLib(name string) bool { return stdlibModules[name] }

// Candidates returns the dotted prefixes of name, longest first, so one
// resolve directive on a top module covers its whole package.
func Candidates(name string) []string {
	var out []string
	for i := strings.LastIndexByte(name, '.'); i > 0; i = strings.LastIndexByte(name[:i], '.') {
		out = append(out, name[:i])
	}
	return out
}
