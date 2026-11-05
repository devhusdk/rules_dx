package main

import (
	"flag"
	"fmt"
	"os"
	"rules_dx/go/tools/analysis/unit"
	"sort"
	"strings"

	"golang.org/x/tools/go/analysis"
	"golang.org/x/tools/go/analysis/passes/appends"
	"golang.org/x/tools/go/analysis/passes/asmdecl"
	"golang.org/x/tools/go/analysis/passes/assign"
	"golang.org/x/tools/go/analysis/passes/atomic"
	"golang.org/x/tools/go/analysis/passes/bools"
	"golang.org/x/tools/go/analysis/passes/buildtag"
	"golang.org/x/tools/go/analysis/passes/cgocall"
	"golang.org/x/tools/go/analysis/passes/composite"
	"golang.org/x/tools/go/analysis/passes/copylock"
	"golang.org/x/tools/go/analysis/passes/defers"
	"golang.org/x/tools/go/analysis/passes/directive"
	"golang.org/x/tools/go/analysis/passes/errorsas"
	"golang.org/x/tools/go/analysis/passes/framepointer"
	"golang.org/x/tools/go/analysis/passes/hostport"
	"golang.org/x/tools/go/analysis/passes/httpresponse"
	"golang.org/x/tools/go/analysis/passes/ifaceassert"
	"golang.org/x/tools/go/analysis/passes/loopclosure"
	"golang.org/x/tools/go/analysis/passes/lostcancel"
	"golang.org/x/tools/go/analysis/passes/nilfunc"
	"golang.org/x/tools/go/analysis/passes/nilness"
	"golang.org/x/tools/go/analysis/passes/printf"
	"golang.org/x/tools/go/analysis/passes/shadow"
	"golang.org/x/tools/go/analysis/passes/shift"
	"golang.org/x/tools/go/analysis/passes/sigchanyzer"
	"golang.org/x/tools/go/analysis/passes/slog"
	"golang.org/x/tools/go/analysis/passes/stdmethods"
	"golang.org/x/tools/go/analysis/passes/stdversion"
	"golang.org/x/tools/go/analysis/passes/stringintconv"
	"golang.org/x/tools/go/analysis/passes/structtag"
	"golang.org/x/tools/go/analysis/passes/testinggoroutine"
	"golang.org/x/tools/go/analysis/passes/tests"
	"golang.org/x/tools/go/analysis/passes/timeformat"
	"golang.org/x/tools/go/analysis/passes/unmarshal"
	"golang.org/x/tools/go/analysis/passes/unreachable"
	"golang.org/x/tools/go/analysis/passes/unsafeptr"
	"golang.org/x/tools/go/analysis/passes/unusedresult"
	"golang.org/x/tools/go/analysis/passes/unusedwrite"
	"golang.org/x/tools/go/analysis/passes/waitgroup"
)

// defaults run unless removed with -NAME. optional ones run only when named.
var defaults = []*analysis.Analyzer{
	appends.Analyzer,
	asmdecl.Analyzer,
	assign.Analyzer,
	atomic.Analyzer,
	bools.Analyzer,
	buildtag.Analyzer,
	cgocall.Analyzer,
	composite.Analyzer,
	copylock.Analyzer,
	defers.Analyzer,
	directive.Analyzer,
	errorsas.Analyzer,
	framepointer.Analyzer,
	httpresponse.Analyzer,
	hostport.Analyzer,
	ifaceassert.Analyzer,
	loopclosure.Analyzer,
	lostcancel.Analyzer,
	nilfunc.Analyzer,
	nilness.Analyzer,
	printf.Analyzer,
	shift.Analyzer,
	sigchanyzer.Analyzer,
	slog.Analyzer,
	stdmethods.Analyzer,
	stdversion.Analyzer,
	stringintconv.Analyzer,
	structtag.Analyzer,
	tests.Analyzer,
	testinggoroutine.Analyzer,
	timeformat.Analyzer,
	unmarshal.Analyzer,
	unreachable.Analyzer,
	unsafeptr.Analyzer,
	unusedwrite.Analyzer,
	unusedresult.Analyzer,
	waitgroup.Analyzer,
}

var optional = []*analysis.Analyzer{
	shadow.Analyzer,
}

var active = defaults

// selectNamed applies names to the defaults: NAME adds an analyzer, -NAME
// removes a default one.
func selectNamed(names []string) ([]*analysis.Analyzer, error) {
	known := map[string]*analysis.Analyzer{}
	var valid []string
	for _, a := range append(append([]*analysis.Analyzer{}, defaults...), optional...) {
		known[a.Name] = a
		valid = append(valid, a.Name)
	}
	sort.Strings(valid)
	chosen := map[string]bool{}
	for _, a := range defaults {
		chosen[a.Name] = true
	}
	for _, name := range names {
		on := !strings.HasPrefix(name, "-")
		name = strings.TrimPrefix(name, "-")
		if known[name] == nil {
			return nil, fmt.Errorf("unknown govet analyzer %q, valid names: %s", name, strings.Join(valid, ", "))
		}
		chosen[name] = on
	}
	var out []*analysis.Analyzer
	for _, a := range append(append([]*analysis.Analyzer{}, defaults...), optional...) {
		if chosen[a.Name] {
			out = append(out, a)
		}
	}
	return out, nil
}

func configure(extra []string) error {
	flags := flag.NewFlagSet("govet", flag.ContinueOnError)
	names := flags.String("analyzers", "", "comma separated analyzers to add, or remove with a leading -")
	if err := flags.Parse(extra); err != nil {
		return err
	}
	active = defaults
	if *names == "" {
		return nil
	}
	chosen, err := selectNamed(strings.Split(*names, ","))
	if err != nil {
		return err
	}
	active = chosen
	return nil
}

func main() {
	opts := unit.Options{
		Configure: configure,
		Select:    func([]string) []*analysis.Analyzer { return active },
	}
	os.Exit(unit.Run("govet", defaults, opts, os.Args[1:], os.Stdout, os.Stderr))
}
