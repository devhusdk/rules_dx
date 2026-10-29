package main

import (
	"os"
	"rules_dx/go/tools/analysis/unit"

	"golang.org/x/tools/go/analysis"
	"honnef.co/go/tools/analysis/lint"
	"honnef.co/go/tools/simple"
	"honnef.co/go/tools/staticcheck"
	"honnef.co/go/tools/stylecheck"
)

// disabled mirrors the staticcheck defaults that are off unless requested.
var disabled = map[string]bool{
	"ST1000": true, "ST1003": true, "ST1016": true,
	"ST1020": true, "ST1021": true, "ST1022": true,
}

func analyzers() []*analysis.Analyzer {
	var out []*analysis.Analyzer
	for _, set := range [][]*lint.Analyzer{staticcheck.Analyzers, simple.Analyzers, stylecheck.Analyzers} {
		for _, a := range set {
			if !disabled[a.Analyzer.Name] {
				out = append(out, a.Analyzer)
			}
		}
	}
	return out
}

func main() {
	os.Exit(unit.Main("staticcheck", analyzers(), nil, os.Args[1:], os.Stdout, os.Stderr))
}
