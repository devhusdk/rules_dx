package staticcheck

import (
	"io"
	"rules_dx/go/private/tools/analysis/unit"
)

// Main runs the configured staticcheck analyzers as a unitchecker over args and returns the exit code.
func Main(args []string, stdout, stderr io.Writer) int {
	opts := unit.Options{Configure: configure, Select: selected}
	return unit.Run("staticcheck", nil, opts, args, stdout, stderr)
}
