package julia

import (
	_ "embed"
	"strings"
)

//go:embed stdlib.txt
var stdlibText string

var stdlibUUIDs = parseStdlib(stdlibText)

func parseStdlib(text string) map[string]string {
	out := make(map[string]string)
	for _, line := range strings.Split(text, "\n") {
		fields := strings.Fields(line)
		if len(fields) == 2 && !strings.HasPrefix(fields[0], "#") {
			out[fields[0]] = strings.ToLower(fields[1])
		}
	}
	return out
}

func IsStdLib(name string) bool {
	_, ok := stdlibUUIDs[name]
	return ok
}

func StdlibUUID(name string) (string, bool) {
	id, ok := stdlibUUIDs[name]
	return id, ok
}
