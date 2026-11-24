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

func IsStdLib(name string) bool { return stdlibModules[name] }

func Candidates(name string) []string {
	var out []string
	for i := strings.LastIndexByte(name, '.'); i > 0; i = strings.LastIndexByte(name[:i], '.') {
		out = append(out, name[:i])
	}
	return out
}
