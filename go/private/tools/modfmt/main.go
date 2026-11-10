// Command modfmt formats go.mod files.
package main

import (
	"bytes"
	"flag"
	"fmt"
	"os"

	"golang.org/x/mod/modfile"
)

func main() {
	os.Exit(run(os.Args[1:], os.Stdout, os.Stderr))
}

func run(args []string, stdout, stderr interface{ Write([]byte) (int, error) }) int {
	fs := flag.NewFlagSet("modfmt", flag.ContinueOnError)
	fs.SetOutput(stderr)
	check := fs.Bool("c", false, "exit 1 when a file is not formatted")
	list := fs.Bool("l", false, "list files that are not formatted")
	write := fs.Bool("w", false, "rewrite files in place")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	status := 0
	for _, path := range fs.Args() {
		src, err := os.ReadFile(path)
		if err != nil {
			fmt.Fprintf(stderr, "modfmt: %v\n", err)
			return 2
		}
		file, err := modfile.Parse(path, src, nil)
		if err != nil {
			fmt.Fprintf(stderr, "modfmt: %v\n", err)
			return 2
		}
		out := modfile.Format(file.Syntax)
		if bytes.Equal(src, out) {
			continue
		}
		if *write {
			if err := os.WriteFile(path, out, 0o644); err != nil {
				fmt.Fprintf(stderr, "modfmt: %v\n", err)
				return 2
			}
		}
		if *list {
			fmt.Fprintln(stdout, path)
		}
		if *check {
			status = 1
		}
	}
	return status
}
