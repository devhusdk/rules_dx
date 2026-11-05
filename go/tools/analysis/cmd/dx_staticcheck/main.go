package main

import (
	"os"
	"rules_dx/go/tools/analysis/unit"
)

func main() {
	opts := unit.Options{Configure: configure, Select: selected}
	os.Exit(unit.Run("staticcheck", nil, opts, os.Args[1:], os.Stdout, os.Stderr))
}
