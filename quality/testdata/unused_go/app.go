package main

import (
	"os"

	_ "rules_dx/quality/testdata/unused_go/keep"
	"rules_dx/quality/testdata/unused_go/private"
	_ "rules_dx/quality/testdata/unused_go/public"
)

func main() {
	os.Exit(private.Used() - 1)
}
