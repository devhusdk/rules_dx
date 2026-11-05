package main

import (
	"flag"
	"fmt"
	"os"
	"strings"

	"rules_dx/go/tools/analysis/unit"

	"github.com/kisielk/errcheck/errcheck"
	"golang.org/x/tools/go/analysis"
)

var boolFlags = []string{"assert", "blank"}

// applyFlags turns on the named bool flags of the errcheck analyzer.
func applyFlags(names []string) error {
	for _, name := range names {
		if !contains(boolFlags, name) {
			return fmt.Errorf("unknown errcheck flag %q, valid names: %s", name, strings.Join(boolFlags, ", "))
		}
		if err := errcheck.Analyzer.Flags.Set(name, "true"); err != nil {
			return err
		}
	}
	return nil
}

func contains(list []string, want string) bool {
	for _, s := range list {
		if s == want {
			return true
		}
	}
	return false
}

func configure(extra []string) error {
	flags := flag.NewFlagSet("errcheck", flag.ContinueOnError)
	names := flags.String("flags", "", "comma separated errcheck bool flags to turn on")
	if err := flags.Parse(extra); err != nil {
		return err
	}
	if *names == "" {
		return nil
	}
	return applyFlags(strings.Split(*names, ","))
}

func main() {
	opts := unit.Options{Configure: configure}
	os.Exit(unit.Run("errcheck", []*analysis.Analyzer{errcheck.Analyzer}, opts, os.Args[1:], os.Stdout, os.Stderr))
}
