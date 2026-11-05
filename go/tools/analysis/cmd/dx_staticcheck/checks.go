package main

import (
	"flag"
	"fmt"
	"os"
	"strings"
	"unicode"

	"github.com/pelletier/go-toml/v2"
	"golang.org/x/tools/go/analysis"
	"honnef.co/go/tools/analysis/lint"
	"honnef.co/go/tools/config"
	"honnef.co/go/tools/simple"
	"honnef.co/go/tools/staticcheck"
	"honnef.co/go/tools/stylecheck"
	"honnef.co/go/tools/unused"
)

func allAnalyzers() []*lint.Analyzer {
	var out []*lint.Analyzer
	for _, set := range [][]*lint.Analyzer{staticcheck.Analyzers, simple.Analyzers, stylecheck.Analyzers} {
		out = append(out, set...)
	}
	return append(out, unused.Analyzer)
}

// defaultConfig is staticcheck's own default: every check except the ones
// marked non-default.
func defaultConfig() config.Config {
	cfg := config.DefaultConfig
	cfg.Checks = []string{"all"}
	for _, a := range allAnalyzers() {
		if a.Doc.NonDefault {
			cfg.Checks = append(cfg.Checks, "-"+a.Analyzer.Name)
		}
	}
	return cfg
}

// loadConfig reads one staticcheck.conf and merges it onto the default.
// No parent directory is searched.
func loadConfig(path string) (config.Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return config.Config{}, err
	}
	var own config.Config
	if err := toml.Unmarshal(data, &own); err != nil {
		return config.Config{}, fmt.Errorf("%s: %w", path, err)
	}
	return defaultConfig().Merge(own), nil
}

// selectChecks applies staticcheck's checks rules: all or *, -NAME,
// category globs such as ST*, prefix globs such as SA1*, and plain names.
func selectChecks(names, rules []string) map[string]bool {
	allowed := map[string]bool{}
	for _, rule := range rules {
		on := true
		if len(rule) > 1 && rule[0] == '-' {
			on = false
			rule = rule[1:]
		}
		rule = strings.ToUpper(rule)
		switch {
		case rule == "*" || rule == "ALL":
			for _, name := range names {
				allowed[name] = on
			}
		case strings.HasSuffix(rule, "*"):
			prefix := strings.TrimSuffix(rule, "*")
			category := strings.IndexFunc(prefix, unicode.IsNumber) == -1
			for _, name := range names {
				if category {
					if i := strings.IndexFunc(name, unicode.IsNumber); i >= 0 && name[:i] == prefix {
						allowed[name] = on
					}
				} else if strings.HasPrefix(name, prefix) {
					allowed[name] = on
				}
			}
		default:
			allowed[rule] = on
		}
	}
	return allowed
}

func selectAnalyzers(cfg config.Config) []*analysis.Analyzer {
	all := allAnalyzers()
	names := make([]string, len(all))
	for i, a := range all {
		names[i] = strings.ToUpper(a.Analyzer.Name)
	}
	allowed := selectChecks(names, cfg.Checks)
	var out []*analysis.Analyzer
	for i, a := range all {
		if allowed[names[i]] {
			out = append(out, a.Analyzer)
		}
	}
	return out
}

var active = defaultConfig()

// configure reads -config FILE and makes the staticcheck config analyzer
// return it instead of searching the source tree.
func configure(extra []string) error {
	flags := flag.NewFlagSet("staticcheck", flag.ContinueOnError)
	path := flags.String("config", "", "staticcheck.conf to apply")
	if err := flags.Parse(extra); err != nil {
		return err
	}
	active = defaultConfig()
	if *path != "" {
		cfg, err := loadConfig(*path)
		if err != nil {
			return err
		}
		active = cfg
	}
	config.Analyzer.Run = func(*analysis.Pass) (any, error) {
		cfg := active
		return &cfg, nil
	}
	return nil
}

func selected([]string) []*analysis.Analyzer {
	return selectAnalyzers(active)
}
