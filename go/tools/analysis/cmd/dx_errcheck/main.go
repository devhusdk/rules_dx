package main

import (
	"flag"
	"fmt"
	"os"

	"rules_dx/go/tools/analysis/unit"

	"github.com/kisielk/errcheck/errcheck"
	"golang.org/x/tools/go/analysis"
)

// strictFlags are the errcheck bool flags the strict profile turns on.
var strictFlags = []string{"blank", "assert"}

func configure(extra []string) error {
	flags := flag.NewFlagSet("errcheck", flag.ContinueOnError)
	profile := flags.String("profile", "standard", "standard or strict")
	exclude := flags.String("exclude", "", "errcheck exclude file")
	if err := flags.Parse(extra); err != nil {
		return err
	}
	switch *profile {
	case "standard":
	case "strict":
		for _, name := range strictFlags {
			if err := errcheck.Analyzer.Flags.Set(name, "true"); err != nil {
				return err
			}
		}
	default:
		return fmt.Errorf("unknown profile %q, want standard or strict", *profile)
	}
	if *exclude == "" {
		return nil
	}
	return errcheck.Analyzer.Flags.Set("exclude", *exclude)
}

func main() {
	opts := unit.Options{Configure: configure}
	os.Exit(unit.Run("errcheck", []*analysis.Analyzer{errcheck.Analyzer}, opts, os.Args[1:], os.Stdout, os.Stderr))
}
