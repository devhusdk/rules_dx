package main

import (
	"os"
	"rules_dx/go/tools/analysis/gotype"
)

func main() {
	os.Exit(gotype.Main(os.Args[1:], os.Stderr))
}
