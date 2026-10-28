package main

import (
	"os"
	"rules_dx/go/tools/analysis/unit"

	"github.com/kisielk/errcheck/errcheck"
	"golang.org/x/tools/go/analysis"
)

func main() {
	os.Exit(unit.Main("errcheck", []*analysis.Analyzer{errcheck.Analyzer}, nil, os.Args[1:], os.Stdout, os.Stderr))
}
