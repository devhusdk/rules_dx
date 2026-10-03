package common

import (
	"fmt"
	"reflect"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/rule"
)

type IgnoreEntry struct {
	Value string
	Path  string
	Used  bool
}

type IgnoreConfig struct {
	Ignores []*IgnoreEntry
}

func (c *IgnoreConfig) IgnoreEntries() []*IgnoreEntry {
	if c == nil {
		return nil
	}
	return c.Ignores
}

type IgnoreSource interface {
	IgnoreEntries() []*IgnoreEntry
}

func InheritedIgnores(c *config.Config, languageName string) []*IgnoreEntry {
	if c == nil {
		return nil
	}
	raw, ok := c.Exts[languageName]
	if !ok || raw == nil {
		return nil
	}
	source, ok := raw.(IgnoreSource)
	if !ok {
		return nil
	}
	if value := reflect.ValueOf(source); value.Kind() == reflect.Ptr && value.IsNil() {
		return nil
	}
	return source.IgnoreEntries()
}

func MergeIgnores(inherited, declared []*IgnoreEntry) []*IgnoreEntry {
	var out []*IgnoreEntry
	out = append(out, inherited...)
	return append(out, declared...)
}

func DeclaredIgnores(languageName, rel string, f *rule.File) ([]*IgnoreEntry, []string) {
	if f == nil {
		return nil, nil
	}
	var declared []*IgnoreEntry
	var errs []string
	for _, directive := range f.Directives {
		if directive.Key != "dx_ignore_import" {
			continue
		}
		fields := strings.Fields(directive.Value)
		if len(fields) == 0 || fields[0] != languageName {
			continue
		}
		value := ""
		switch {
		case len(fields) == 2:
			value = fields[1]
		case len(fields) == 3 && fields[1] == languageName:
			value = fields[2]
		default:
			errs = append(errs, fmt.Sprintf("%s: //%s: malformed # gazelle:dx_ignore_import %s",
				languageName, rel, directive.Value))
			continue
		}
		declared = append(declared, &IgnoreEntry{Value: value, Path: rel})
	}
	return declared, errs
}

func MatchingIgnore(c *config.Config, languageName, name string) *IgnoreEntry {
	ignores := InheritedIgnores(c, languageName)
	for i := len(ignores) - 1; i >= 0; i-- {
		if entry := ignores[i]; entry != nil && entry.Value == name {
			return entry
		}
	}
	return nil
}

func StaleIgnores(languageName string, declared []*IgnoreEntry) []string {
	var errs []string
	for _, ignore := range declared {
		if ignore == nil || ignore.Used {
			continue
		}
		errs = append(errs, fmt.Sprintf("%s: //%s: stale # gazelle:dx_ignore_import %s %s matches no literal reference",
			languageName, ignore.Path, languageName, ignore.Value))
	}
	return errs
}

func UsedIgnores(c *config.Config, languageName string) [][2]string {
	return usedIgnorePairs(InheritedIgnores(c, languageName))
}

func UsedImports(languageName string, declared []*IgnoreEntry) []IgnoredImport {
	var out []IgnoredImport
	for _, ignore := range declared {
		if ignore == nil || !ignore.Used {
			continue
		}
		out = append(out, IgnoredImport{Path: ignore.Path, Language: languageName, Value: ignore.Value})
	}
	return out
}

func usedIgnorePairs(entries []*IgnoreEntry) [][2]string {
	seen := map[[2]string]bool{}
	var out [][2]string
	for _, ig := range entries {
		if ig == nil || !ig.Used {
			continue
		}
		key := [2]string{ig.Path, ig.Value}
		if seen[key] {
			continue
		}
		seen[key] = true
		out = append(out, key)
	}
	return out
}