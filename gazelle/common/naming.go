package common

import (
	"fmt"
	"path"
	"sort"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/language"
)

func Normalize(base string) (string, error) {
	var b strings.Builder
	b.Grow(len(base))
	pending := false
	for i := 0; i < len(base); i++ {
		c := base[i]
		if c <= 0x7F && (c == '_' ||
			(c >= 'a' && c <= 'z') ||
			(c >= 'A' && c <= 'Z') ||
			(c >= '0' && c <= '9')) {
			if pending && b.Len() > 0 {
				b.WriteByte('_')
			}
			pending = false
			b.WriteByte(c)
			continue
		}
		pending = true
	}
	out := strings.Trim(b.String(), "_")
	if out == "" {
		return "", fmt.Errorf("naming: %q normalizes to an empty target name", base)
	}
	return out, nil
}

func stripExt(base string, exts []string) (string, bool) {
	for _, ext := range exts {
		if strings.HasSuffix(base, ext) {
			return strings.TrimSuffix(base, ext), true
		}
	}
	return "", false
}

func DirTargetName(dir string) (string, error) {
	return Normalize(path.Base(dir))
}

func RootTargetName(hints ...string) string {
	for _, hint := range hints {
		if name, err := Normalize(hint); err == nil {
			return name
		}
	}
	return "lib"
}

func PackageTargetName(args language.GenerateArgs, hints ...string) (string, error) {
	if args.Rel != "" {
		return DirTargetName(args.Rel)
	}
	repoName := ""
	if args.Config != nil {
		repoName = args.Config.RepoName
	}
	return RootTargetName(append(append([]string{}, hints...), repoName)...), nil
}

func Stem(name string, exts []string) string {
	base := path.Base(name)
	if trimmed, ok := stripExt(base, exts); ok {
		return trimmed
	}
	return base
}

func IsEntryFile(name string, exts []string) bool {
	base := path.Base(name)
	for _, ext := range exts {
		if base == "main"+ext {
			return true
		}
	}
	return false
}

func BinaryName(target string) string {
	return target + "_bin"
}

func UnitTestName(target string) string {
	return target + "_test"
}

func ClassIdentity(name string, exts []string) string {
	short := Stem(name, exts)
	if i := strings.LastIndex(short, "."); i >= 0 {
		return short[i+1:]
	}
	return short
}

func IsTestSource(name string, exts []string, suffixes []string) bool {
	short := Stem(name, exts)
	for _, suffix := range suffixes {
		if strings.HasSuffix(short, suffix) {
			return true
		}
	}
	return false
}

type Claimant struct {
	Name   string
	Source string
	Kind   string
}

type CollisionError struct {
	Name      string
	Claimants []string
}

func (e *CollisionError) Error() string {
	return fmt.Sprintf("naming: normalized name %q claimed by %s; rename a source or keep one target handwritten",
		e.Name, strings.Join(e.Claimants, ", "))
}

func UnionStrings(a, b []string) []string {
	seen := make(map[string]bool, len(a)+len(b))
	var out []string
	for _, s := range append(append([]string{}, a...), b...) {
		if !seen[s] {
			seen[s] = true
			out = append(out, s)
		}
	}
	sort.Strings(out)
	return out
}

func IsFixturePath(rel string) bool {
	padded := "/" + rel + "/"
	return strings.Contains(padded, "/tests/") || strings.Contains(padded, "/fixtures/") || strings.Contains(padded, "/testdata/")
}
