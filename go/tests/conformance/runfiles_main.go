package main

import (
	"fmt"
	"os"

	"github.com/bazelbuild/rules_go/go/runfiles"
)

func main() {
	r, err := runfiles.New()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	path, err := r.Rlocation(os.Args[1])
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	content, err := os.ReadFile(path)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	fmt.Print(string(content))
}
