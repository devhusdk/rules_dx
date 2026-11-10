package main

import (
	"os"
	"rules_dx/go/private/tools/analysis/gotype"
)

func main() {
	os.Exit(gotype.Main(os.Args[1:], os.Stderr))
}
