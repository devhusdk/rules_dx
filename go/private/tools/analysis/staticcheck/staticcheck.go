package staticcheck

import (
	"io"
	"rules_dx/go/private/tools/analysis/unit"
)

func Main(args []string, stdout, stderr io.Writer) int {
	opts := unit.Options{Configure: configure, Select: selected}
	return unit.Run("staticcheck", nil, opts, args, stdout, stderr)
}
