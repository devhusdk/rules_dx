// Command dx_go runs one of the rules_dx Go tools, chosen by the name it is invoked as.
package main

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"rules_dx/go/private/tools/analysis/errcheck"
	"rules_dx/go/private/tools/analysis/gotype"
	"rules_dx/go/private/tools/analysis/govet"
	"rules_dx/go/private/tools/analysis/staticcheck"
	"rules_dx/go/private/tools/modfmt"
	"sort"
	"strings"
)

type tool func(args []string, stdout, stderr io.Writer) int

var tools = map[string]tool{
	"dx_errcheck":    errcheck.Main,
	"dx_gotype":      func(args []string, _, stderr io.Writer) int { return gotype.Main(args, stderr) },
	"dx_govet":       govet.Main,
	"dx_modfmt":      modfmt.Main,
	"dx_staticcheck": staticcheck.Main,
}

// toolName maps an argv[0] such as /x/dx-govet.exe to the tool key dx_govet.
func toolName(argv0 string) string {
	name := filepath.Base(argv0)
	name = strings.TrimSuffix(strings.TrimSuffix(name, ".exe"), ".EXE")
	return strings.ReplaceAll(name, "-", "_")
}

// run picks the tool from argv[0], or from argv[1] when invoked as dx_go itself.
func run(argv []string, stdout, stderr io.Writer) int {
	name := toolName(argv[0])
	args := argv[1:]
	if name == "dx_go" && len(args) > 0 {
		name, args = toolName(args[0]), args[1:]
	}
	if t, ok := tools[name]; ok {
		return t(args, stdout, stderr)
	}
	names := make([]string, 0, len(tools))
	for n := range tools {
		names = append(names, n)
	}
	sort.Strings(names)
	fmt.Fprintf(stderr, "dx_go: unknown tool %q; run as or pass one of: %s\n", name, strings.Join(names, ", "))
	return 2
}

func main() {
	os.Exit(run(os.Args, os.Stdout, os.Stderr))
}
