package common

import (
	"fmt"
	"reflect"
	"sort"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/rule"
)

type IgnoreEntry struct {
	Value  string
	Path   string
	UsedBy []string
}

// MarkUsed records rel as a package that used the entry, keeping UsedBy sorted and unique.
func (e *IgnoreEntry) MarkUsed(rel string) {
	e.UsedBy = insertSorted(e.UsedBy, rel)
}

// Used reports whether any package used the entry.
func (e *IgnoreEntry) Used() bool { return len(e.UsedBy) > 0 }

func insertSorted(list []string, value string) []string {
	i := sort.SearchStrings(list, value)
	if i < len(list) && list[i] == value {
		return list
	}
	list = append(list, "")
	copy(list[i+1:], list[i:])
	list[i] = value
	return list
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

// GeneratedRels records the packages a run called GenerateRules for; the zero value is empty.
type GeneratedRels struct{ m map[string]bool }

// Add records rel as generated.
func (g *GeneratedRels) Add(rel string) {
	if g.m == nil {
		g.m = map[string]bool{}
	}
	g.m[rel] = true
}

// Has reports whether rel was generated.
func (g GeneratedRels) Has(rel string) bool {
	return g.m[rel]
}

// StaleIgnores reports unused entries declared in generated packages, so a scoped run only checks its own directives.
func StaleIgnores(languageName string, declared []*IgnoreEntry, generated GeneratedRels) []string {
	var errs []string
	for _, ignore := range declared {
		if ignore == nil || ignore.Used() || !generated.Has(ignore.Path) {
			continue
		}
		errs = append(errs, fmt.Sprintf("%s: //%s: stale # gazelle:dx_ignore_import %s %s matches no literal reference",
			languageName, ignore.Path, languageName, ignore.Value))
	}
	return errs
}

// UsedIgnores returns the used entries visible from c, one per (Path, Value).
func UsedIgnores(c *config.Config, languageName string) []IgnoredImport {
	return usedImports(languageName, InheritedIgnores(c, languageName))
}

// UsedImports returns the used declared entries, one per (Path, Value).
func UsedImports(languageName string, declared []*IgnoreEntry) []IgnoredImport {
	return usedImports(languageName, declared)
}

func usedImports(languageName string, entries []*IgnoreEntry) []IgnoredImport {
	index := map[[2]string]int{}
	var out []IgnoredImport
	for _, ig := range entries {
		if ig == nil || !ig.Used() {
			continue
		}
		key := [2]string{ig.Path, ig.Value}
		if i, ok := index[key]; ok {
			out[i].UsedBy = mergeSorted(out[i].UsedBy, ig.UsedBy)
			continue
		}
		index[key] = len(out)
		out = append(out, IgnoredImport{Path: ig.Path, Language: languageName, Value: ig.Value, UsedBy: mergeSorted(nil, ig.UsedBy)})
	}
	return out
}

func mergeSorted(dst, src []string) []string {
	for _, rel := range src {
		dst = insertSorted(dst, rel)
	}
	return dst
}
