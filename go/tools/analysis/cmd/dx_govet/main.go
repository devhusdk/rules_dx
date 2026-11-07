package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"regexp"
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
	unusedresult.Analyzer,
	waitgroup.Analyzer,
}

// strictOnly are the analyzers the strict profile adds to the defaults.
var strictOnly = []*analysis.Analyzer{
	nilness.Analyzer,
	shadow.Analyzer,
	unusedwrite.Analyzer,
}

var (
	active   = defaults
	excludes = map[string][]*regexp.Regexp{}
)

// analyzersFor returns the analyzers the profile runs.
func analyzersFor(profile string) ([]*analysis.Analyzer, error) {
	switch profile {
	case "standard":
		return defaults, nil
	case "strict":
		return append(append([]*analysis.Analyzer{}, defaults...), strictOnly...), nil
	}
	return nil, fmt.Errorf("unknown profile %q, want standard or strict", profile)
}

// analyzerConfig is the nogo style config: per analyzer, regexes of files
// whose findings are dropped, each with a reason.
type analyzerConfig map[string]struct {
	ExcludeFiles map[string]string `json:"exclude_files"`
}

func loadExcludes(path string, valid []*analysis.Analyzer) (map[string][]*regexp.Regexp, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var cfg analyzerConfig
	if err := json.Unmarshal(raw, &cfg); err != nil {
		return nil, fmt.Errorf("parse govet config %s: %w", path, err)
	}
	known := map[string]bool{}
	var names []string
	for _, a := range valid {
		known[a.Name] = true
		names = append(names, a.Name)
	}
	sort.Strings(names)
	out := map[string][]*regexp.Regexp{}
	for name, entry := range cfg {
		if !known[name] {
			return nil, fmt.Errorf("govet config %s: unknown analyzer %q, valid names: %s", path, name, strings.Join(names, ", "))
		}
		for pattern := range entry.ExcludeFiles {
			re, err := regexp.Compile(pattern)
			if err != nil {
				return nil, fmt.Errorf("govet config %s: %w", path, err)
			}
			out[name] = append(out[name], re)
		}
	}
	return out, nil
}

func configure(extra []string) error {
	flags := flag.NewFlagSet("govet", flag.ContinueOnError)
	profile := flags.String("profile", "standard", "standard or strict")
	config := flags.String("config", "", "govet config with excludes per analyzer")
	if err := flags.Parse(extra); err != nil {
		return err
	}
	chosen, err := analyzersFor(*profile)
	if err != nil {
		return err
	}
	active = chosen
	excludes = map[string][]*regexp.Regexp{}
	if *config == "" {
		return nil
	}
	excludes, err = loadExcludes(*config, chosen)
	return err
}

type diagnostic struct {
	Posn string `json:"posn"`
}

// filter drops the findings whose file an analyzer excludes.
func filter(_ *unit.Config, tree unit.Tree) unit.Tree {
	for _, byAnalyzer := range tree {
		for name, raw := range byAnalyzer {
			res := excludes[name]
			if len(res) == 0 {
				continue
			}
			var list []json.RawMessage
			if err := json.Unmarshal(raw, &list); err != nil {
				continue
			}
			kept := []json.RawMessage{}
			for _, item := range list {
				var d diagnostic
				if json.Unmarshal(item, &d) == nil && excluded(res, d.Posn) {
					continue
				}
				kept = append(kept, item)
			}
			if len(kept) == 0 {
				delete(byAnalyzer, name)
				continue
			}
			if encoded, err := json.Marshal(kept); err == nil {
				byAnalyzer[name] = encoded
			}
		}
	}
	return tree
}

func excluded(res []*regexp.Regexp, posn string) bool {
	for range 2 {
		if i := strings.LastIndex(posn, ":"); i >= 0 {
			posn = posn[:i]
		}
	}
	for _, re := range res {
		if re.MatchString(posn) {
			return true
		}
	}
	return false
}

func main() {
	opts := unit.Options{
		Configure: configure,
		Filter:    filter,
		Select:    func([]string) []*analysis.Analyzer { return active },
	}
	os.Exit(unit.Run("govet", defaults, opts, os.Args[1:], os.Stdout, os.Stderr))
}
