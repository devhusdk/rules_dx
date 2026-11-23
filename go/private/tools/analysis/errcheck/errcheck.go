package errcheck

import (
	"flag"
	"fmt"
	"io"
	"rules_dx/go/private/tools/analysis/unit"

	"github.com/kisielk/errcheck/errcheck"
	"golang.org/x/tools/go/analysis"
)

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

func Main(args []string, stdout, stderr io.Writer) int {
	opts := unit.Options{Configure: configure}
	return unit.Run("errcheck", []*analysis.Analyzer{errcheck.Analyzer}, opts, args, stdout, stderr)
}
